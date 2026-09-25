//! Validated helpers for the current BitMEX v2 order endpoint.

use crate::{
    AccountId, ApiResponse, Client, Error, OperationError, ProviderRejection, Symbol,
    generated::models::{NewOrderV2Body, NewOrderV2Response},
    realtime::Pool,
};
use rust_decimal::Decimal;
use std::num::NonZeroI64;

/// Explicit order side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// Buy side.
    Buy,
    /// Sell side.
    Sell,
}
impl Side {
    fn as_str(self) -> &'static str {
        match self {
            Self::Buy => "Buy",
            Self::Sell => "Sell",
        }
    }
}

/// BitMEX time-in-force selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeInForce {
    /// Rest until filled or explicitly cancelled.
    GoodTillCancel,
    /// Execute immediately and cancel any remainder.
    ImmediateOrCancel,
    /// Execute completely or cancel.
    FillOrKill,
    /// Valid through the trading day.
    Day,
    /// Execute at the close.
    AtTheClose,
}
impl TimeInForce {
    fn as_str(self) -> &'static str {
        match self {
            Self::GoodTillCancel => "GoodTillCancel",
            Self::ImmediateOrCancel => "ImmediateOrCancel",
            Self::FillOrKill => "FillOrKill",
            Self::Day => "Day",
            Self::AtTheClose => "AtTheClose",
        }
    }
}

/// A validated v2 order request with explicit side and type.
#[derive(Debug, Clone)]
pub struct NewOrder {
    body: NewOrderV2Body,
}

impl NewOrder {
    fn base(symbol: Symbol, side: Side, qty: NonZeroI64, order_type: &str) -> Result<Self, Error> {
        if qty.get() <= 0 {
            return Err(Error::InvalidInput("quantity must be positive".into()));
        }
        Ok(Self {
            body: NewOrderV2Body {
                cl_ord_id: None,
                cl_ord_link_id: None,
                contingency_type: None,
                display_qty: None,
                exec_inst: None,
                expiry_time: None,
                max_slippage_pct: None,
                ord_type: Some(order_type.into()),
                order_id: None,
                order_qty: Some(qty.get()),
                peg_offset_value: None,
                peg_price_type: None,
                pool: None,
                price: None,
                side: Some(side.as_str().into()),
                stop_px: None,
                strategy: None,
                symbol: symbol.as_str().into(),
                target_account_id: None,
                text: None,
                time_in_force: None,
            },
        })
    }

    /// Build a limit order.
    pub fn limit(
        symbol: Symbol,
        side: Side,
        qty: NonZeroI64,
        price: Decimal,
    ) -> Result<Self, Error> {
        if price <= Decimal::ZERO {
            return Err(Error::InvalidInput("price must be positive".into()));
        }
        let mut order = Self::base(symbol, side, qty, "Limit")?;
        order.body.price = Some(price);
        Ok(order)
    }

    /// Build a market order.
    pub fn market(symbol: Symbol, side: Side, qty: NonZeroI64) -> Result<Self, Error> {
        Self::base(symbol, side, qty, "Market")
    }

    /// Build a stop-market order.
    pub fn stop(
        symbol: Symbol,
        side: Side,
        qty: NonZeroI64,
        stop_price: Decimal,
    ) -> Result<Self, Error> {
        if stop_price <= Decimal::ZERO {
            return Err(Error::InvalidInput("stop price must be positive".into()));
        }
        let mut order = Self::base(symbol, side, qty, "Stop")?;
        order.body.stop_px = Some(stop_price);
        Ok(order)
    }

    /// Build a stop-limit order.
    pub fn stop_limit(
        symbol: Symbol,
        side: Side,
        qty: NonZeroI64,
        stop_price: Decimal,
        price: Decimal,
    ) -> Result<Self, Error> {
        let mut order = Self::limit(symbol, side, qty, price)?;
        if stop_price <= Decimal::ZERO {
            return Err(Error::InvalidInput("stop price must be positive".into()));
        }
        order.body.ord_type = Some("StopLimit".into());
        order.body.stop_px = Some(stop_price);
        Ok(order)
    }

    /// Set the client order ID used for reconciliation.
    pub fn client_order_id(mut self, id: impl Into<String>) -> Result<Self, Error> {
        let id = id.into();
        if id.is_empty() || id.len() > 64 || id.chars().any(char::is_control) {
            return Err(Error::InvalidInput("invalid client order ID".into()));
        }
        self.body.cl_ord_id = Some(id);
        Ok(self)
    }

    /// Set an explicit account identity for a linked account.
    pub fn target_account(mut self, account: AccountId) -> Self {
        self.body.target_account_id = Some(account.get());
        self
    }

    /// Choose a protected liquidity pool.
    pub fn pool(mut self, pool: Pool) -> Self {
        self.body.pool = Some(pool.as_str().into());
        self
    }

    /// Choose time in force.
    pub fn time_in_force(mut self, value: TimeInForce) -> Self {
        self.body.time_in_force = Some(value.as_str().into());
        self
    }

    /// Restrict an order to reducing the open position.
    pub fn reduce_only(mut self) -> Self {
        self.add_exec_inst("ReduceOnly");
        self
    }

    /// Place this order passively, cancelling if it would cross the book.
    pub fn post_only(mut self) -> Self {
        self.add_exec_inst("ParticipateDoNotInitiate");
        self
    }

    fn add_exec_inst(&mut self, instruction: &str) {
        match self.body.exec_inst.as_mut() {
            Some(current) if !current.split(',').any(|value| value == instruction) => {
                current.push(',');
                current.push_str(instruction);
            }
            Some(_) => {}
            None => self.body.exec_inst = Some(instruction.into()),
        }
    }
}

impl Client {
    /// Place a validated order through BitMEX's current v2 endpoint.
    pub async fn place_order_v2(
        &self,
        order: &NewOrder,
    ) -> Result<ApiResponse<NewOrderV2Response>, OperationError<ProviderRejection>> {
        self.new_order_v2(&order.body).await
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn execution_instructions_compose() {
        let order = NewOrder::limit(
            Symbol::new("XBTUSD").expect("symbol"),
            Side::Buy,
            NonZeroI64::new(1).expect("quantity"),
            Decimal::from(100),
        )
        .expect("order")
        .reduce_only()
        .post_only()
        .post_only();
        assert_eq!(
            order.body.exec_inst.as_deref(),
            Some("ReduceOnly,ParticipateDoNotInitiate")
        );
    }
}
