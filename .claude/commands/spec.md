---
description: Write a decision-based spec for a tier 2 or 3 change, then stop before any code
argument-hint: <what to build, or a milestone such as M2>
---

Write a spec for: $ARGUMENTS

Don't edit code.

1. **Understand it.** Read the milestone in `docs/ROADMAP.md` if one is named, the settled entries in `docs/DECISIONS.md`, and the code involved. A decision that names a deleted spec can be read from git history (see `AGENTS.md`). Check assumptions against the running server where you can, not just the files.
2. **Pick the tier** using the table in `AGENTS.md`, and say why. A tier 1 change doesn't need a spec, so say so and stop.
3. **Name the systems.** Which cards in `docs/systems/` does this touch, and what is each set to? A change that trips a tripwire counts as teach-me for that card.
4. **Write the spec** in this shape, kept short:

   ```markdown
   # <Title>

   **Tier:** 2 or 3, and why
   **Systems:** <card> (<setting>), ...
   **Why:** the problem and who it affects, stated in full

   ## Decisions
   ### <Decision>
   - **Options:** what was considered
   - **Chosen:** which, and why
   - **Still uncertain:** what we don't know yet

   ## How it could go wrong
   For each teach-me system: the two or three realistic failures, and what catches each.

   ## Changes
   Each file, and what changes in it: behaviour, types and function signatures. No implementation bodies.

   ## Testing
   What proves it works (unit tests, a `websocat` or two-tab check), and what can't be checked here.

   ## Owner steps
   Anything the owner has to do, in order.
   ```

5. **Tier 3 only:** run the `spec-critic` subagent on it. Check its findings, fix the spec for the ones that hold up, and report which you rejected and why.
6. **Save it** as `docs/specs/YYYY-MM-DD_short-name.md`, following `docs/specs/README.md`. If it describes an unfixed security hole, don't save it: keep it in chat.
7. **Stop.** Walk the owner through it one decision at a time, waiting for a go-ahead after each, and wait for approval before any code is written. After it's built, run `/review-loop`.
