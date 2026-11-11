//! Pagination and search helpers shared by repositories and services.

use serde::{Deserialize, Serialize};

pub const DEFAULT_PAGE_LIMIT: u32 = 100;
pub const MAX_PAGE_LIMIT: u32 = 500;

/// Raw page input from a caller. `normalized()` applies the contract.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageRequest {
    /// 0 means "use the default".
    pub limit: u32,
    pub offset: u32,
}

impl PageRequest {
    pub fn new(limit: u32, offset: u32) -> Self {
        Self { limit, offset }
    }

    pub fn normalized(self) -> Self {
        let limit = if self.limit == 0 {
            DEFAULT_PAGE_LIMIT
        } else {
            self.limit.clamp(1, MAX_PAGE_LIMIT)
        };
        Self {
            limit,
            offset: self.offset,
        }
    }
}

/// One page plus the total number of matching rows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: u64,
    pub offset: u32,
    pub limit: u32,
}

impl<T> Page<T> {
    pub fn new(items: Vec<T>, total: u64, page: PageRequest) -> Self {
        let page = page.normalized();
        Self {
            items,
            total,
            offset: page.offset,
            limit: page.limit,
        }
    }

    pub fn map<U>(self, mut map: impl FnMut(T) -> U) -> Page<U> {
        Page {
            items: self.items.into_iter().map(&mut map).collect(),
            total: self.total,
            offset: self.offset,
            limit: self.limit,
        }
    }
}

/// Build a case-insensitive contains pattern for SQLite `LIKE … ESCAPE '\'`.
///
/// `None` means the term is blank and must not filter. The caller must use
/// `ESCAPE '\'`; SQLite's `LIKE` is ASCII-only case-insensitive, which is why the
/// term is lowercased here and the column holds normalized text.
pub fn like_pattern(term: &str) -> Option<String> {
    let term = term.trim();
    if term.is_empty() {
        return None;
    }
    let mut escaped = String::with_capacity(term.len() + 2);
    for ch in term.to_lowercase().chars() {
        match ch {
            '\\' | '%' | '_' => {
                escaped.push('\\');
                escaped.push(ch);
            }
            _ => escaped.push(ch),
        }
    }
    Some(format!("%{escaped}%"))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn clamps_and_defaults_page_requests() {
        assert_eq!(
            PageRequest::new(0, 0).normalized(),
            PageRequest::new(DEFAULT_PAGE_LIMIT, 0)
        );
        assert_eq!(PageRequest::new(501, 7).normalized().limit, MAX_PAGE_LIMIT);
        assert_eq!(PageRequest::new(1, 7).normalized().limit, 1);
        assert_eq!(PageRequest::new(42, 7).normalized().offset, 7);
    }

    #[test]
    fn blank_search_terms_do_not_filter() {
        assert_eq!(like_pattern("   "), None);
        assert_eq!(like_pattern(""), None);
    }

    #[test]
    fn like_pattern_escapes_wildcards_and_lowercases() {
        assert_eq!(like_pattern(" Ada "), Some("%ada%".to_owned()));
        assert_eq!(like_pattern("100%"), Some("%100\\%%".to_owned()));
        assert_eq!(like_pattern("a_b"), Some("%a\\_b%".to_owned()));
        assert_eq!(like_pattern("a\\b"), Some("%a\\\\b%".to_owned()));
        assert_eq!(like_pattern("A_B%C"), Some("%a\\_b\\%c%".to_owned()));
    }

    #[test]
    fn page_map_keeps_metadata() {
        let page = Page::new(vec![1, 2], 9, PageRequest::new(2, 4));
        let mapped = page.map(|value| value * 10);
        assert_eq!(mapped.items, vec![10, 20]);
        assert_eq!(mapped.total, 9);
        assert_eq!(mapped.offset, 4);
        assert_eq!(mapped.limit, 2);
    }
}
