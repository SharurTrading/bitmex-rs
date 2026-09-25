//! Explicitly armed read-only sweep of REST queries and JSON WebSocket topics.
#![cfg(feature = "live-tests")]
#![allow(clippy::expect_used, clippy::panic)]

use bitmex_client::generated::models::*;
use bitmex_client::{
    ApiCredentials, ApiResponse, Client, Environment, Error, OperationError, PathId,
    ProviderRejection, Symbol,
    realtime::{ALL_FEEDS, Event, Topic},
};
use std::time::Duration;

#[derive(Default)]
struct Tally {
    success: usize,
    rejected: usize,
    failed: Vec<String>,
}

fn record<T>(
    name: &str,
    result: Result<ApiResponse<T>, OperationError<ProviderRejection>>,
    tally: &mut Tally,
) {
    match result {
        Ok(_) => {
            tally.success += 1;
            eprintln!("{name}: decoded 2xx");
        }
        Err(OperationError::Rejected { status, .. }) => {
            tally.rejected += 1;
            eprintln!("{name}: HTTP {status}");
        }
        Err(OperationError::Client(Error::Decode(message))) => {
            tally.failed.push(format!("{name}: decode {message}"))
        }
        Err(OperationError::Client(Error::BodyTooLarge)) => {
            tally.failed.push(format!("{name}: response too large"))
        }
        Err(OperationError::Client(Error::RateLimited)) => {
            tally.failed.push(format!("{name}: rate limited"))
        }
        Err(OperationError::Client(_)) => tally
            .failed
            .push(format!("{name}: local or transport error")),
    }
}

fn armed_client() -> Client {
    assert_eq!(
        std::env::var("BITMEX_READ_ONLY_PROBE").ok().as_deref(),
        Some("I_ACCEPT_READ_ONLY_TESTNET")
    );
    let key = std::env::var("BITMEX_TESTNET_API_KEY").expect("set Testnet API key");
    let secret = std::env::var("BITMEX_TESTNET_API_SECRET").expect("set Testnet API secret");
    Client::builder(Environment::Testnet)
        .credentials(ApiCredentials::new(key, secret).expect("valid Testnet credentials"))
        .build()
        .expect("Testnet client")
}

#[tokio::test]
#[ignore = "requires explicit arming and injected Testnet credentials"]
async fn all_callable_gets() {
    let client = armed_client();
    let mut tally = Tally::default();
    macro_rules! probe {
        ($name:ident $(, $arg:expr)*) => {{
            let result = tokio::time::timeout(Duration::from_secs(35), client.$name($($arg),*)).await;
            match result {
                Ok(result) => record(stringify!($name), result, &mut tally),
                Err(_) => tally.failed.push(format!("{}: timeout", stringify!($name))),
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }};
    }
    probe!(address_get);
    probe!(address_config_get);
    probe!(
        announcement_get,
        &AnnouncementGetQuery {
            ..Default::default()
        }
    );
    probe!(announcement_get_urgent);
    probe!(
        api_key_get,
        &ApiKeyGetQuery {
            ..Default::default()
        }
    );
    probe!(api_key_self);
    probe!(account_range);
    probe!(
        chat_get,
        &ChatGetQuery {
            count: Some(rust_decimal::Decimal::ONE),
            ..Default::default()
        }
    );
    probe!(chat_get_channels);
    probe!(chat_get_connected);
    probe!(
        chat_get_pinned_message,
        &ChatGetPinnedMessageQuery {
            channel_id: rust_decimal::Decimal::ONE
        }
    );
    probe!(
        get_execution,
        &GetExecutionQuery {
            count: Some(1),
            ..Default::default()
        }
    );
    probe!(
        get_execution_trade_history,
        &GetExecutionTradeHistoryQuery {
            count: Some(1),
            ..Default::default()
        }
    );
    probe!(
        get_funding,
        &GetFundingQuery {
            count: Some(1),
            ..Default::default()
        }
    );
    probe!(guild_get);
    probe!(
        get_instruments,
        &GetInstrumentsQuery {
            count: Some(1),
            ..Default::default()
        }
    );
    probe!(get_active_instruments);
    probe!(get_active_and_indices_instruments);
    probe!(get_active_intervals);
    probe!(
        get_composite_index,
        &GetCompositeIndexQuery {
            count: Some(1),
            ..Default::default()
        }
    );
    probe!(get_indices_instruments);
    probe!(
        get_instrument_usd_volume,
        &GetInstrumentUsdVolumeQuery {
            ..Default::default()
        }
    );
    probe!(
        get_insurances,
        &GetInsurancesQuery {
            count: Some(1),
            ..Default::default()
        }
    );
    probe!(leaderboard_get_name);
    probe!(
        get_liquidation,
        &GetLiquidationQuery {
            count: Some(1),
            ..Default::default()
        }
    );
    probe!(managed_sub_account_binding_get_investor_bindings);
    probe!(managed_sub_account_binding_get_trading_team_bindings);
    probe!(
        get_order_v1,
        &GetOrderV1Query {
            count: Some(1),
            ..Default::default()
        }
    );
    probe!(get_market_data_v1);
    probe!(porl_get_nonce);
    probe!(porl_get_snapshots);
    probe!(
        get_position,
        &GetPositionQuery {
            count: Some(1),
            ..Default::default()
        }
    );
    probe!(
        get_position_history,
        &GetPositionHistoryQuery {
            count: Some(1),
            ..Default::default()
        }
    );
    probe!(
        get_quote,
        &GetQuoteQuery {
            count: Some(1),
            ..Default::default()
        }
    );
    probe!(
        get_quote_bucketed,
        &GetQuoteBucketedQuery {
            count: Some(1),
            ..Default::default()
        }
    );
    probe!(referral_code_get_all_codes_for_user);
    probe!(
        referral_code_check_referral_code,
        &PathId::new("fixture").expect("path id")
    );
    probe!(
        get_settlements,
        &GetSettlementsQuery {
            count: Some(1),
            ..Default::default()
        }
    );
    probe!(get_stats);
    probe!(get_stats_history);
    probe!(get_stats_history_usd);
    probe!(
        get_trade,
        &GetTradeQuery {
            count: Some(1),
            ..Default::default()
        }
    );
    probe!(
        get_trade_bucketed,
        &GetTradeBucketedQuery {
            count: Some(1),
            ..Default::default()
        }
    );
    probe!(user_get);
    probe!(
        get_affiliate_status,
        &GetAffiliateStatusQuery {
            ..Default::default()
        }
    );
    probe!(get_user_commission);
    probe!(get_user_csa);
    probe!(
        user_get_deposit_address,
        &UserGetDepositAddressQuery {
            currency: "XBt".into(),
            network: "Bitcoin".into()
        }
    );
    probe!(
        user_get_deposit_address_information,
        &UserGetDepositAddressInformationQuery {
            currency: "XBt".into(),
            network: "Bitcoin".into()
        }
    );
    probe!(
        get_execution_history,
        &GetExecutionHistoryQuery {
            symbol: "XBTUSD".into(),
            timestamp: "2025-01-01T00:00:00Z".into()
        }
    );
    probe!(user_get_wallet_transfer_accounts);
    probe!(
        get_margin,
        &GetMarginQuery {
            ..Default::default()
        }
    );
    probe!(
        get_margining_mode,
        &GetMarginingModeQuery {
            ..Default::default()
        }
    );
    probe!(
        get_quote_fill_ratio,
        &GetQuoteFillRatioQuery {
            ..Default::default()
        }
    );
    probe!(
        user_get_quote_value_ratio,
        &UserGetQuoteValueRatioQuery {
            ..Default::default()
        }
    );
    probe!(
        get_staked_amount,
        &GetStakedAmountQuery {
            ..Default::default()
        }
    );
    probe!(
        get_staking_instruments,
        &GetStakingInstrumentsQuery {
            ..Default::default()
        }
    );
    probe!(
        get_trading_settings,
        &GetTradingSettingsQuery {
            ..Default::default()
        }
    );
    probe!(get_trading_volume);
    probe!(
        get_unstaking_requests,
        &GetUnstakingRequestsQuery {
            status: "Pending".into()
        }
    );
    probe!(
        get_user_wallet,
        &GetUserWalletQuery {
            account: "0".into(),
            ..Default::default()
        }
    );
    probe!(
        get_wallet_history,
        &GetWalletHistoryQuery {
            count: Some(1),
            ..Default::default()
        }
    );
    probe!(
        get_wallet_summary,
        &GetWalletSummaryQuery {
            ..Default::default()
        }
    );
    probe!(
        user_affiliates_get,
        &UserAffiliatesGetQuery {
            ..Default::default()
        }
    );
    probe!(
        user_event_get,
        &UserEventGetQuery {
            count: Some(rust_decimal::Decimal::ONE),
            ..Default::default()
        }
    );
    probe!(
        user_price_alert_get_alerts,
        &UserPriceAlertGetAlertsQuery {
            ..Default::default()
        }
    );
    probe!(
        get_volume_rank,
        &GetVolumeRankQuery {
            start_time: "2025-01-01T00:00:00Z".into(),
            end_time: "2025-01-02T00:00:00Z".into(),
            ..Default::default()
        }
    );
    probe!(get_wallet_assets);
    probe!(get_wallet_currencies);
    probe!(get_wallet_conversion_haircut);
    probe!(get_wallet_networks);
    eprintln!(
        "GET sweep: {}/{} decoded 2xx; {} provider rejections; {} failures",
        tally.success,
        71,
        tally.rejected,
        tally.failed.len()
    );
    assert!(
        tally.failed.is_empty(),
        "GET sweep failures: {:?}",
        tally.failed
    );
    assert_eq!(tally.success + tally.rejected, 71);
}

#[tokio::test]
#[ignore = "requires explicit arming and injected Testnet credentials"]
async fn all_documented_websocket_topics() {
    let client = armed_client();
    let mut accepted = 0;
    let mut rejected = 0;
    let mut failed = Vec::new();
    for feed in ALL_FEEDS {
        let symbol = if feed.supports_pool() {
            Some(Symbol::new("XBTUSD").expect("Testnet symbol"))
        } else {
            None
        };
        let expected = match symbol.as_ref() {
            Some(symbol) => format!("{}:{}", feed.as_str(), symbol.as_str()),
            None => feed.as_str().to_owned(),
        };
        let topic = Topic::new(feed, symbol, None).expect("documented topic");
        let connection = tokio::time::timeout(
            Duration::from_secs(12),
            client.connect_realtime(feed.service()),
        )
        .await;
        let mut connection = match connection {
            Ok(Ok(connection)) => connection,
            Ok(Err(error)) => {
                failed.push(format!("{}: {error}", feed.as_str()));
                continue;
            }
            Err(_) => {
                failed.push(format!("{}: connect timeout", feed.as_str()));
                continue;
            }
        };
        if connection.subscribe(&topic).await.is_err() {
            failed.push(format!("{}: subscribe send failed", feed.as_str()));
            let _ = connection.close().await;
            continue;
        }
        let deadline = tokio::time::Instant::now() + Duration::from_secs(12);
        let mut acknowledged = false;
        for _ in 0..64 {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                break;
            }
            match tokio::time::timeout(remaining, connection.next_event()).await {
                Ok(Ok(Event::Subscribed { topic, .. })) if topic == expected => {
                    accepted += 1;
                    acknowledged = true;
                    eprintln!("{}: subscribed", feed.as_str());
                    break;
                }
                Ok(Ok(Event::Rejected { status, .. })) => {
                    rejected += 1;
                    acknowledged = true;
                    eprintln!("{}: rejected {status:?}", feed.as_str());
                    break;
                }
                Ok(Ok(Event::Gap { reason })) => {
                    failed.push(format!("{}: gap {reason:?}", feed.as_str()));
                    acknowledged = true;
                    break;
                }
                Ok(Ok(_)) => {}
                _ => {
                    failed.push(format!("{}: receive failed", feed.as_str()));
                    acknowledged = true;
                    break;
                }
            }
        }
        if !acknowledged {
            failed.push(format!("{}: no acknowledgement", feed.as_str()));
        }
        let _ = connection.close().await;
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    eprintln!(
        "WebSocket sweep: {accepted}/30 subscribed; {rejected} rejected; {} failures",
        failed.len()
    );
    assert!(failed.is_empty(), "WebSocket sweep failures: {failed:?}");
    assert_eq!(accepted + rejected, ALL_FEEDS.len());
}
