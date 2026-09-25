//! Exact decimal handling for BitMEX numeric JSON values.

use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};
use std::str::FromStr;

fn decode<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Decimal, D::Error> {
    let value = serde_json::Value::deserialize(deserializer)?;
    match value {
        serde_json::Value::Number(number) => {
            parse_decimal(&number.to_string()).map_err(D::Error::custom)
        }
        serde_json::Value::String(text) => parse_decimal(&text).map_err(D::Error::custom),
        _ => Err(D::Error::custom("expected decimal number or string")),
    }
}

pub(crate) fn parse_decimal(value: &str) -> Result<Decimal, rust_decimal::Error> {
    if value.contains(['e', 'E']) {
        Decimal::from_scientific(value)
    } else {
        Decimal::from_str(value)
    }
}

fn encode<S: Serializer>(decimal: &Decimal, serializer: S) -> Result<S::Ok, S::Error> {
    let number =
        serde_json::Number::from_str(&decimal.to_string()).map_err(serde::ser::Error::custom)?;
    number.serialize(serializer)
}

pub(crate) mod value {
    use super::*;
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Decimal, D::Error> {
        decode(deserializer)
    }
    pub fn serialize<S: Serializer>(value: &Decimal, serializer: S) -> Result<S::Ok, S::Error> {
        encode(value, serializer)
    }
}

pub(crate) mod optional {
    use super::*;
    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<Decimal>, D::Error> {
        let value = Option::<serde_json::Value>::deserialize(deserializer)?;
        value
            .map(|v| match v {
                serde_json::Value::Number(n) => {
                    parse_decimal(&n.to_string()).map_err(D::Error::custom)
                }
                serde_json::Value::String(s) => parse_decimal(&s).map_err(D::Error::custom),
                _ => Err(D::Error::custom("expected decimal number or string")),
            })
            .transpose()
    }
    pub fn serialize<S: Serializer>(
        value: &Option<Decimal>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match value {
            Some(v) => encode(v, serializer),
            None => serializer.serialize_none(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize)]
    struct Fixture {
        #[serde(with = "value")]
        amount: Decimal,
    }

    #[test]
    fn numeric_and_string_decimal_are_exact() {
        let numeric: Fixture = serde_json::from_str(r#"{"amount":0.000000000000000000123456789}"#)
            .expect("numeric decimal");
        let quoted: Fixture = serde_json::from_str(r#"{"amount":"0.000000000000000000123456789"}"#)
            .expect("quoted decimal");
        assert_eq!(numeric.amount, quoted.amount);
        assert_eq!(
            serde_json::to_string(&numeric).expect("serialize decimal"),
            r#"{"amount":0.000000000000000000123456789}"#
        );
        let scientific: Fixture =
            serde_json::from_str(r#"{"amount":1.23e-8}"#).expect("scientific decimal");
        assert_eq!(scientific.amount.to_string(), "0.0000000123");
    }
}
