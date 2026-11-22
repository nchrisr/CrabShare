# Specs

Plans for tier 2 and tier 3 changes, written with `/spec` and approved before any code. The tiers are in `AGENTS.md`. This folder holds work in progress only.

- Name files `YYYY-MM-DD_short-name.md`: the date, one underscore, then lowercase words joined by dashes. `tests/spec_file_names.rs` fails on any other name.
- A spec records the decisions at the time it was written. It isn't kept up to date afterwards, so if a spec and the code disagree, the code wins. A decision that outlasts the change also goes in `docs/DECISIONS.md`.
- Once a change has shipped, a later change copies its lasting decisions into `docs/DECISIONS.md`, naming the spec's file, and deletes the spec. Git history keeps it; `AGENTS.md` says how to read a deleted spec.
- State the reason in full, without pointing at anything outside the repo.
- Never commit a spec that describes an unfixed security hole. Keep it in chat until the fix ships.
