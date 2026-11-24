#!/bin/bash
# Shows, word for word, every command that runs on its own in this project, before and after a
# change: agent hooks, their scripts, git hooks, and Cargo build scripts. A reviewer pastes the
# output into the owner's briefing unchanged, so the owner reads the commands themselves, not a
# description of them.
#
# Usage: scripts/hooks-diff.sh [base-ref]     (default base: where this branch left origin/HEAD)
# Prints a unified diff, or "No changes to anything that runs on its own." Needs jq.

set -u
root=$(git rev-parse --show-toplevel) || exit 1
cd "$root" || exit 1

# Without a base argument, compare against where this branch left the remote's default branch
# (develop, D11), so changes that landed there since don't show up as this branch's own.
if [ -n "${1:-}" ]; then
  base="$1"
else
  default_branch=$(git rev-parse --abbrev-ref --verify --quiet origin/HEAD) || {
    echo "origin/HEAD isn't set, so the default branch is unknown. Run: git remote set-head origin --auto" >&2
    exit 1
  }
  base=$(git merge-base "$default_branch" HEAD) || exit 1
fi

# Files whose whole content runs: hook scripts, git hooks, what Cargo runs at build time, and CI
# workflows, which run on GitHub's machines.
script_pattern='^(\.claude/hooks/|\.codex/hooks/|\.husky/|\.githooks/|\.cargo/|\.github/workflows/)|^\.pre-commit-config\.ya?ml$|(^|/)build\.rs$'
# Files whose hook entries run: agent settings.
settings_files='.claude/settings.json .codex/config.json'

# Prints a file's content at the base ref, or in the working tree when ref is "WORKTREE".
read_at() {
  if [ "$1" = "WORKTREE" ]; then
    cat "$2" 2>/dev/null
  else
    git show "$1:$2" 2>/dev/null
  fi
}

# Prints, one per line, every command that runs on its own, as of the given ref.
collect() {
  ref="$1"
  for file in $settings_files; do
    content=$(read_at "$ref" "$file")
    [ -n "$content" ] || continue
    printf '%s' "$content" | jq -r --arg f "$file" '
      (.hooks // {}) | to_entries[] | .key as $event
      | .value[] | (.matcher // "*") as $matcher
      | (.hooks // [])[] | "\($f) hook \($event) [\($matcher)]: \(.command // .prompt // "?")"' 2>/dev/null
  done

  if [ "$ref" = "WORKTREE" ]; then
    paths=$(git ls-files -co --exclude-standard | grep -E "$script_pattern")
  else
    paths=$(git ls-tree -r --name-only "$ref" | grep -E "$script_pattern")
  fi
  for path in $paths; do
    printf '=== %s\n' "$path"
    read_at "$ref" "$path"
    printf '\n'
  done
  # A hooks folder set in git's own config would run outside every list above.
  hooks_path=$(git config --get core.hooksPath)
  if [ -n "$hooks_path" ]; then
    printf 'git config core.hooksPath: %s\n' "$hooks_path"
  fi
}

before=$(collect "$base")
after=$(collect WORKTREE)
if [ "$before" = "$after" ]; then
  echo "No changes to anything that runs on its own."
  exit 0
fi
diff -u --label "before ($base)" --label "after (this change)" <(printf '%s\n' "$before") <(printf '%s\n' "$after")
exit 0
