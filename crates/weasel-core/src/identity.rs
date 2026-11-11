//! Person identity.
//!
//! The identity strategy is stored in `app_meta.identity_strategy`; changing it is a
//! data migration, not a config tweak.

use unicode_normalization::UnicodeNormalization;

/// Trim, collapse internal whitespace runs to one space, then NFC-normalize.
pub fn clean_name(raw: &str) -> String {
    let collapsed = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.nfc().collect()
}

/// How a person is recognized across imports when their email changes.
pub trait IdentityStrategy: Send + Sync {
    /// Stable id persisted in `app_meta.identity_strategy`.
    fn id(&self) -> &'static str;
    /// `None` when the input cannot identify anyone.
    fn key(&self, first: &str, last: &str) -> Option<String>;
}

/// First + last name, lowercased after NFC normalization.
///
/// The key is a plain string, not a hash: it is debuggable and there is no collision
/// risk. It stores no information the contacts table does not already hold.
#[derive(Debug, Default, Clone, Copy)]
pub struct NameIdentity;

pub const NAME_IDENTITY_ID: &str = "name-v1";

impl IdentityStrategy for NameIdentity {
    fn id(&self) -> &'static str {
        NAME_IDENTITY_ID
    }

    fn key(&self, first: &str, last: &str) -> Option<String> {
        let first = clean_name(first).to_lowercase();
        let last = clean_name(last).to_lowercase();
        if first.is_empty() || last.is_empty() {
            return None;
        }
        Some(format!("{first}\u{1f}{last}"))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn clean_name_trims_collapses_and_composes() {
        assert_eq!(clean_name("  Ada   Lovelace  "), "Ada Lovelace");
        assert_eq!(clean_name("Ada\t\nLovelace"), "Ada Lovelace");
        // Decomposed "ü" composes to one code point.
        assert_eq!(clean_name("Mu\u{308}ller"), "Müller");
        assert_eq!(clean_name("   "), "");
    }

    #[test]
    fn key_is_case_and_normalization_insensitive() {
        let identity = NameIdentity;
        let composed = identity.key("Müller", "Straße").unwrap();
        let decomposed = identity.key("Mu\u{308}ller", "STRASSE");
        // Lowercasing happens after NFC; ß is not expanded here.
        assert_eq!(composed, "müller\u{1f}straße");
        assert_eq!(decomposed, Some("müller\u{1f}strasse".to_owned()));

        assert_eq!(
            identity.key(" Ada ", "LOVELACE").unwrap(),
            identity.key("ada", "lovelace  ").unwrap()
        );
    }

    #[test]
    fn missing_names_have_no_key() {
        let identity = NameIdentity;
        assert_eq!(identity.key("", "Lovelace"), None);
        assert_eq!(identity.key("Ada", "   "), None);
        assert_eq!(identity.id(), "name-v1");
    }
}
