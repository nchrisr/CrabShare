#!/bin/bash
# Formats a Rust file right after an agent edits it. Never fails the edit: any problem is ignored,
# and the Stop hook and `cargo fmt --check` catch what's left.

input=$(cat)
file=$(printf '%s' "$input" | jq -r '.tool_input.file_path // empty' 2>/dev/null)

case "$file" in
  *.rs) ;;
  *) exit 0 ;;
esac

cd "$CLAUDE_PROJECT_DIR" 2>/dev/null || exit 0
# rustfmt reads rustfmt.toml from the project; the edition matches Cargo.toml.
rustfmt --edition 2024 "$file" >/dev/null 2>&1
exit 0
