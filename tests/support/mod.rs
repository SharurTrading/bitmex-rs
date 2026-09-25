use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::oneshot,
};
use url::Url;

pub async fn serve(status: u16, body: &'static str) -> (Url, oneshot::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("loopback fixture");
    let addr = listener.local_addr().expect("fixture address");
    let url = Url::parse(&format!("http://{addr}/")).expect("fixture URL");
    let (tx, rx) = oneshot::channel();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("fixture connection");
        let mut bytes = Vec::new();
        let mut buffer = [0_u8; 4096];
        loop {
            let n = stream.read(&mut buffer).await.expect("fixture read");
            if n == 0 {
                break;
            }
            bytes.extend_from_slice(&buffer[..n]);
            if bytes.len() > 1024 * 1024 {
                break;
            }
            if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&bytes[..end]).to_ascii_lowercase();
                let length = headers
                    .lines()
                    .find_map(|line| {
                        line.strip_prefix("content-length:")
                            .and_then(|v| v.trim().parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                if bytes.len() >= end + 4 + length {
                    break;
                }
            }
        }
        let request = String::from_utf8_lossy(&bytes).to_string();
        let reason = if status == 200 { "OK" } else { "Bad Request" };
        let response = format!(
            "HTTP/1.1 {status} {reason}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        stream
            .write_all(response.as_bytes())
            .await
            .expect("fixture write");
        let _ = tx.send(request);
    });
    (url, rx)
}
