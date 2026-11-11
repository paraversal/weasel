// Repository modules use these so row decoding is uniform.

use std::{fmt::Display, str::FromStr};

use serde::{Serialize, de::DeserializeOwned};
use weasel_core::{CoreError, CoreResult};

/// Decode an enum stored as its `as_str()` text. Unknown values are bugs: the CHECK
/// constraints in the schema are the authority.
pub(crate) fn decode_enum<T>(col: &str, raw: String) -> CoreResult<T>
where
    T: FromStr,
    T::Err: Display,
{
    match raw.parse::<T>() {
        Ok(value) => Ok(value),
        Err(err) => Err(CoreError::internal(std::io::Error::other(format!(
            "unknown {col} value {raw:?}: {err}"
        )))),
    }
}

/// Decode a JSON column. Malformed JSON is a bug, not user input.
pub(crate) fn decode_json<T: DeserializeOwned>(col: &str, raw: &str) -> CoreResult<T> {
    serde_json::from_str(raw).map_err(|err| {
        CoreError::internal(std::io::Error::other(format!(
            "invalid JSON in {col}: {err}"
        )))
    })
}

/// Encode a value for a JSON column.
pub(crate) fn encode_json<T: Serialize>(value: &T) -> CoreResult<String> {
    serde_json::to_string(value).map_err(CoreError::internal)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[derive(Debug, PartialEq, Eq)]
    enum Kind {
        Alpha,
        Beta,
    }

    impl FromStr for Kind {
        type Err = String;

        fn from_str(raw: &str) -> Result<Self, Self::Err> {
            match raw {
                "alpha" => Ok(Self::Alpha),
                "beta" => Ok(Self::Beta),
                other => Err(format!("bad kind {other}")),
            }
        }
    }

    #[test]
    fn decodes_known_and_rejects_unknown_enums() {
        assert_eq!(decode_enum::<Kind>("kind", "alpha".into()).unwrap(), Kind::Alpha);
        assert!(matches!(
            decode_enum::<Kind>("kind", "gamma".into()),
            Err(CoreError::Internal(_))
        ));
    }

    #[test]
    fn json_round_trip_and_rejection() {
        let encoded = encode_json(&serde_json::json!({ "a": 1 })).unwrap();
        let decoded: serde_json::Value = decode_json("col", &encoded).unwrap();
        assert_eq!(decoded["a"], 1);
        assert!(matches!(
            decode_json::<serde_json::Value>("col", "{"),
            Err(CoreError::Internal(_))
        ));
    }
}
