# weasel
Simple email campaign management

> **Rewrite planned:** the Textual TUI is being replaced by a Tauri 2 + SvelteKit
> desktop app. See [`docs/rewrite/`](docs/rewrite/README.md) for the architecture,
> data model, extensibility model, UI design and phased delivery plan.

# roadmap

### Better managment of email lists

- currently, we're only checking for new email addresses in each sheet. This ignores two cases:
    1. a person leaving the program (being deleted from the list)
    2. the email address of a person changing in a future sheet (because *that's* something we have to worry about now)
- people leaving the program are currently ignored (they stay in the system and continue to receive emails), people with a changed email address are entered twice (leading to failed sends in later campaigns)
- observation: the db state pertaining to names + email addresses added from email lists should be identical to the email list at any given point - if there is some discrepancy, since the email list is the source of truth, we know that the db is in an invalid state
- ideally, we'd want to have a way for the program to automatically resolve these conflicts
- question is - how do we identify one particular row knowing that the email address might change? Either, by collating names, or, by creating a hash from all columns (other than "email"). The latter feels (slightly) more complicated to implement and I'm not sure there is an actual benefit
    - what do we do if columns *other* than email change? Probably in that case, we should scream and panic!

### Manual addition of emails

- we probably want to have a way to manually add email addresses to deal with edge-cases
    - these should (probably?) be excluded from the db check laid out above since they wouldn't appear in the email lists