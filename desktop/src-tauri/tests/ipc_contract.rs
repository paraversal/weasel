//! The generated TypeScript and the backend registry must describe the same IPC surface.
//!
//! Regenerate with `pnpm bindings`; this test fails when a command is registered but not
//! advertised in a `FeatureDescriptor`, or advertised but never registered.

use std::collections::BTreeSet;
use std::fs;

const BINDINGS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/lib/ipc/bindings.ts");

fn bindings_source() -> String {
    fs::read_to_string(BINDINGS).expect("bindings.ts missing: run `pnpm bindings`")
}

/// Extract every command name from `__TAURI_INVOKE("name")` calls.
fn commands_in_bindings(source: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut rest = source;
    while let Some(start) = rest.find("__TAURI_INVOKE(") {
        let after = &rest[start..];
        let Some(open) = after.find("(\"") else { break };
        let tail = &after[open + 2..];
        let Some(end) = tail.find('"') else { break };
        names.insert(tail[..end].to_owned());
        rest = &tail[end..];
    }
    names
}

fn advertised_commands() -> BTreeSet<String> {
    weasel_desktop_lib::feature_descriptors()
        .iter()
        .flat_map(|descriptor| descriptor.commands.iter().map(|name| (*name).to_owned()))
        .collect()
}

#[test]
fn bindings_and_descriptors_describe_the_same_commands() {
    let generated = commands_in_bindings(&bindings_source());
    assert!(!generated.is_empty(), "no commands found in bindings.ts");
    assert_eq!(
        generated,
        advertised_commands(),
        "run `pnpm bindings` or fix the feature descriptors"
    );
}

#[test]
fn every_command_is_owned_by_exactly_one_feature() {
    let mut seen = BTreeSet::new();
    for descriptor in weasel_desktop_lib::feature_descriptors() {
        for command in descriptor.commands {
            assert!(
                seen.insert(*command),
                "command {command:?} is advertised by more than one feature"
            );
        }
    }
}

#[test]
fn event_channel_is_generated_with_the_expected_name() {
    let source = bindings_source();
    assert!(source.contains("\"weasel-event\""), "event channel renamed");
    assert!(
        source.contains("weaselEvent: makeEvent"),
        "WeaselEvent is not exported as `weaselEvent`"
    );
}
