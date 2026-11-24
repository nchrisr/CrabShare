# Work on `develop`, release to `main`

**Tier:** 3. It isn't one of the six listed items, but it rewrites a settled decision (D11), changes who can get code into the repository, and adds a workflow that runs code from outside pull requests on GitHub's machines. When unsure, the higher tier.
**Systems:** `harness-files` (teach-me): `.github/`, `scripts/`, `.claude/` and `AGENTS.md` all change.
**Why:** The project expects pull requests from outside contributors. Today every change goes straight to `main` (D11), and every push to `main` that touches `landing/` deploys the public site (D13). An outside change would have no automatic checks, and nothing separates "merged" from "live". GitHub's default branch is already `develop`, but `develop` is four commits behind `main` and nothing uses it.

## Decisions

### 1. Two branches: `develop` for work, `main` for releases
- **Options:** (a) keep D11 and take outside pull requests into `main`; (b) `develop` takes all work and pull requests, `main` takes only releases; (c) full git-flow with release and hotfix branches.
- **Chosen:** (b). Merging an outside change no longer deploys it, and the owner picks when the site changes. (c) adds branches nobody needs at one maintainer.
- **Still uncertain:** nothing.

### 2. The owner keeps pushing straight to `develop`
- **Options:** (a) the owner pushes to `develop` directly, through the pull-request ruleset's bypass; (b) the owner opens a pull request for every change and merges it through the bypass (bypass mode "pull request only").
- **Chosen:** (a). D11's reason still holds for the owner's own work: GitHub doesn't let an author approve their own pull request, so (b) adds a step without adding a reviewer. The local Stop hook already runs clippy and the tests, CI runs again on every push to `develop`, and a commit can't be released until its checks pass (decision 5).
- **Revisit:** as the repository grows (more contributors, or a second maintainer), move the owner's own changes to pull requests too, (b).

### 3. Rulesets: review rules the owner can bypass, history and check rules nobody can
- **Today:** `protect_main` targets `~DEFAULT_BRANCH` and `refs/heads/main`. Since the default branch became `develop`, it already covers `develop` too. Its admin bypass is "always", and a bypass skips every rule in its ruleset, including "no force push" and "no deletion".
- **Chosen:** four rulesets.
  - `protect_main`: target `refs/heads/main` only. "Restrict updates", so only the bypass list can change `main` at all: no outside pull request can be merged into it, even if approved. It keeps its pull-request rule (one approval, code-owner review, the extra approval for unattributed changes) as a second layer. Admin bypass "always".
  - `protect_develop` (new): target `refs/heads/develop`. The same pull-request settings as `protect_main`, plus: dismiss stale approvals on a new push, require approval of the most recent push, merge method squash only, and the `checks` status check required, with its source set to the GitHub Actions app, and the branch up to date with `develop`. Admin bypass "always".
  - `branch_history` (new): target `refs/heads/main` and `refs/heads/develop`. No deletion, no force push. No bypass actors.
  - `release_checks` (new): target `refs/heads/main`. The `checks` status check required, source GitHub Actions, without the up-to-date requirement. No bypass actors.
- **Why squash:** one commit per outside pull request, easy to read and to revert.
- **What the GitHub Actions source does and doesn't prove:** no other app or token can report `checks` as passed. But a pull request runs its own copy of the workflow, so one that edits the checks can make itself pass. A pull request touching `.github/`, `scripts/`, `tests/`, `build.rs`, `.cargo/`, `Cargo.toml` or `Cargo.lock` gets its diff read before its green check counts. `.github/CODEOWNERS` adds `tests/`, `Cargo.toml`, `Cargo.lock`, `*build.rs` and `.cargo/`, so code-owner review covers them.
- **Still uncertain:** a mistake on `develop` or `main` can no longer be undone with a force push. The owner would turn `branch_history` off for that one push, on purpose.

### 4. A check workflow on every pull request and every push to `develop` and `main`
- **Options:** (a) the same commands the Stop hook runs; (b) also a vulnerability scan (`scripts/deps-check.sh` uses OSV-Scanner).
- **Chosen:** (a) now: `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked`, and `python3 scripts/doc-refs-check.py`. (b) needs its own download and pin, so it's a later change.
- **Least privilege:**
  - Trigger `pull_request`, never `pull_request_target`. A pull request from a fork then runs with a read-only token and no secrets, even if it edits the workflow. The Cloudflare token is a secret of the `production` environment, which only `main` can use.
  - `permissions: contents: read` at the top, and nothing else.
  - `actions/checkout` pinned to the same commit as `deploy-landing.yml` (v7.0.1), with `persist-credentials: false`.
  - No third-party actions. The runner's own `rustup` installs the pinned Rust toolchain with clippy and rustfmt.
- **Rust toolchain pinned:** a new `rust-toolchain.toml` names one exact Rust version (the current stable when this is built) with clippy and rustfmt. CI and every developer's machine use the same compiler and lints. Moving to a newer Rust is an ordinary reviewed change to that file.
  - No cache. The build is small; a cache adds a way for one run to affect the next. Revisit if runs get slow.
  - No `paths` filter: a required check that doesn't run leaves a pull request waiting forever.
  - `timeout-minutes: 15`. A newer run cancels an older one only on the same pull request. Pushes are never cancelled, so every `develop` commit ends with a result and can be released.
- **Still uncertain:** `rustup` downloads the pinned toolchain the first time `cargo` runs on a machine that lacks it. That's a one-time, expected download from the Rust project.

### 5. A release is a pull request from `develop` to `main`, merged with a merge commit
- **Options:** (a) a pull request from `develop` to `main`, merged with a merge commit; (b) the same with squash or rebase; (c) the owner fast-forwards `main` to a checked `develop` commit and pushes.
- **Chosen:** (a). (c) works too, but leaves no visible record of each release. It's the owner's release workflow: the release is visible as a pull request, CI runs on it, and the owner merges it (through the bypass, since an author can't approve their own). `main` gains only merge commits, whose content matches `develop` at that point, so the next release merges cleanly. (b) rewrites `develop`'s commits into new ones on `main`, and the branches drift apart until releases conflict. `protect_main` allows the merge method "merge" only.
- **Enforced:** only the owner can update `main` (decision 3), and `release_checks` refuses the merge unless `checks` passed on the release pull request. That check can't be bypassed, so the deploy only ever sees checked content.
- **Rule:** `main` never gets a change that isn't on `develop` first. An urgent fix goes to `develop`, then is released.
- **Still uncertain:** tags or version numbers for releases aren't needed until the server is published (M8).

### 6. `develop` catches up with `main` by a fast-forward
- **Today:** `origin/develop` is at `8ce8af8`, an ancestor of `origin/main` (`77c8f00`), four commits behind and none ahead. The landing deploy change (D13's workflow) is on `main`.
- **Chosen:** land the landing deploy on `main` first, as D11 still allows, including retiring the old Git-connected Pages project and the Cloudflare GitHub app. Otherwise Pages would keep building previews of `develop`, and merged outside changes would be public at a preview address. Then fast-forward `develop` to `main`. No merge commit is needed, and from then on work starts from `develop`.
- **Still uncertain:** nothing.

### 7. Review scripts compare against the default branch
- **Options:** (a) change the default base from `origin/main` to `origin/develop`; (b) use `origin/HEAD` (the remote's default branch, `develop` here), and diff from where the current branch left it (`git merge-base origin/HEAD HEAD`).
- **Chosen:** (b). It follows GitHub's default branch without naming it, so the scripts stay the same as the harness's. The merge base stops changes that landed on `develop` after a branch was cut from showing up as that branch's own. If `origin/HEAD` isn't set, the script stops and says to run `git remote set-head origin --auto`, rather than guessing a branch.
- **Still uncertain:** this is a change to harness files. It should go upstream to the harness too, or the next harness upgrade undoes it.

### 8. Workflows count as code that runs on its own
- **Chosen:** `scripts/hooks-diff.sh` includes `.github/workflows/`, so the owner reads every workflow change word for word. The `harness-files` card gets tripwires for `pull_request_target`, `workflow_run`, a `permissions` entry set to `write`, `secrets.`, and a `uses:` not pinned to a full commit hash. Its "runs on its own" invariant is split: on a developer's machine, unchanged (no downloads, no network); in CI, the least-privilege list in decision 4, which allows `rustup` and crates.io downloads on GitHub's machine.
- **Still uncertain:** nothing.

### 9. Telling contributors where to aim
- **Chosen:** `develop` as the default branch already makes new pull requests target it. A short `CONTRIBUTING.md` says so, names the checks, and says that `main` takes releases only. A pull request opened against `main` is retargeted to `develop` by the owner.

## How it could go wrong

`harness-files` (teach-me):
1. **A fork's pull request does something harmful on the runner,** through an edited `ci.yml`, a test or a `build.rs`. It runs with a read-only token, no secrets, and on GitHub's machine, not the owner's. GitHub holds the run until the owner approves it (owner step 8), and the owner reads the diff first. Caught by: the `pull_request` trigger, the `production` environment's branch rule, and the card's new tripwires.
2. **A pull request makes its own checks pass** by weakening them. Caught by: code-owner review on every check-related path, and the rule that such a diff is read before its green check counts.
3. **The required check never reports**, because its job name changes or a filter skips it, so every outside pull request waits forever and nothing can be released. Caught by: the job is named `checks` in both the workflow and the rulesets, and the workflow has no `paths` filter.

## Changes

- **`.github/workflows/ci.yml`** (new). Name `CI`. Triggers: `pull_request` to `develop` or `main`, `push` to `develop` or `main`. One job, `checks`, on `ubuntu-latest`, steps: check out (pinned, no credentials), install stable Rust with clippy and rustfmt through `rustup`, then the four commands in decision 4. Settings as listed there.
- **`rust-toolchain.toml`** (new): `channel` set to one exact version, `components = ["clippy", "rustfmt"]`, `profile = "minimal"`. Added to `.github/CODEOWNERS` with the rest.
- **`.github/CODEOWNERS`:** adds `tests/`, `Cargo.toml`, `Cargo.lock`, `*build.rs` and `.cargo/`.
- **`docs/DECISIONS.md`:** D11 rewritten as "Work goes to `develop`; `main` takes releases - settled": decisions 1, 2, 3 and 5, the rulesets, and the rejected options (pull requests straight into `main`; a pull request per owner change; squash or fast-forward releases; git-flow). D11's ruleset paragraph is replaced. D13 gains one line: a change reaches `main` only through a checked release pull request, so only a release deploys. Both name this spec.
- **`AGENTS.md`:** the "Changes" line becomes "The owner pushes to `develop`; outside contributors open pull requests to `develop`. `main` takes releases only, through a pull request from `develop` merged with a merge commit (D11). Agents never commit or push." The "Test" section says CI runs the same checks on every pull request and every push to `develop` and `main`. "Where things live" adds `.github/workflows/`.
- **`scripts/systems-touched.sh`, `scripts/blast-radius.sh`, `scripts/hooks-diff.sh`, `scripts/deps-check.sh`:** default base per decision 7. Usage comments and the D11 comment in `systems-touched.sh` updated. An explicit base argument still wins. `hooks-diff.sh` also adds `.github/workflows/` to its list of files whose content runs.
- **`.claude/commands/review-loop.md`:** the base is the merge base with `origin/HEAD` unless told otherwise; the "no branch or PR" sentence goes.
- **`.claude/agents/reviewer.md`:** "usually `origin/main`" becomes "usually the merge base with `origin/HEAD`".
- **`docs/systems/harness-files.md`:** the new tripwires and split invariant from decision 8; the rule that a green check on a check-related pull request counts only after its diff is read; and the "How it breaks" line about no ruleset replaced by the ruleset setup.
- **`.gitignore`:** `.DS_Store` is ignored in every folder, not just the top one (the owner's request during review).
- **`.claude/hooks/check-on-stop.sh`, `.claude/hooks/format-on-edit.sh`:** set `RUSTUP_AUTO_INSTALL=0`, so a hook never downloads the pinned Rust. If it isn't installed, the Stop hook skips clippy and the tests and tells the developer to run `rustup toolchain install` (the owner's decision during review).
- **`CONTRIBUTING.md`** (new): target `develop`, the checks to run, `main` is releases only.
- **`README.md`:** "Development" lists the exact commands CI runs (`cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked`) and says CI runs them on every pull request; "Project docs" links `CONTRIBUTING.md`. The rest is checked against the repo as it is when built, and anything out of date is fixed.

## Testing

- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` and `python3 scripts/doc-refs-check.py` pass locally.
- `scripts/systems-touched.sh` with no argument, from this worktree, lists `harness-files` with the new tripwires; with `origin/HEAD` unset it stops with the `set-head` message. `scripts/hooks-diff.sh` shows `ci.yml` as new.
- The workflow's syntax is only proven on GitHub: the first push to `develop` must show a green `CI / checks` run.
- A test pull request from a branch to `develop` shows `checks` as required.
- The first release proves `release_checks`: the release pull request can't be merged until `checks` passes on it.
- Can't be checked here: the rulesets and Actions settings, which only the owner can change, and a pull request from a fork, which needs a second account.

## Owner steps

In order, one at a time. The commands will be given in chat when it's time.

1. Finish the landing deploy on `main`, as planned (current D11), including deleting the old Pages project and uninstalling the Cloudflare GitHub app. Check: the Cloudflare tokens are listed under the `production` environment's secrets, not the repository's.
2. Fast-forward `develop` to `main` and push it. Check: `git rev-list --count origin/main..origin/develop` and the reverse both print 0.
3. In the main checkout, switch to `develop`, and run `git remote set-head origin --auto`.
4. Commit this change to `develop` and push. Check: Actions shows `CI` green on `develop`.
5. Create `protect_develop` as in decision 3, choosing `checks` from GitHub Actions.
6. Create `branch_history` and `release_checks`, no bypass actors.
7. Edit `protect_main`: target `main` only, and remove its deletion and force-push rules (now in `branch_history`).
8. Settings > Actions > General: "Require approval for all external contributors" for fork pull request workflows; default workflow permissions read-only; "Allow GitHub Actions to create and approve pull requests" off.
9. Release: open a pull request from `develop` to `main`, wait for `checks` to pass, and merge it with a merge commit. Check: `CI` is green on `main`; the landing deploy runs only if `landing/` changed.
