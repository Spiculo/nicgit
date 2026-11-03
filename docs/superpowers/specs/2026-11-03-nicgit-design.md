# NicGit — design

**Date:** 2026-11-03 · **Status:** v2, owner answers folded in, for review · **Engine:** KimBim (to be renamed NicBim) in `../kimcad`
**Related:** [decisions](../../decisions.md) · [future work](../../future-work.md)

NicGit is the forge for NicBim projects: a git server plus a web app where architects, consultants and clients **see, compare, review and combine** versions of a building model in the browser. It is a product, not a proof of concept. It runs **mainly as a hosted service operated by the owner**, and the same build can be **deployed on premises**. It is the app, not a marketing site: after login you see your projects.

NicGit is central to the business. Revenue comes from **Actions budgets, package hosting and AI services** (through the owner's cloud AI router, integrated into NicGit and NicBim). Certification programmes exist but are out of scope here. So metering, tenant isolation and data protection are part of the core design, not later additions.

---

## 1. Intent and why

NicBim already stores every project as a standard git repository of canonical IFC records: one JSON blob per rooted IFC entity, design options = branches, milestones = tags, project settings = `settings.json` in each version's tree (kimcad `docs/02`, `docs/03`). Any git host can *store* that, but none can *show* it:

- Generic forges show `objects/3f/a9/0f3a….json` changed, not "12 walls moved, 3 doors changed fire rating, the permit export settings changed".
- Their **merge button runs git's text merge on records**, which can silently corrupt a model. NicBim's record merge (`kimbim.merge`) must be the only merge.
- People who never install NicBim (clients, consultants, contractors, authorities) cannot look at a model, a version or a proposal.

NicGit is **GitHub's workflow rebuilt around buildings.** Every place GitHub shows a text diff, NicGit shows a 3D model, an element list, a property table and a settings table.

## 2. Goals

1. **Git hosting that works with the app today:** `kimbim versions push/pull` (and plain `git`) to `https://<host>/<account>/<project>.git` with token auth, with no app changes.
2. **See any version in the browser:** 3D viewer, levels, element tree, properties, project info and settings.
3. **Compare any two versions or options:** semantic diff by class, level, element and property (old → new), **project settings diff**, and a coloured 3D overlay.
4. **Proposals** (PRs) between design options: visual diff, merge preview, **conflict policy** (§9), role-based approvals, comments pinned to elements and viewpoints, checks, and **Combine**, which runs the NicBim merge on the server.
5. **Element history** ("blame").
6. **Issues as BCF:** import and export `.bcfzip`.
7. **Milestones and phases** (Concept → Schematic → DD → Permit → Tender → Construction) with **reproducible deliverables** (IFC, later PDF/DXF sheet sets) produced from that version's own project settings.
8. **Checks** (schema, IDS, the project's pre-commit checks) on every push, shown as statuses and usable as Combine rules. They run on **Actions** (metered).
9. **Private by default, customers' data protected** (§7).
10. **Metering for the revenue lines:** Actions minutes, package storage and transfer, AI usage (through the router).
11. **AI as reviewer, not author:** summaries, "ask the model", and AI-made options flagged. All of it goes through the owner's AI router.
12. **Architect language, EN/ES** from day one: *project, version, design option, proposal, combine, milestone*. Git words appear only in the clone panel and in settings.

## 3. Non-goals (this spec)

- **The browser never edits the model**, its settings or a conflict's values. The only server writes to a project are a Combine (merge commit with whole or per-conflict side choices) and imports (`.kim` / `.ifc`).
- A landing or marketing page. Certification programmes.
- General code-forge polish: syntax highlighting, text code review, wikis. Text files get a plain viewer and line diff.
- Live co-editing, federation, P2P, per-element discipline permissions.
- Billing and payment processing. NicGit records usage and enforces budgets; invoicing is a later integration.
- Renaming KimBim → NicBim (done elsewhere; NicGit talks to the engine through one adapter).

## 4. Anti-goals: failures even if it "works"

- Combine ever producing a git text merge of records. Pushes that look like one are rejected at the hook.
- One customer seeing, or being able to infer, another customer's data. That includes caches, job outputs, logs and AI requests.
- An architect needs git knowledge to review a proposal.
- A heavy page blocks for minutes with no feedback. Heavy work is a background job with progress, cached forever by content hash.
- The server silently changes a model. Server-written records go through the same engine code as the app and hash identically.
- Lock-in: everything is plain git plus exportable BCF/IFC. Reviews and issues are exportable.
- A permit deliverable that cannot be regenerated identically from its milestone.

## 5. Constraints

- **AGPL-3.0-or-later** (kimcad D08). Linking the GPL-3 `kimbim-repo` crate is compatible.
- **Reuse the engine, never fork its logic.** Missing engine capabilities (opening a bare repo, a Combine that takes whole-side resolutions, …) are added **in kimcad**, with tests.
- **Hosted first, on-prem identical:** one codebase, one set of container images. On-prem is the same compose file with local storage.
- **Limited hardware budget:** the always-on processes must be small. That drives the stack (§6).
- Git storage rules from kimcad `docs/03`: no record checkout, packs not loose objects, no deltas for record blobs.
- **Determinism:** a server Combine gives the same tree hash the desktop app gives for the same three commits and resolutions.
- Infra (compose, deploy scripts, ops docs) lives in this repo under `deploy/` until it moves to its own repo.

## 6. Architecture

```text
 browser (TS SPA)                         NicBim desktop / git CLI
     │ HTTPS JSON + websocket                  │ git smart HTTP (PAT)
┌────▼─────────────────────────────────────────▼─────────────────────┐
│ nicgit-server (Rust: axum + tokio)                                  │
│  API · auth · tenancy · permissions · git transport · hooks        │
│  fast git reads + tree diff via kimbim-repo (gix)                   │
│  job queue (Postgres) · metering · AI-router client                │
└───┬───────────────────────┬────────────────────────┬────────────────┘
 Postgres            repos volume                object storage (S3 API;
 (forge data,        <account>/<project>.git     MinIO or filesystem on-prem)
  jobs, usage)       (bare)                      caches, artifacts, packages
                          ▲ read-only
┌─────────────────────────┴──────────────────────────────────────────┐
│ engine workers (Python, kimbim + IfcOpenShell), sandboxed:          │
│ no network, CPU/RAM limits, one job at a time, scaled 0..N           │
│ semantic diff · merge preview/Combine · IFC export · geometry ·      │
│ checks · imports                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

**Stack (owner asked for a tougher, efficient stack):**

| Concern | Choice | Why |
|---|---|---|
| Server | **Rust: axum, tokio, tower, sqlx** | small memory and CPU footprint for the always-on process |
| DB | **Postgres** (also on-prem; it ships in the compose file) | multi-tenant, row-level security, `SKIP LOCKED` queue; one DB to test |
| Git transport | Rust spawns `git upload-pack` / `receive-pack --stateless-rpc` (as Gitea does), with our auth in front; hooks call back into the same binary (`nicgit hook …`) | proven protocol, no reimplementation |
| Git reads, tree diff | the engine's **`kimbim-repo` crate** (path dependency) | same code as the desktop app; skips identical subtrees |
| Heavy BIM work | **Python engine workers** pulling from the Postgres queue | diff details, merge and export live in Python/IfcOpenShell today; they move to Rust in kimcad over time ([future work](../../future-work.md)) |
| Caches / artifacts / packages | S3-compatible object storage, keyed by content (tree hash, blob hash) and namespaced per account | computed once, never shared across accounts |
| Frontend | **TypeScript + React + Vite SPA**, TanStack Query, EN/ES i18n | ecosystem for 3D and data-heavy UI |
| 3D | **That Open Engine** (three.js, fragments, web-ifc). Geometry per version produced by workers, streamed to the browser | browser IFC stack with picking, clipping, colour by GlobalId |
| Auth | argon2 passwords, cookie sessions, personal access tokens (scoped, hashed), TOTP 2FA; OIDC later | |
| Deploy | container images (server, worker, web assets in server), `deploy/compose.yaml` for on-prem and as the hosted baseline | |

Repository layout:

```text
crates/nicgit-server/     axum app: api/, auth/, tenancy/, git/, forge/, jobs/, metering/, ai/
crates/nicgit-core/       domain types, permissions, conflict policy (pure, unit-tested)
workers/engine/           Python package: job runner around kimbim
web/                      Vite SPA
deploy/                   compose, images, ops notes
docs/                     this spec, decisions, future work
pixi.toml                 dev env (Rust, Python engine from ../kimcad, Node, git, Postgres)
```

## 7. Tenancy, privacy and data protection

- **Accounts** are users or organisations. Every project belongs to exactly one account. **Projects are private by default**; *public* is an explicit, confirmed switch.
- Every query is scoped by account in code, with **Postgres row-level security** as a second wall.
- Caches and artifacts are namespaced per account even when content hashes match. No dedup across customers.
- **Workers are sandboxed:** no network, read-only access to only the repo the job names, CPU/RAM/time limits. IFC and `.kim` uploads are treated as hostile input (zip-slip and size checks are already in `kimfile.unpack`, plus zip-bomb limits).
- Encryption in transit (TLS only) and at rest (volume encryption; S3 server-side encryption); secrets such as AI keys and webhook secrets are encrypted at the application level. Tokens are stored hashed.
- **Audit log** per account: logins, permission changes, visibility changes, Combines, deletions, exports and downloads of private projects.
- Backups: Postgres PITR plus repo volume snapshots (hosted); documented on-prem procedure.
- Data rights: an account can export everything (git bundles, BCF, reviews as JSON) and delete everything.
- AI: model data leaves NicGit only when the account has enabled AI, only to the router, and only the structured diff or query context needed.

## 8. Domain model

| UI | Git / engine | Stored |
|---|---|---|
| Project | bare repo | repos volume + `projects` |
| Version | commit | git |
| Design option | branch (`agent/*` = AI-made) | git |
| Protected option | branch with rules (e.g. `main`, `permit`) | `option_rules` |
| Milestone / phase | annotated tag + phase | git + `milestones` |
| Proposal | combine source option → target option | `proposals`, ref `refs/proposals/<n>/head` |
| Review | approve / request changes / comment, by a member with a role | `reviews` |
| Pin | GlobalId(s) + BCF viewpoint + version | `comments`, exportable as BCF |
| Issue | BCF topic | `issues` |
| Check run | schema / IDS / project checks per commit | `check_runs` + artifacts |
| Project settings | `settings.json` in the version's tree (merged per key by the engine) | git |

**Files.** NicGit **version-controls NicBim project repositories.** The `.kim` file is a *transport* format: imported (its history kept) and exported (full, latest-only or one option), **never stored as a versioned file**. **`.ifc` files can be viewed**, and imported into a project as a new version through the engine's reconciled import; they are not versioned as files. See [decisions](../../decisions.md) N3 and N4.

**Roles** (per project, granted directly or through organisation teams):

| Role | Typical person | Can |
|---|---|---|
| Viewer | client, authority | view, comment, download allowed deliverables |
| Reviewer | consultant | + approve / request changes, open issues |
| Member | architect | + push, open proposals, Combine into unprotected options |
| Lead | project architect | + Combine into protected options, create milestones, change rules |
| Admin | office admin | + members, visibility, deletion, billing budget |

## 9. Proposals, conflicts and Combine

**Conflict policy** (owner: no editing in the browser; auto-solve or keep one side):

1. **Automatic:** the engine's record merge combines everything that does not conflict.
2. **Whole-side:** one choice for every conflict in the proposal: *target option wins* or *source option wins*.
3. **Granular:** per conflict (element, attribute or property, settings key, integrity conflict), choose target or source. The UI shows base, target and source values and the element in 3D. Values are never typed in.
4. If a choice leaves the model invalid (the engine's integrity pass still fails, e.g. a dangling reference), Combine is blocked with an explanation. The fix happens in the desktop app and is pushed to the source option.

**Rules per protected option**, configured by Leads:

- Required approvals: a count, plus required roles (e.g. "1 Lead + 1 Reviewer from Structure").
- Approvals are dismissed when the source option gets new versions (configurable).
- Allowed conflict modes (e.g. a `permit` option may forbid whole-side resolution, forcing a per-conflict review).
- Required checks (status must pass).
- **Protected settings keys:** changes to the project settings that drive deliverables (IFC export options, georeferencing and true north, sheet/title block templates, PDF/DXF export, units and model resolution, pre-commit checks) are highlighted in the diff and need an extra approval from a Lead. The exact key list comes from the engine's settings schema; it starts with the keys named here.

Combine: lock the proposal → re-run the merge preview at current heads → apply the chosen resolutions → merge commit (combiner as author, participants as co-author trailers, policy and approvals recorded in the commit message trailers) → compare-and-swap the target ref → enqueue jobs.

## 10. Screens

Model-first. The 3D view is the hero and lists sit beside it.

1. **Dashboard:** your projects, proposals awaiting your approval, issues assigned, activity, usage against budget (admins).
2. **Project overview:** 3D of the default option's latest version, project info (schema, units, georeference, levels, counts), options, milestones, open proposals and issues, clone and "open in NicBim" panel.
3. **Model browser (any version):** viewer, spatial tree, properties, search and filter, schedule table with CSV export, section box, level isolate, plan cut, measure, shareable viewpoints.
4. **Versions:** per-option timeline with plain summaries, author, AI flag, check status; option graph.
5. **Compare:** any two versions, options or milestones. A 3D overlay (before / after / overlay / side by side) with change colours; changes grouped by class and level; field-by-field table; **Settings** section listing changed keys old → new, protected keys flagged.
6. **Proposals:** *Overview* (description, AI summary, required and received approvals by role, checks, Combine with the allowed conflict modes), *Changes*, *Conflicts* (whole-side switch or per-conflict picks, 3D per conflict), *Conversation* (pins shown as 3D markers).
7. **Element history.**
8. **Issues (BCF).**
9. **Milestones / phases:** sign-off and deliverables built from that version's settings.
10. **Checks and Actions runs**, with usage.
11. **Files (raw)** and raw record JSON behind a toggle.
12. **Settings:** project (rules, protected options and keys, checks, AI), members and roles, organisation and teams, profile, tokens, 2FA, usage and budgets, audit log.

## 11. Server flows

- **Push:** auth → `pre-receive`: records parse as canonical JSON of the project's format/schema; protected options accept only fast-forwards from users with rights (Combine is the normal path into them); size limits. `post-receive`: activity, proposal updates, jobs (`summary`, `geometry`, `checks`).
- **Compare:** Rust tree diff answers "what changed, how much" instantly; the detail job runs the engine's `diff_details` (cached by tree-hash pair).
- **Combine:** §9.
- **Imports:** `.kim` → worker unpacks in its sandbox, the server fetches its objects and refs into the bare repo. `.ifc` → engine reconciled import as a new version on a chosen option (heuristic matches listed for review first).
- **Exports:** IFC per version (cached); `.kim` export (full / latest / one option); deliverables per milestone.

## 12. Metering and budgets

- `usage_events` (account, project, kind, quantity, time). Kinds: `action_seconds`, `storage_bytes` (sampled), `egress_bytes`, `package_storage_bytes`, `package_egress_bytes`, `ai_units` (as reported by the router).
- Budgets per account per kind: soft limit (warn), hard limit (block new Actions and AI calls; never block reading or pushing your own data).
- Engine jobs the platform needs to show a project (geometry, summaries) are **not** Actions. Actions are the checks and workflows the customer configures. Exact split: [decisions](../../decisions.md) N7.
- Billing integration later.

## 13. AI

All AI goes through the owner's **cloud AI router** (endpoint and credentials in server config; on-prem installs can point at it or at a router they run). Off by default per account.

- Summaries of versions and proposals from the engine's structured diff, labelled as AI.
- "Ask the model" about a version, answered with the engine's read-only agent tools.
- `agent/*` options and their proposals are badged; AI never Combines.

## 14. Phases (build order)

1. **Spine:** Cargo workspace, Postgres + migrations, accounts, orgs, teams, roles, sessions, PATs, RLS, audit log, git smart HTTP with hooks, SPA shell, dashboard, project list, raw files. *Exit:* `kimbim versions push` to NicGit and clone back give identical tree hashes; a second account cannot see the project.
2. **See it:** worker runner + queue, engine adapter on bare repos (kimcad change), geometry and IFC export jobs, viewer, model browser, overview, timeline with summaries. *Exit:* the sample house is browsable at every version.
3. **Compare it:** compare screen including the settings diff, element history. *Exit:* matches `kimbim versions diff --json` on the M5 scenario.
4. **Propose and Combine:** proposals, roles, approvals and rules, conflict modes, protected keys, Combine. *Exit:* the M5 conflict resolved whole-side and per-conflict in the browser gives the CLI's tree hashes.
5. **Issues, milestones, imports/exports:** BCF, phases, `.kim`/`.ifc` import, `.kim` export, IFC deliverables.
6. **Actions and metering:** checks as Actions, budgets, usage UI.
7. **AI via the router**, notifications, webhooks, Spanish completeness.
8. Then [future work](../../future-work.md): packages registry, PDF/DXF deliverables, SSH, OIDC, linked `.kim` projects, Rust engine paths.

## 15. Testing

- Rust: unit tests (permissions, conflict policy, tenancy) and integration tests against real Postgres and real `git`.
- Workers: pytest against kimcad fixture projects (sample house, M5 options).
- **Golden:** push/clone tree hashes; server Compare == CLI diff; server Combine tree hash == CLI merge with the same resolutions.
- **Isolation tests:** every API route is called as a foreign account and must return 404.
- Web: Vitest; Playwright for login → push → view → compare → propose → approve → combine.
- Performance: overview of a 20k-element model under 2 s warm; idle server under 100 MB RSS.
