//! Email parsing and normalization.
//!
//! This is deliberately not RFC 5322. The app deals with human-entered university
//! addresses; anything more permissive would let junk into the database and anything
//! stricter would reject valid addresses from real spreadsheets.

use serde::{Deserialize, Serialize};

use crate::{CoreError, CoreResult};

/// Maximum address length in bytes (RFC 5321 path limit).
pub const MAX_EMAIL_LEN: usize = 254;

/// A validated address, kept in display form and normalized form.
///
/// `display` is what the person typed (trimmed); `normalized` is the whole address
/// lowercased. Matching and uniqueness use `normalized`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailAddress {
    pub display: String,
    pub normalized: String,
}

impl EmailAddress {
    /// Parse and normalize a raw address.
    ///
    /// Rejections carry `field: "email"` so the UI can attach the message to the input.
    pub fn parse(raw: &str) -> CoreResult<Self> {
        let display = raw.trim();
        if display.is_empty() {
            return Err(invalid("email is empty"));
        }
        if display.len() > MAX_EMAIL_LEN {
            return Err(invalid("email is longer than 254 characters"));
        }
        if display.chars().any(char::is_whitespace) {
            return Err(invalid("email contains whitespace"));
        }
        let mut parts = display.split('@');
        let local = parts.next().unwrap_or_default();
        let domain = parts.next().unwrap_or_default();
        if parts.next().is_some() {
            return Err(invalid("email must contain exactly one @"));
        }
        if local.is_empty() {
            return Err(invalid("email is missing the part before @"));
        }
        if domain.is_empty() {
            return Err(invalid("email is missing the part after @"));
        }
        if !domain.contains('.') {
            return Err(invalid("email domain must contain a dot"));
        }
        if domain.starts_with('.') || domain.ends_with('.') {
            return Err(invalid("email domain cannot start or end with a dot"));
        }

        Ok(Self {
            display: display.to_owned(),
            normalized: display.to_lowercase(),
        })
    }
}

impl std::fmt::Display for EmailAddress {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.display)
    }
}

fn invalid(reason: &str) -> CoreError {
    CoreError::validation("email", reason)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn accepts_plausible_addresses_and_normalizes_the_whole_address() {
        let cases = [
            ("ada@example.edu", "ada@example.edu"),
            ("  Ada.Lovelace@Example.EDU  ", "ada.lovelace@example.edu"),
            ("a+tag@sub.example.co.uk", "a+tag@sub.example.co.uk"),
            ("o'brien@example.edu", "o'brien@example.edu"),
        ];
        for (raw, normalized) in cases {
            let email = EmailAddress::parse(raw).unwrap();
            assert_eq!(email.normalized, normalized);
            assert_eq!(email.display, raw.trim());
        }
    }

    #[test]
    fn rejects_malformed_addresses() {
        let too_long = format!("{}@example.edu", "a".repeat(MAX_EMAIL_LEN));
        let cases = [
            "",
            "   ",
            "ada example.edu",
            "ada@exa mple.edu",
            "ada@@example.edu",
            "@example.edu",
            "ada@",
            "ada@example",
            "ada@.example.edu",
            "ada@example.edu.",
            "ada@exa@mple.edu",
            too_long.as_str(),
        ];
        for raw in cases {
            let err = EmailAddress::parse(raw).unwrap_err();
            assert!(
                matches!(err, CoreError::Validation { field: "email", .. }),
                "raw {raw:?} produced {err:?}"
            );
        }
    }
}
