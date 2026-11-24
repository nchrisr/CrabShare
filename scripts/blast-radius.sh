#!/bin/bash
# Lists the functions, classes and constants a change touched, and every other place that uses each
# one, as file:line. Breakage usually lives outside the diff, so reviewers get this list with it.
# Names only, no source: a reviewer reads the lines it needs.
#
# Usage: scripts/blast-radius.sh [base-ref]     (default base: where this branch left origin/HEAD)
# Covers committed and uncommitted changes since the base. A name counts as touched when its
# top-level definition changed (an unindented or exported line), or when a changed line sits inside
# it (from git's hunk headers). Nested helpers and class methods are left out.
# A use is a whole-word match in a file that also names the defining module (its import), so a
# same-named thing elsewhere isn't counted; those are summed on one line. A language pack may swap in
# its compiler's find-references for exact results.

set -u
limit="${BLAST_LIMIT:-15}"
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

# A definition: keyword then name, or `name = (` / `name = async (` for arrow functions.
def_re='(function|class|def|fn|func|interface|type|enum|struct|trait)[[:space:]]+\*?[[:space:]]*[A-Za-z_$][A-Za-z0-9_$]*|(const|let|var)[[:space:]]+[A-Za-z_$][A-Za-z0-9_$]*[[:space:]]*='
names_from() {
  grep -E '^(export[[:space:]]|pub[[:space:]]|async[[:space:]]|[A-Za-z])' | grep -oE "$def_re" | sed -E 's/^(function|class|def|fn|func|interface|type|enum|struct|trait|const|let|var)[[:space:]]+\*?[[:space:]]*//; s/[[:space:]]*=$//'
}

# The name other files use to import a file: its stem, or its folder's for index/mod/__init__.
module_of() {
  local stem
  stem=$(basename "$1"); stem="${stem%.*}"
  case "$stem" in index|mod|__init__|main|lib) stem=$(basename "$(dirname "$1")") ;; esac
  printf '%s' "$stem"
}

changed=$(git diff --name-only "$base" -- . ':(exclude)*.md' ':(exclude)*.lock' ':(exclude)*lock.json' \
  ':(exclude)*.lockb' ':(exclude)*.snap')
[ -n "$changed" ] || { echo "No code changed since $base."; exit 0; }

found=0
unused=""
while IFS= read -r file; do
  [ -f "$file" ] || continue
  diff=$(git diff -U0 "$base" -- "$file")
  names=$( {
    printf '%s\n' "$diff" | grep -E '^[+-][^+-]' | cut -c2- | names_from
    printf '%s\n' "$diff" | grep -E '^@@' | sed -E 's/^@@[^@]*@@ ?//' | names_from
  } | awk 'length($0) >= 3' | sort -u)
  while IFS= read -r name; do
    [ -n "$name" ] || continue
    def=$(grep -nE "(function|class|def|fn|func|interface|type|enum|struct|trait|const|let|var)[[:space:]]+\*?[[:space:]]*$name\b" "$file" | head -1 | cut -d: -f1)
    module=$(module_of "$file")
    all=$(git grep -n -w -F -e "$name" -- . ':(exclude)*.md' ":(exclude)$file" 2>/dev/null | cut -d: -f1,2)
    uses=""; other=0
    while IFS= read -r hit; do
      [ -n "$hit" ] || continue
      # The module's name as part of an import path or statement, not as a plain word.
      # `:` and `{` cover Rust paths such as `use crate::peer::{Peer, remove_peer};`.
      if grep -qE -e "[/'\".:]$module(['\"/.[:space:];:{]|\$)|import[[:space:]]+$module\b" "${hit%%:*}"; then uses="${uses}${hit}"$'\n'; else other=$((other + 1)); fi
    done <<< "$all"
    count=$(printf '%s' "$uses" | grep -c .)
    found=1
    note=""; [ "$other" -gt 0 ] && note="; $other same-named match(es) in files that don't import it"
    if [ "$count" -eq 0 ]; then
      unused="${unused:+$unused, }$name ($file)"
      continue
    fi
    printf '%s  (%s%s)  %d use(s) outside its file%s:\n' "$name" "$file" "${def:+:$def}" "$count" "$note"
    printf '%s' "$uses" | head -n "$limit" | sed 's/^/  /'
    [ "$count" -gt "$limit" ] && printf '  +%d more\n' $((count - limit))
  done <<< "$names"
done <<< "$changed"

[ -n "$unused" ] && printf 'No uses outside their own file: %s\n' "$unused"
[ "$found" -eq 1 ] || echo "No functions, classes or constants touched since $base."
exit 0
