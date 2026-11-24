---
description: Review a finished tier 2 or 3 change with a general reviewer, one system reviewer per touched card, and a separate fixer; then brief the owner by the dial
argument-hint: <spec path, if not obvious>
---

Run the review loop for the change since this branch left the default branch, committed or not. Spec: $ARGUMENTS

Tier 2 and 3 only. Tier 1 changes skip this. Run it after the change is built and its checks pass, and before the demo and the commit. The base is the commit where this branch left the default branch, `git merge-base origin/HEAD HEAD`, unless told otherwise (D11).

**The point is that the author doesn't judge their own work.** Pass each agent only what's listed below. Don't explain what you built or why, don't filter the findings, and don't add your view of them.

1. **Find the systems.** Run `scripts/systems-touched.sh <base>`. Each line is a card, its setting for this change, and why it woke. Cards set to `trust` need no reviewer. Then run `scripts/blast-radius.sh <base>`: the functions the change touched and every other file that uses them. Keep its output unchanged.

2. **Review, in parallel.** Start, in one message:
   - the `reviewer` subagent with only: the base, that the change is everything since `<base>` committed or not, the spec path, the blast-radius output, and that findings come back in its numbered format;
   - one `system-reviewer` subagent per touched card set to `teach-me` or `keep-me-posted`, with only: the base, the spec path, the card path, its setting, why it woke, and the blast-radius output.

   If `harness-files` is touched, also run `scripts/hooks-diff.sh <base>` yourself and keep its output unchanged for step 6; pass it to that card's system reviewer too. If `dependencies` is touched, do the same with `scripts/deps-check.sh <base>`.

   Merge every numbered finding into one list, renumbered, each keeping its wording and `[card]` tag. Add, as findings too, any problem a system reviewer noted outside its own card, tagged `[noticed by <card>]`, word for word. Where several findings describe the same problem in the same place, keep one, with every reviewer's tag, and note how many reviewers raised it; count it once. Keep the system reviewers' briefings aside for step 6. If there are no findings, skip to step 6.

3. **Fix.** First stage the author's work with `git add -A`, so that whatever changes after this point is the fixer's. Say that you staged it. Then start the `fixer` subagent with only the base, the spec path and the merged findings, word for word.

4. **Recheck, once.** If the fixer changed any file, start a new `reviewer` in recheck mode with the spec path and the fixer's changes: plain `git diff` (unstaged changes only) of the files it listed, plus the full contents of any file still untracked. Anything this raises goes to the owner as flagged. Never run a third round.

5. **Confirm.** Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` yourself. If any fail, say so with the output. Don't change the fixer's work to make them pass; flag it instead.

6. **Tell the owner, by the dial,** in plain language:
   - **teach-me** cards: each system reviewer's briefing, one system at a time, checking it makes sense before the next. For `harness-files`, show the hooks diff word for word first, before any summary; for `dependencies`, the deps-check output.
   - the counts: findings, fixed, rejected, flagged; then each flagged item as a question, one at a time, with what happens either way.
   - **keep-me-posted** cards: their paragraphs go in the summary below, not in chat.

   An answer that needs a code change is made here, by you, as an ordinary edit, and noted. Report any security hole the change doesn't fix in chat only.

7. **Suggest two commits** for the owner to make, after the demo: first what's staged (the change itself, `git commit` without `-a`), then the fixer's changes and owner-decided edits as "Address review findings". Agents don't commit.

8. **Write the summary** in this shape, and add it to the session log in `docs/STATUS.md` if that file exists, or give it in chat:

   ```markdown
   ## Review loop

   Systems: <card> (<setting>, <why>), ...
   Reviewer findings: <n> (+<m> from the recheck). Fixed <n>, rejected <n>, flagged <n>.

   **Found**
   - <finding, one line> (`file`) [card]

   **Fixed**
   - <finding> (`file`): <how>. Test: <test name>

   **Rejected**
   - <finding> (`file`): <why>

   **Flagged, and what the owner decided**
   - <question>: <answer>

   **Keep me posted**
   - <card>: <paragraph>
   ```

   Leave out any heading with nothing under it. Never include a security hole this change doesn't fix.

9. **After it's committed,** the spec has done its job. In the next change in this area (or a small cleanup), copy any decision that outlasts the change into `docs/DECISIONS.md`, with the spec's file name, and delete the spec. Keep the folder to work in progress.
