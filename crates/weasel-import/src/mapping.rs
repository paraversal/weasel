//! Column mapping: suggest it from headers, then validate the user's choice.

use serde::{Deserialize, Serialize};
use weasel_core::{CoreError, CoreResult, import::ColumnMapping};

use crate::header::{self, FIRST, LAST, EMAIL};

/// Fields found by name. Any field may be absent; the user then picks manually.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestedMapping {
    pub first_name: Option<u32>,
    pub last_name: Option<u32>,
    pub email: Option<u32>,
}

impl SuggestedMapping {
    pub fn is_complete(&self) -> bool {
        self.first_name.is_some() && self.last_name.is_some() && self.email.is_some()
    }
}

/// First column whose folded header is a synonym for each field.
pub fn suggest(headers: &[String]) -> SuggestedMapping {
    let mut suggested = SuggestedMapping::default();
    for (index, header) in headers.iter().enumerate() {
        let folded = header::fold(header);
        let index = u32::try_from(index).unwrap_or(u32::MAX);
        if suggested.first_name.is_none() && FIRST.iter().any(|syn| header::fold(syn) == folded) {
            suggested.first_name = Some(index);
        }
        if suggested.last_name.is_none() && LAST.iter().any(|syn| header::fold(syn) == folded) {
            suggested.last_name = Some(index);
        }
        if suggested.email.is_none() && EMAIL.iter().any(|syn| header::fold(syn) == folded) {
            suggested.email = Some(index);
        }
    }
    suggested
}

/// Turn an explicit or suggested mapping into a validated one.
///
/// `width` is the widest row (header included). Missing fields, out-of-range
/// indexes and a column used twice are user input errors.
pub fn resolve(
    explicit: Option<ColumnMapping>,
    suggested: SuggestedMapping,
    width: usize,
) -> CoreResult<ColumnMapping> {
    let mapping = match explicit {
        Some(mapping) => mapping,
        None => {
            let missing: Vec<&str> = [
                ("first name", suggested.first_name),
                ("last name", suggested.last_name),
                ("email", suggested.email),
            ]
            .into_iter()
            .filter_map(|(name, column)| column.is_none().then_some(name))
            .collect();
            if !missing.is_empty() {
                return Err(CoreError::Validation {
                    field: "mapping",
                    reason: format!(
                        "Could not find a column for {}. Pick the columns manually.",
                        missing.join(", ")
                    ),
                });
            }
            ColumnMapping {
                first_name: suggested.first_name.unwrap_or_default(),
                last_name: suggested.last_name.unwrap_or_default(),
                email: suggested.email.unwrap_or_default(),
            }
        }
    };

    let limit = u32::try_from(width).unwrap_or(u32::MAX);
    for (field, column) in [
        ("first name", mapping.first_name),
        ("last name", mapping.last_name),
        ("email", mapping.email),
    ] {
        if column >= limit {
            return Err(CoreError::Validation {
                field: "mapping",
                reason: format!(
                    "The {field} column index {column} is outside the table ({} columns).",
                    width
                ),
            });
        }
    }
    if mapping.first_name == mapping.last_name
        || mapping.first_name == mapping.email
        || mapping.last_name == mapping.email
    {
        return Err(CoreError::Validation {
            field: "mapping",
            reason: "Each field needs its own column.".to_owned(),
        });
    }
    Ok(mapping)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    fn headers(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    #[test]
    fn suggests_german_and_english_headers() {
        let suggested = suggest(&headers(&["Vorname", "Nachname", "E-Mail-Adresse"]));
        assert_eq!(suggested.first_name, Some(0));
        assert_eq!(suggested.last_name, Some(1));
        assert_eq!(suggested.email, Some(2));
        assert!(suggested.is_complete());

        let english = suggest(&headers(&["First Name", "Last Name", "Email Address"]));
        assert_eq!(english, suggested);
    }

    #[test]
    fn resolve_requires_every_field() {
        let suggested = suggest(&headers(&["Vorname", "Nachname"]));
        let err = resolve(None, suggested, 2).unwrap_err();
        assert!(matches!(
            err,
            CoreError::Validation {
                field: "mapping",
                ..
            }
        ));

        let mapping = resolve(None, suggested, 3).unwrap_err();
        assert!(matches!(mapping, CoreError::Validation { .. }));
    }

    #[test]
    fn resolve_rejects_out_of_range_and_duplicates() {
        let suggested = SuggestedMapping {
            first_name: Some(0),
            last_name: Some(1),
            email: Some(2),
        };
        let err = resolve(None, suggested, 2).unwrap_err();
        assert!(matches!(err, CoreError::Validation { .. }));

        let err = resolve(
            Some(ColumnMapping {
                first_name: 0,
                last_name: 0,
                email: 2,
            }),
            suggested,
            3,
        )
        .unwrap_err();
        assert!(matches!(err, CoreError::Validation { .. }));

        assert_eq!(
            resolve(None, suggested, 3).unwrap(),
            ColumnMapping {
                first_name: 0,
                last_name: 1,
                email: 2
            }
        );
    }
}
