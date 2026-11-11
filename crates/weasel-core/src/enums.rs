//! Enumerations that cross the persistence and IPC boundaries.
//!
//! Each `as_str()` is exactly the text stored by SQLite's CHECK constraints. Parsing
//! is total over stored values and rejects everything else.

macro_rules! string_enum {
    ($(#[$meta:meta])* $vis:vis enum $name:ident { $($variant:ident => $text:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
        #[serde(rename_all = "snake_case")]
        $vis enum $name {
            $($variant),+
        }

        impl $name {
            pub fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $text),+
                }
            }
        }

        impl std::str::FromStr for $name {
            type Err = String;

            fn from_str(raw: &str) -> Result<Self, Self::Err> {
                match raw {
                    $($text => Ok(Self::$variant),)+
                    other => Err(format!(
                        concat!("unknown ", stringify!($name), " value {:?}"),
                        other
                    )),
                }
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(self.as_str())
            }
        }
    };
}

string_enum! {
    /// Whether a person is in the active roster, deliberately removed, or archived.
    pub enum ContactStatus {
        Active => "active",
        Removed => "removed",
        Archived => "archived",
    }
}

string_enum! {
    /// Where a contact row came from.
    pub enum ContactSource {
        Import => "import",
        Manual => "manual",
        Api => "api",
    }
}

string_enum! {
    /// Why an address changed.
    pub enum EmailChangeReason {
        Import => "import",
        Manual => "manual",
        Merge => "merge",
        Correction => "correction",
    }
}

string_enum! {
    pub enum CampaignStatus {
        Draft => "draft",
        Active => "active",
        Archived => "archived",
    }
}

string_enum! {
    pub enum SendRecordStatus {
        Created => "created",
        Copied => "copied",
        Sent => "sent",
        Failed => "failed",
        Skipped => "skipped",
    }
}

string_enum! {
    pub enum ImportStatus {
        Draft => "draft",
        Applied => "applied",
        Failed => "failed",
        Discarded => "discarded",
    }
}

string_enum! {
    /// Roster imports own the list; delta imports only add/update.
    pub enum ImportScope {
        Roster => "roster",
        Delta => "delta",
    }
}

string_enum! {
    pub enum SourceKind {
        Xlsx => "xlsx",
        Xls => "xls",
        Ods => "ods",
        Csv => "csv",
        Manual => "manual",
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn every_enum_round_trips_through_its_stored_text() {
        let cases: Vec<(&str, &str)> = vec![
            (ContactStatus::Archived.as_str(), "archived"),
            (ContactSource::Import.as_str(), "import"),
            (EmailChangeReason::Merge.as_str(), "merge"),
            (CampaignStatus::Draft.as_str(), "draft"),
            (SendRecordStatus::Copied.as_str(), "copied"),
            (ImportStatus::Discarded.as_str(), "discarded"),
            (ImportScope::Roster.as_str(), "roster"),
            (SourceKind::Ods.as_str(), "ods"),
        ];
        for (actual, expected) in cases {
            assert_eq!(actual, expected);
        }

        assert_eq!(
            "active".parse::<ContactStatus>().unwrap(),
            ContactStatus::Active
        );
        assert_eq!("xlsx".parse::<SourceKind>().unwrap(), SourceKind::Xlsx);
        assert!("legacy".parse::<SourceKind>().is_err());
        assert!("Active".parse::<ContactStatus>().is_err());
    }

    #[test]
    fn serde_uses_the_stored_text() {
        assert_eq!(
            serde_json::to_string(&ContactStatus::Removed).unwrap(),
            "\"removed\""
        );
        assert_eq!(
            serde_json::from_str::<SourceKind>("\"csv\"").unwrap(),
            SourceKind::Csv
        );
    }
}
