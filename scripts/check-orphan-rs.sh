#!/usr/bin/env bash
# SPDX-License-Identifier: LGPL-3.0-or-later
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

declare -A root_set=()

add_root() {
    local dir="$1"
    local rel
    rel="$(realpath --relative-to="$repo_root" "$dir")"
    if [ "$rel" = "." ]; then
        return
    fi
    root_set["$rel"]=1
}

# Workspace members from Cargo metadata.
while IFS= read -r manifest; do
    [ -z "$manifest" ] && continue
    add_root "$(dirname "$manifest")"
done < <(cargo metadata --no-deps --format-version 1 | jq -r '.packages[].manifest_path')

# Standalone Cargo.toml files, including non-workspace tools.
while IFS= read -r cargo_toml; do
    [ -z "$cargo_toml" ] && continue
    add_root "$(dirname "$cargo_toml")"
done < <(git ls-files '*Cargo.toml')

orphan_count=0
while IFS= read -r file; do
    [ -z "$file" ] && continue
    ok=0
    for root in "${!root_set[@]}"; do
        case "$file" in
            "$root"/*)
                ok=1
                break
                ;;
        esac
    done
    if [ "$ok" -eq 0 ]; then
        echo "orphan: $file"
        orphan_count=$((orphan_count + 1))
    fi
done < <(git ls-files '*.rs')

path_count=0
while IFS= read -r hit; do
    [ -z "$hit" ] && continue
    exempt=0
    if [ -f scripts/orphan-rs-allow.txt ]; then
        while IFS= read -r allow || [ -n "$allow" ]; do
            [ -z "$allow" ] && continue
            case "$allow" in \#*) continue ;; esac
            if [[ "$hit" == "$allow"* ]]; then
                exempt=1
                break
            fi
        done < scripts/orphan-rs-allow.txt
    fi
    if [ "$exempt" -eq 0 ]; then
        echo "path-attr: $hit"
        path_count=$((path_count + 1))
    fi
done < <(git grep -n -E '^\s*#\s*\[\s*path\s*=' -- '*.rs' || true)

if [ "$orphan_count" -ne 0 ] || [ "$path_count" -ne 0 ]; then
    echo "$orphan_count orphan file(s), $path_count #[path] attribute(s)"
    exit 1
fi

echo "orphan check: ok"
