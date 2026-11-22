#!/bin/bash
# Lists the system cards a change touches, so the review loop knows which system reviewers to wake.
#
# Usage: scripts/systems-touched.sh [base-ref]     (default base: origin/main, the last push)
# Output, one line per touched card:  <card path><TAB><involvement><TAB><why>
# A card is touched when a changed path matches one of its `owns` globs, or when an added line
# matches one of its `watch` patterns (a tripwire, which raises it to teach-me).
# Covers committed, uncommitted and untracked changes since the base.

set -u
# Changes are committed straight to main (D11), so origin/main covers local commits not yet pushed.
base="${1:-origin/main}"
cards_dir="${CARDS_DIR:-docs/systems}"
root=$(git rev-parse --show-toplevel) || exit 1
cd "$root" || exit 1

changed=$( { git diff --name-only "$base"; git ls-files --others --exclude-standard; } | sort -u)
# Tripwires look at code, not prose: Markdown is left out, so the cards don't trip themselves.
added=$( { git diff "$base" -- . ':(exclude)*.md' | grep -E '^\+[^+]'; \
  git ls-files --others --exclude-standard -z -- . ':(exclude)*.md' | xargs -0 cat 2>/dev/null; } )

# Reads one list from a card's frontmatter: the lines "  - ..." under "<key>:". Write each item in
# single quotes, so a regex's backslashes are taken literally.
read_list() {
  awk -v key="$1:" '
    NR == 1 && $0 == "---" { inside = 1; next }
    inside && $0 == "---" { exit }
    inside && $0 == key { grab = 1; next }
    inside && grab && /^  - / { sub(/^  - /, ""); gsub(/^[\047"]|[\047"]$/, ""); print; next }
    inside && grab { grab = 0 }
  ' "$2"
}

for card in "$cards_dir"/*.md; do
  [ -f "$card" ] || continue
  involvement=$(awk '/^involvement:/ { print $2; exit }' "$card")
  [ -n "$involvement" ] || continue
  why=""

  while IFS= read -r pattern; do
    [ -n "$pattern" ] || continue
    while IFS= read -r file; do
      [ -n "$file" ] || continue
      # In [[ == ]], * matches across /, so "app/*/actions.ts" covers nested folders too.
      if [[ "$file" == $pattern ]]; then
        why="owns $file"
        break 2
      fi
    done <<< "$changed"
  done < <(read_list owns "$card")

  while IFS= read -r pattern; do
    [ -n "$pattern" ] || continue
    if printf '%s\n' "$added" | grep -qE -- "$pattern"; then
      involvement="teach-me"
      why="${why:+$why; }tripwire /$pattern/"
      break
    fi
  done < <(read_list watch "$card")

  [ -n "$why" ] && printf '%s\t%s\t%s\n' "$card" "$involvement" "$why"
done
exit 0
