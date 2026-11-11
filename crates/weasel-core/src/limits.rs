//! Size limits. Services validate with the field name so the UI can attach errors.

/// Names are stored cleaned; a person's name is 1..=200 characters.
pub const NAME_MAX: usize = 200;
/// Contact notes.
pub const NOTES_MAX: usize = 10_000;
/// Campaign name.
pub const CAMPAIGN_NAME_MAX: usize = 200;
/// Campaign description (the old `info` note).
pub const DESCRIPTION_MAX: usize = 10_000;
/// Email subject (RFC 2822 recommended line limit).
pub const SUBJECT_MAX: usize = 998;
/// Campaign body.
pub const BODY_MAX: usize = 100_000;
/// Per-batch note in the send history.
pub const BATCH_NOTE_MAX: usize = 2_000;
