---
name: dependencies
title: Dependencies
involvement: teach-me
owns:
  - 'Cargo.toml'
  - '*/Cargo.toml'
  - 'Cargo.lock'
watch:
  - '^\+?[A-Za-z0-9_-]+ *= *\{[^}]*\b(git|registry) *='
  - '^\+?\[(patch|replace)'
---

# Dependencies

**Involvement:** teach-me. Every crate is code written by strangers. A crate's `build.rs` and procedural macros run on the machine that builds it, and can't be switched off. A known-vulnerable version, a look-alike name or a hijacked update can reach the server, its users or a developer's machine.
**The question this card asks of every change:** what new code from outside does this change bring in, and do we trust it?

## Invariants (always true)
- `Cargo.lock` is committed, and checks and builds use `--locked`, which refuses to change it.
- No change is committed with a known high or critical vulnerability in a crate it adds or upgrades, unless the owner accepts it in writing, with the reason and a date to revisit.
- Crates come from crates.io. A git or other-registry source, or a `[patch]` override, needs the owner's OK.
- Every newly added crate passes the stranger check below.

## The stranger check (for each newly added crate)
`scripts/deps-check.sh` lists the new crates. For each, answer and report:
- **Name:** spelled exactly like the crate you meant?
- **Age:** first published under 90 days ago is a flag.
- **Use:** very low downloads or few dependents is a flag.
- **Who:** the owners; a recent change of owner is a flag.
- **Build-time code:** does it, or anything it pulls in, have a `build.rs` or a procedural macro?
- **Need:** could the standard library or an existing dependency do this instead?

Where to look: `https://crates.io/api/v1/crates/<crate>`. Any flag goes to the owner as a question. None of this proves a crate is safe; it catches the common attacks.

## Tripwires (changes that pull you in, even from a trusted area)
- A dependency from a git repository or another registry.
- A `[patch]` or `[replace]` section.

## How it breaks
- No incidents yet.

## Checks
- **Programs:** `scripts/deps-check.sh <base>` lists new and changed crates and runs OSV-Scanner on `Cargo.lock`. Dependabot alerts and security updates watch weekly on GitHub.
- **Reviewer:** the stranger check for each new crate; for each upgrade, is it a major version, and does its changelog mention security or breaking changes?

## Briefing
The deps-check output unchanged, then the stranger check per new crate in one line each, and any vulnerability with its severity and fix version.
