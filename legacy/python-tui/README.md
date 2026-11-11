# Weasel TUI (reference only)

Reference implementation of the Textual TUI. Not maintained. Its database format is
**not** supported by the new app — there is no import path and no compatibility
guarantee.

The code is kept because [docs/rewrite/01-current-state.md](../../docs/rewrite/01-current-state.md)
inventories its behaviour, and having the source next to that inventory is cheaper than
reading git history.

## Running it

For inspection only:

```sh
cd legacy/python-tui
uv run main.py
```

It opens `weasel_lair.db` relative to the current directory.

## Original roadmap

Kept verbatim from the root README before the rewrite. [03-data-model.md](../../docs/rewrite/03-data-model.md)
and [05-extensibility.md](../../docs/rewrite/05-extensibility.md) reference it.

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
