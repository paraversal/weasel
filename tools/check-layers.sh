#!/usr/bin/env bash
# Enforce crate layering. Fails if a shipped dependency points the wrong way.
#
# Only [dependencies] and [build-dependencies] are checked: [dev-dependencies] exist
# for tests and never ship, so e.g. weasel-core may use tokio to drive its tests.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

status=0

# Print the dependency lines of the shipped dependency sections of a manifest.
shipped_deps() {
    awk '
        /^\[/ {
            section = ($0 ~ /^\[(dependencies|build-dependencies)(\.|\])/)
            next
        }
        section { print }
    ' "$1"
}

check() {
    local crate="$1"
    shift
    local manifest="crates/$crate/Cargo.toml"
    [[ -f "$manifest" ]] || return 0
    local deps
    deps="$(shipped_deps "$manifest")"
    local dep
    for dep in "$@"; do
        if grep -Eq "^[[:space:]]*${dep}[[:space:]]*=" <<<"$deps"; then
            echo "layer violation: crates/$crate must not depend on $dep" >&2
            status=1
        fi
    done
}

check weasel-core tauri sqlx calamine specta tokio weasel-store weasel-import weasel-platform
check weasel-import tauri sqlx weasel-store weasel-platform
check weasel-store tauri specta weasel-import weasel-platform
check weasel-platform tauri sqlx weasel-store weasel-import

if [[ $status -eq 0 ]]; then
    echo "layers ok"
fi
exit "$status"
