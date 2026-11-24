# Deploy the landing page from GitHub Actions

**Tier:** 2. A new deploy path and new third-party code (a GitHub Action and the `wrangler` npm package) running with a Cloudflare token. It changes none of the tier 3 items: the server, peers, wire format and hashing are untouched.
**Systems:** harness-files (teach-me, owns `.github/*`), dependencies (teach-me: the card covers crates, but its question, "what new code from outside does this bring in", applies to the action and `wrangler`).
**Why:** `landing/` is deployed by the Cloudflare Pages project `crabshare` through the Cloudflare GitHub app, which holds write access to this repository's code and settings. A compromise of that app, or of the Cloudflare account, could push code to `main`. Deploying from GitHub Actions with a Cloudflare API token limited to Pages reverses the direction: GitHub can deploy to Cloudflare, Cloudflare gets no access to GitHub.

## Decisions

### Which Pages project receives the deploys
- **Options:**
  - A. Keep `crabshare`. Turn off its automatic builds, deploy into it with `wrangler`, then uninstall the GitHub app. Cloudflare's docs say a Git-connected project "cannot switch to Direct Upload later", suggest disabling automatic deployments and deploying with Wrangler instead, and say removing GitHub access "will disable new builds". They don't say plainly that Wrangler deploys keep working once the app is gone, and the project stays labelled as Git-connected to a repo it can't reach.
  - B. Create a new Direct Upload project, `crabshare-landing`, deploy into it, then move its custom domains (`www.crabshare.com`, and `crabshare.com` if attached) to it and delete `crabshare`.
- **Chosen:** B. Every step is a documented path, and nothing is left half-connected. The cost is a few minutes in which the domains may not resolve while they move between projects, acceptable for a coming-soon page.
- **Still uncertain:** whether Cloudflare lets a domain be added to the new project before it is removed from the old one. The steps assume it must be removed first.

### How the workflow deploys
- **Options:** `cloudflare/wrangler-action`, or `npx wrangler pages deploy` in a plain `run:` step.
- **Chosen:** `cloudflare/wrangler-action` at v4.1.3, pinned to commit `953926a2e2182532811c01a25e53647d93bf07c0`, with `wranglerVersion: 4.158.0`. It is Cloudflare's own action and the one their Pages docs use. Pinning `wrangler` itself matters as much as pinning the action, because the action installs `wrangler` from npm at run time. 4.158.0 was published on 2026-11-05: old enough that a bad release would likely have been caught, recent enough to be supported.
- **Still uncertain:** none.

### Triggers, permissions and the GitHub deployment record
- **Chosen:**
  - Runs on push to `main` when `landing/**` or the workflow file changes, and on manual dispatch (needed for the first deploy and for re-deploys).
  - `permissions: contents: read` only. Cloudflare's example also grants `deployments: write` and passes `GITHUB_TOKEN`, so the action can record a GitHub "deployment". That record is a nice-to-have, so it is left out.
  - `actions/checkout` pinned to v7.0.1 (`3d3c42e5aac5ba805825da76410c181273ba90b1`) with `persist-credentials: false`, so the GitHub token isn't left in `.git/config` for later steps.
  - A `concurrency` group so two deploys never overlap; a newer push waits rather than cancelling one in flight.
  - Deploys with `--branch=main`, so it lands as production whatever branch the workflow was dispatched from, provided the project's production branch is `main`.
- **Still uncertain:** none.

### The Cloudflare token
- **Chosen:** a custom API token with one permission, Account > Cloudflare Pages > Edit, limited to the CrabShare account, stored as the repository secret `CLOUDFLARE_API_TOKEN`; the account ID goes in `CLOUDFLARE_ACCOUNT_ID`.
- **Still uncertain:** Cloudflare can't limit the token to one Pages project. Whoever holds it can deploy to, or delete, any Pages project in the account. Today that is only `crabshare-landing` (and the old `crabshare` until it is deleted).

### Keeping the pins current
- **Options:** add `.github/dependabot.yml` for the `github-actions` ecosystem, so Dependabot opens pull requests when the action or checkout release a new version; or leave it and update by hand.
- **Chosen:** leave it out of this change. Owner's call whether to add it later; `wranglerVersion` would still need updating by hand either way.

## How it could go wrong

**harness-files**
- The workflow runs on more events than it needs, for example on `pull_request` from forks, where secrets could be reached. Caught by the trigger list being push-to-`main` and manual dispatch only; the reviewer checks it.
- A pin drifts to a tag (`@v4`) that its owner can move. Caught by reviewing that both `uses:` lines are full SHAs.

**dependencies**
- A compromised `wrangler` or action release runs with the Pages token. Limited by the SHA and version pins (a new release can't arrive unreviewed) and by the token's scope (Pages only, no DNS, no other products, no GitHub access).
- The token leaks. It is only in GitHub's encrypted secrets, masked in logs, and can be rolled in Cloudflare without touching the workflow.

## Changes

- `.github/workflows/deploy-landing.yml` (new): the workflow above. One job, `deploy`, on `ubuntu-latest`: checkout, then `wrangler-action` with `command: pages deploy landing --project-name=crabshare-landing --branch=main`.
- `docs/DECISIONS.md`: D13 rewritten to the new decision: GitHub Actions deploys `landing/` to the Direct Upload project `crabshare-landing` with a Pages-only token; the Cloudflare GitHub app is rejected because it needs write access to the repository.

## Testing

- `actionlint` if available; otherwise a careful read. The workflow can only really be tested on GitHub.
- First real run: manual dispatch from the Actions tab, then check the Cloudflare dashboard shows a production deployment and `crabshare-landing.pages.dev` serves the page.
- After the domains move: `crabshare.com` and `www.crabshare.com` serve the page, and a small edit to `landing/` pushed to `main` deploys on its own.

## Owner steps

In the order given in chat with this spec: create the project, create the token, add the two secrets, commit and push, run the workflow, move the domains, delete `crabshare`, uninstall the Cloudflare GitHub app.
