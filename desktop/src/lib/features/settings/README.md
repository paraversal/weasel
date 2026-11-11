# Settings

Preference screen composed from `registry.settingsSections`; other features add their own
sections without touching this folder.

- Commands: `settings_get_all`, `settings_update`.
- Proves the write path: optimistic update → `settingsChanged` → query invalidation.
- Spec: `docs/rewrite/08-ui-design.md` § Settings.
