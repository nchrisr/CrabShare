---
description: Write a decision-based spec for a tier 2 or 3 change, then stop before any code
argument-hint: <what to build, or a milestone such as M2>
---

Write a spec for: $ARGUMENTS

Don't edit code.

1. **Understand it.** Read the milestone in `docs/ROADMAP.md` if one is named, the settled entries in `docs/DECISIONS.md`, and the code involved. Check assumptions against the running server where you can, not just the files.
2. **Pick the tier** using the table in `AGENTS.md`, and say why. A tier 1 change doesn't need a spec, so say so and stop.
3. **Write the spec** in this shape, kept short:

   ```markdown
   # <Title>

   **Tier:** 2 or 3, and why
   **Why:** the problem and who it affects, stated in full

   ## Decisions
   ### <Decision>
   - **Options:** what was considered
   - **Chosen:** which, and why
   - **Still uncertain:** what we don't know yet

   ## Changes
   Each file, and what changes in it: behaviour, types and function signatures. No implementation bodies.

   ## Testing
   What proves it works (unit tests, a `websocat` or two-tab check), and what can't be checked here.

   ## Owner steps
   Anything the owner has to do, in order.
   ```

4. **Tier 3 only:** run the `spec-critic` subagent on it. Check its findings, fix the spec for the ones that hold up, and report which you rejected and why.
5. **Save it** as `docs/specs/YYYY-MM-DD-short-name.md`, following `docs/specs/README.md`. If it describes an unfixed security hole, don't save it: keep it in chat.
6. **Stop.** Walk the owner through it one decision at a time, waiting for a go-ahead after each, and wait for approval before any code is written.
