#!/bin/bash
# Runs the fast checks when a turn ends with Rust code changed. A failure exits 2, which makes the
# agent fix it before finishing. It runs at the end of a turn rather than after each edit, because a
# multi-file change is often broken halfway through.

input=$(cat)

# Already continuing because of this hook: stop here so an unfixable failure can't loop forever.
if [ "$(printf '%s' "$input" | jq -r '.stop_hook_active // false')" = "true" ]; then
  exit 0
fi

cd "$CLAUDE_PROJECT_DIR" || exit 0

# Without a Rust toolchain there is nothing to run.
if ! command -v cargo >/dev/null 2>&1; then
  exit 0
fi

changed=$(
  git diff --name-only HEAD 2>/dev/null
  git ls-files --others --exclude-standard 2>/dev/null
)
# Docs can break from either side: a renamed function breaks a doc, and so does a doc edit. Fast, so
# it runs on any change. Warnings don't stop the turn; broken references do.
if [ -n "$changed" ] && [ -f scripts/doc-refs-check.py ]; then
  if ! output=$(python3 scripts/doc-refs-check.py 2>&1); then
    printf 'A doc points at code that has moved or gone. Fix the doc before finishing:\n%s\n' \
      "$output" | tail -40 >&2
    exit 2
  fi
fi

# AGENTS.md is read on every turn of every session, so it has a size budget. Checked only when this
# turn changed it, so it never blocks unrelated work.
budget="${AGENTS_MD_BUDGET:-250}"
if printf '%s\n' "$changed" | grep -qx 'AGENTS.md' && [ "$(wc -l < AGENTS.md)" -gt "$budget" ]; then
  printf 'AGENTS.md is %s lines, over its %s-line budget. Move detail into docs/ and leave a\n' \
    "$(wc -l < AGENTS.md | tr -d ' ')" "$budget" >&2
  printf 'one-line pointer saying when to read it.\n' >&2
  exit 2
fi

# Docs-only turns skip the Rust checks.
if ! printf '%s\n' "$changed" | grep -qE '\.(rs|toml)$'; then
  exit 0
fi

if ! output=$(cargo clippy --locked --all-targets --quiet -- -D warnings 2>&1); then
  printf 'Clippy failed. Fix this before finishing:\n%s\n' "$output" | tail -60 >&2
  exit 2
fi

if ! output=$(cargo test --locked --quiet 2>&1); then
  printf 'Tests failed. Fix this before finishing:\n%s\n' "$output" | tail -60 >&2
  exit 2
fi

exit 0
