#!/bin/bash
# Shows what outside code a change brings in, and whether any dependency has a known vulnerability.
#   1. Direct dependencies added, removed or changed since the base, per manifest.
#   2. Every locked package added or changed since the base (the full tree, from the lock files).
#   3. A known-vulnerability scan of the lock files with OSV-Scanner (https://google.github.io/osv-scanner/).
# The reviewer pastes the output into the owner's briefing unchanged, then runs the stranger check
# (docs/systems/dependencies.md) on each newly added package.
#
# Usage: scripts/deps-check.sh [base-ref]     (default base: where this branch left origin/HEAD)
# Needs jq for npm projects. Reaches the network only for the vulnerability scan.

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

# Prints a file at the base ref, or in the working tree when ref is "WORKTREE".
read_at() {
  if [ "$1" = "WORKTREE" ]; then
    cat "$2" 2>/dev/null
  else
    git show "$1:$2" 2>/dev/null
  fi
}

tracked_and_new() {
  git ls-files -co --exclude-standard | grep -E "$1" | grep -v '/node_modules/'
}

# "name version" for each direct dependency in a manifest.
direct_deps() {
  case "$2" in
    *package.json)
      read_at "$1" "$2" | jq -r '
        [(.dependencies // {}), (.devDependencies // {}), (.optionalDependencies // {})]
        | add // {} | to_entries[] | "\(.key) \(.value)"' 2>/dev/null ;;
    *Cargo.toml)
      read_at "$1" "$2" | awk '
        /^\[/ { inside = ($0 ~ /dependencies\]$/) ; next }
        inside && /^[A-Za-z0-9_-]+ *=/ { split($0, a, "="); gsub(/ /, "", a[1]); v = $0; sub(/^[^=]*= */, "", v); print a[1], v }' ;;
    *requirements*.txt)
      read_at "$1" "$2" | grep -vE '^\s*(#|-|$)' | sed 's/[[:space:]]*#.*//' ;;
    *pyproject.toml)
      read_at "$1" "$2" | awk '
        /^dependencies *= *\[/ { inside = 1; next }
        inside && /^\]/ { inside = 0 }
        inside { gsub(/[",]/, ""); gsub(/^ +| +$/, ""); if ($0 != "") print }' ;;
  esac
}

# "name@version" for every package in a lock file.
locked_packages() {
  case "$2" in
    *package-lock.json)
      read_at "$1" "$2" | jq -r '
        (.packages // {}) | to_entries[] | select(.key != "")
        | "\(.key | sub(".*node_modules/"; ""))@\(.value.version)"' 2>/dev/null ;;
    *Cargo.lock|*uv.lock|*poetry.lock)
      read_at "$1" "$2" | awk '
        /^name = / { gsub(/"/, "", $3); name = $3 }
        /^version = / { gsub(/"/, "", $3); if (name != "") print name "@" $3; name = "" }' ;;
    *pnpm-lock.yaml)
      read_at "$1" "$2" | grep -E "^  '?/?[^ ]+@[0-9][^:]*'?:$" | sed -E "s/^  '?\/?//; s/'?:$//" ;;
  esac
}

compare() {
  diff <(printf '%s\n' "$1" | sort -u) <(printf '%s\n' "$2" | sort -u) | grep -E '^[<>]' |
    sed -e 's/^< /  - removed: /' -e 's/^> /  + added or changed: /'
}

echo "== Direct dependencies (since $base)"
found=0
for manifest in $(tracked_and_new '(^|/)(package\.json|Cargo\.toml|pyproject\.toml|requirements[^/]*\.txt)$'); do
  changes=$(compare "$(direct_deps "$base" "$manifest")" "$(direct_deps WORKTREE "$manifest")")
  if [ -n "$changes" ]; then
    found=1
    echo "$manifest"
    echo "$changes"
  fi
done
[ "$found" = 1 ] || echo "  no changes"

echo
echo "== Locked packages, full tree (since $base)"
found=0
for lockfile in $(tracked_and_new '(^|/)(package-lock\.json|pnpm-lock\.yaml|Cargo\.lock|uv\.lock|poetry\.lock)$'); do
  changes=$(compare "$(locked_packages "$base" "$lockfile")" "$(locked_packages WORKTREE "$lockfile")")
  if [ -n "$changes" ]; then
    found=1
    count=$(printf '%s\n' "$changes" | grep -c 'added or changed')
    echo "$lockfile ($count added or changed)"
    echo "$changes" | head -60
  fi
done
[ "$found" = 1 ] || echo "  no changes"

echo
echo "== Known vulnerabilities (OSV-Scanner)"
if ! command -v osv-scanner >/dev/null 2>&1; then
  echo "  NOT RUN: osv-scanner isn't installed. Install it (macOS: brew install osv-scanner;"
  echo "  others: https://google.github.io/osv-scanner/installation/) and run this again."
  exit 0
fi
if osv-scanner scan --help >/dev/null 2>&1; then
  osv-scanner scan source --recursive . 2>&1 | tail -80
else
  osv-scanner --recursive . 2>&1 | tail -80
fi
exit 0
