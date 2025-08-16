use async_graphql::{InputValueError, InputValueResult, Scalar, ScalarType, Value};
use std::fmt;

/// Very simple Money scalar represented as integer cents (e.g., 1299 == €12.99).
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, Default)]
pub struct Money(pub i64);

impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let euros = self.0 as f64 / 100.0;
        write!(f, "{:.2}", euros)
    }
}

#[Scalar]
impl ScalarType for Money {
    fn parse(value: async_graphql::Value) -> InputValueResult<Self> {
        match value {
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(Money(i))
                } else {
                    Err(InputValueError::custom("Money expects integer cents (i64)"))
                }
            }
            _ => Err(InputValueError::custom(
                "Money must be a number (integer cents)",
            )),
        }
    }

    fn to_value(&self) -> Value {
        Value::Number(self.0.into())
    }
}
