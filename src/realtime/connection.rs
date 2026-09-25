use super::types::{Action, Event, Feed, GapReason, Pool, Service, TableEvent, Topic, WireValue};
use crate::{Client, Error, client::MutationGuard};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::{collections::HashSet, time::Duration};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async_with_config,
    tungstenite::{Error as WebSocketError, Message, protocol::WebSocketConfig},
};

type Socket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

/// One owned WebSocket generation. No automatic reconnect occurs.
pub struct Connection {
    socket: Socket,
    client: Client,
    service: Service,
    ready_tables: HashSet<Feed>,
    pending_deadman: Option<MutationGuard>,
    max_frame: usize,
    closed: bool,
}

impl Client {
    /// Connect to the primary or platform JSON service.
    pub async fn connect_realtime(&self, service: Service) -> Result<Connection, Error> {
        use tokio_tungstenite::tungstenite::client::IntoClientRequest;
        let platform = service == Service::Platform;
        let mut request = self
            .realtime_endpoint(platform)
            .into_client_request()
            .map_err(|_| Error::InvalidEndpoint)?;
        // BitMEX signs both JSON sockets as GET /realtime, including platform.
        if let Some((key, expires, signature)) = self.realtime_headers()? {
            let headers = request.headers_mut();
            headers.insert(
                "api-key",
                key.parse()
                    .map_err(|_| Error::InvalidInput("invalid API key header".into()))?,
            );
            headers.insert("api-expires", expires.parse().map_err(|_| Error::Clock)?);
            headers.insert(
                "api-signature",
                signature.parse().map_err(|_| Error::Clock)?,
            );
        }
        let max_frame = self.realtime_frame_limit();
        let config = WebSocketConfig::default()
            .max_message_size(Some(max_frame))
            .max_frame_size(Some(max_frame));
        let (socket, _) = connect_async_with_config(request, Some(config), false)
            .await
            .map_err(|error| match error {
                tokio_tungstenite::tungstenite::Error::Http(response) => Error::Transport(format!(
                    "WebSocket upgrade rejected with HTTP {}",
                    response.status().as_u16()
                )),
                _ => Error::Transport("WebSocket connection failed".into()),
            })?;
        Ok(Connection {
            socket,
            client: self.clone(),
            service,
            ready_tables: HashSet::new(),
            pending_deadman: None,
            max_frame,
            closed: false,
        })
    }
}

impl Connection {
    /// Send a subscription request; observe its acknowledgement via `next_event`.
    pub async fn subscribe(&mut self, topic: &Topic) -> Result<(), Error> {
        self.check_topic(topic)?;
        self.client.admit_realtime(false).await?;
        self.send_command("subscribe", Value::Array(vec![Value::String(topic.wire())]))
            .await
    }

    /// Send an unsubscription request.
    pub async fn unsubscribe(&mut self, topic: &Topic) -> Result<(), Error> {
        self.check_topic(topic)?;
        self.client.admit_realtime(false).await?;
        self.send_command(
            "unsubscribe",
            Value::Array(vec![Value::String(topic.wire())]),
        )
        .await
    }

    /// Set the dead-man timer in milliseconds; zero clears it.
    ///
    /// An uncertain send or lost acknowledgement fences mutations until the
    /// caller reconciles state using the REST client.
    pub async fn dead_man_switch(&mut self, milliseconds: u64) -> Result<(), Error> {
        if self.service != Service::Primary {
            return Err(Error::InvalidInput(
                "dead-man switch needs the primary service".into(),
            ));
        }
        if self.pending_deadman.is_some() {
            return Err(Error::MutationFenced {
                scope: "credential".into(),
            });
        }
        self.client.admit_realtime(true).await?;
        self.pending_deadman = Some(self.client.reserve_realtime_mutation()?);
        if self
            .send_command("cancelAllAfter", Value::from(milliseconds))
            .await
            .is_err()
        {
            self.pending_deadman.take();
            return Err(Error::AmbiguousMutation {
                scope: "credential".into(),
            });
        }
        Ok(())
    }

    /// Receive one event, actively probing an idle socket before reporting loss.
    pub async fn next_event(&mut self) -> Result<Event, Error> {
        if self.closed {
            return Ok(Event::Gap {
                reason: GapReason::ConnectionLost,
            });
        }
        let message = match tokio::time::timeout(Duration::from_secs(5), self.socket.next()).await {
            Ok(Some(Ok(message))) => message,
            Ok(Some(Err(WebSocketError::Capacity(_)))) => {
                return Ok(self.gap(GapReason::Oversized));
            }
            Ok(Some(Err(_))) | Ok(None) => return Ok(self.gap(GapReason::ConnectionLost)),
            Err(_) => {
                if self
                    .socket
                    .send(Message::Ping(Vec::new().into()))
                    .await
                    .is_err()
                {
                    return Ok(self.gap(GapReason::ConnectionLost));
                }
                match tokio::time::timeout(Duration::from_secs(5), self.socket.next()).await {
                    Ok(Some(Ok(message))) => message,
                    Ok(Some(Err(WebSocketError::Capacity(_)))) => {
                        return Ok(self.gap(GapReason::Oversized));
                    }
                    _ => return Ok(self.gap(GapReason::ConnectionLost)),
                }
            }
        };
        match message {
            Message::Pong(_) => Ok(Event::Pong),
            Message::Close(_) => Ok(self.gap(GapReason::ConnectionLost)),
            Message::Text(text) => Ok(self.decode(&text)),
            Message::Binary(bytes) => {
                if bytes.len() > self.max_frame {
                    return Ok(self.gap(GapReason::Oversized));
                }
                let text = match std::str::from_utf8(&bytes) {
                    Ok(text) => text,
                    Err(_) => return Ok(self.gap(GapReason::Malformed)),
                };
                Ok(self.decode(text))
            }
            _ => Ok(Event::Pong),
        }
    }

    /// Close this generation explicitly.
    pub async fn close(mut self) -> Result<(), Error> {
        self.closed = true;
        self.pending_deadman.take();
        self.socket
            .close(None)
            .await
            .map_err(|_| Error::Transport("WebSocket close failed".into()))
    }

    fn check_topic(&self, topic: &Topic) -> Result<(), Error> {
        if topic.feed().service() != self.service {
            return Err(Error::InvalidInput(
                "topic belongs to another service".into(),
            ));
        }
        if topic.feed().requires_auth() && self.client.realtime_headers()?.is_none() {
            return Err(Error::MissingCredentials);
        }
        Ok(())
    }

    async fn send_command(&mut self, op: &str, args: Value) -> Result<(), Error> {
        let message = serde_json::json!({"op": op, "args": args}).to_string();
        if message.len() > self.max_frame {
            return Err(Error::InvalidInput("WebSocket command too large".into()));
        }
        self.socket
            .send(Message::Text(message.into()))
            .await
            .map_err(|_| Error::Transport("WebSocket send failed".into()))
    }

    fn gap(&mut self, reason: GapReason) -> Event {
        self.ready_tables.clear();
        self.pending_deadman.take();
        if reason == GapReason::ConnectionLost {
            self.closed = true;
        }
        Event::Gap { reason }
    }

    fn decode(&mut self, text: &str) -> Event {
        if text.len() > self.max_frame {
            return self.gap(GapReason::Oversized);
        }
        let value: Value = match serde_json::from_str(text) {
            Ok(v) => v,
            Err(_) => return self.gap(GapReason::Malformed),
        };
        if value.get("info").is_some() {
            return Event::Welcome;
        }
        if value.get("success").and_then(Value::as_bool) == Some(false)
            && value.get("error").is_none()
        {
            if value
                .get("request")
                .and_then(|r| r.get("op"))
                .and_then(Value::as_str)
                == Some("cancelAllAfter")
            {
                self.pending_deadman.take();
            }
            return Event::Rejected {
                status: value
                    .get("status")
                    .and_then(Value::as_u64)
                    .and_then(|n| u16::try_from(n).ok()),
                message: "provider rejected command".into(),
            };
        }
        if value.get("error").is_none()
            && let Some(topic) = value.get("subscribe").and_then(Value::as_str)
        {
            return Event::Subscribed {
                topic: topic.into(),
                pool: value
                    .get("pool")
                    .and_then(Value::as_str)
                    .and_then(Pool::from_wire),
            };
        }
        if value.get("error").is_none()
            && let Some(topic) = value.get("unsubscribe").and_then(Value::as_str)
        {
            return Event::Unsubscribed {
                topic: topic.into(),
            };
        }
        let command = value
            .get("request")
            .and_then(|v| v.get("op"))
            .and_then(Value::as_str);
        if command == Some("cancelAllAfter")
            && value.get("error").is_none()
            && value.get("cancelTime").is_some()
        {
            if let Some(mut guard) = self.pending_deadman.take() {
                guard.disarm();
            }
            return Event::DeadManAcknowledged {
                cancel_time: value
                    .get("cancelTime")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
            };
        }
        if let Some(error) = value.get("error") {
            let status = value
                .get("status")
                .and_then(Value::as_u64)
                .and_then(|n| u16::try_from(n).ok());
            if command == Some("cancelAllAfter")
                && let Some(mut guard) = self.pending_deadman.take()
                && matches!(status, Some(400 | 401 | 403 | 404 | 409 | 422 | 429))
            {
                guard.disarm();
            }
            return Event::Rejected {
                status,
                message: error.as_str().unwrap_or("provider error").to_owned(),
            };
        }
        let Some(table) = value.get("table").and_then(Value::as_str) else {
            return self.gap(GapReason::Malformed);
        };
        let Some(feed) = Feed::from_wire(table) else {
            return self.gap(GapReason::UnknownTable);
        };
        let action = match value.get("action").and_then(Value::as_str) {
            Some("partial") => Action::Partial,
            Some("insert") => Action::Insert,
            Some("update") => Action::Update,
            Some("delete") => Action::Delete,
            _ => return self.gap(GapReason::Malformed),
        };
        if action != Action::Partial && !self.ready_tables.contains(&feed) {
            return self.gap(GapReason::BeforePartial);
        }
        let Some(data) = value.get("data").and_then(Value::as_array) else {
            return self.gap(GapReason::Malformed);
        };
        let rows = data
            .iter()
            .cloned()
            .map(|v| match WireValue::try_from(v)? {
                WireValue::Object(row) => Ok(row),
                _ => Err(Error::Decode("table row is not an object".into())),
            })
            .collect::<Result<Vec<_>, Error>>();
        let Ok(rows) = rows else {
            return self.gap(GapReason::Malformed);
        };
        let keys = value.get("keys").and_then(Value::as_array).map(|v| {
            v.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        });
        let filter = match value
            .get("filter")
            .cloned()
            .map(WireValue::try_from)
            .transpose()
        {
            Ok(Some(WireValue::Object(filter))) => Some(filter),
            Ok(None) => None,
            _ => return self.gap(GapReason::Malformed),
        };
        if action == Action::Partial {
            self.ready_tables.insert(feed);
        }
        Event::Table(TableEvent {
            feed,
            action,
            rows,
            keys,
            filter,
        })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::{ApiCredentials, Environment};
    use tokio::net::TcpListener;
    use tokio_tungstenite::{accept_async, connect_async};

    #[tokio::test]
    async fn partial_then_delta_and_disconnect_gap() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
        let addr = listener.local_addr().expect("address");
        tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.expect("accept");
            let mut server = accept_async(tcp).await.expect("websocket accept");
            for text in [
                r#"{"info":"welcome"}"#,
                r#"{"success":false,"subscribe":"orderBookL2:XBTUSD","error":"bad pool","status":400}"#,
                r#"{"table":"orderBookL2","action":"update","data":[{"symbol":"XBTUSD","id":42,"size":3}]}"#,
                r#"{"table":"orderBookL2","action":"partial","keys":["symbol","id"],"data":[{"symbol":"XBTUSD","id":42,"side":"Buy","size":2,"price":12345.5}]}"#,
                r#"{"table":"orderBookL2","action":"update","data":[{"symbol":"XBTUSD","id":42,"size":3}]}"#,
            ] {
                server
                    .send(Message::Text(text.into()))
                    .await
                    .expect("send frame");
            }
            server.close(None).await.expect("close");
        });
        let (socket, _) = connect_async(format!("ws://{addr}"))
            .await
            .expect("websocket client");
        let client = Client::builder(Environment::Testnet)
            .build()
            .expect("client");
        let mut connection = Connection {
            socket,
            max_frame: client.realtime_frame_limit(),
            client,
            service: Service::Primary,
            ready_tables: HashSet::new(),
            pending_deadman: None,
            closed: false,
        };
        assert!(matches!(connection.next_event().await, Ok(Event::Welcome)));
        assert!(matches!(
            connection.next_event().await,
            Ok(Event::Rejected {
                status: Some(400),
                ..
            })
        ));
        assert!(matches!(
            connection.next_event().await,
            Ok(Event::Gap {
                reason: GapReason::BeforePartial
            })
        ));
        assert!(matches!(
            connection.next_event().await,
            Ok(Event::Table(TableEvent {
                action: Action::Partial,
                ..
            }))
        ));
        assert!(matches!(
            connection.next_event().await,
            Ok(Event::Table(TableEvent {
                action: Action::Update,
                ..
            }))
        ));
        assert!(matches!(
            connection.next_event().await,
            Ok(Event::Gap {
                reason: GapReason::ConnectionLost
            })
        ));
    }

    #[tokio::test]
    async fn dead_man_acknowledgement_clears_inflight_guard() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
        let addr = listener.local_addr().expect("address");
        tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.expect("accept");
            let mut server = accept_async(tcp).await.expect("websocket accept");
            let command = server.next().await.expect("command").expect("frame");
            assert!(command.to_text().expect("text").contains("cancelAllAfter"));
            server.send(Message::Text(r#"{"cancelTime":"2026-09-25T01:00:00Z","request":{"op":"cancelAllAfter","args":60000}}"#.into())).await.expect("ack");
        });
        let (socket, _) = connect_async(format!("ws://{addr}"))
            .await
            .expect("websocket client");
        let client = Client::builder(Environment::Testnet)
            .credentials(ApiCredentials::new("key", "secret").expect("credentials"))
            .build()
            .expect("client");
        let mut connection = Connection {
            socket,
            max_frame: client.realtime_frame_limit(),
            client: client.clone(),
            service: Service::Primary,
            ready_tables: HashSet::new(),
            pending_deadman: None,
            closed: false,
        };
        connection
            .dead_man_switch(60_000)
            .await
            .expect("send dead-man command");
        assert!(matches!(
            connection.next_event().await,
            Ok(Event::DeadManAcknowledged { .. })
        ));
        assert!(connection.pending_deadman.is_none());
        client.acknowledge_reconciliation("credential");
    }
}
