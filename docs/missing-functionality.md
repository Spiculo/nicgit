# Missing functionality — audit

**Date:** 2026-11-27 · **Scope:** the NicGit web app and the NicBim ↔ NicGit flow · **Status:** audit
**Related:** [design](superpowers/specs/2026-11-03-nicgit-design.md) · [future work](future-work.md) · [repository navigation](specs/2026-11-25-repository-navigation.md)

This is the list of what the apps do not do yet, or do only partly. It is based on the working tree on
2026-11-27: every route, page, API module and documented claim was read, and the empty states were
checked against the server routes. It is a static audit; a browser pass over every page is still
needed before release.

The list is ordered by what a user notices first. "Built" means the page and its API exist and are
wired; "partial" means the page exists but an important action is missing; "empty" means the page is a
single empty state.

## 1. Summary

| Tier | Area | State |
|---|---|---|
| 1 | CI / Actions tab | **Built in this change**: runs, required checks, detail and printable report |
| 1 | Renders tab | Empty |
| 1 | Sheets tab | Empty (needs the deliverables worker) |
| 1 | Packages tab | Empty (needs the package registry) |
| 1 | Agents tab | Empty (needs server-side agent sessions) |
| 1 | Project Configs → Webhooks | Empty |
| 1 | Project Configs → Audit log | Empty |
| 2 | Preview tab | Partial: latest version and downloads, no 3D or plan preview |
| 2 | Issues | Partial: no edit of title, body, priority, assignee or viewpoint; no 3D camera restore |
| 2 | Proposals | Partial: comments cannot be pinned in 3D; no check log per run |
| 2 | Versions | Partial: no version detail page, no AI summary on the timeline |
| 2 | Files | Partial: raw JSON only, no file history, blame or raw download |
| 2 | Organisations and teams | Partial: create/add/grant, but no remove/rename/delete in the UI |
| 2 | Account access | Partial: tokens only; no 2FA, sessions list or scoped tokens |
| 2 | Notifications | Partial: posts on Home; inbox endpoint has no page, no read state, no email |
| 2 | Billing and budgets | Partial: usage and storage meters; no budget editor, invoice or plan |
| 2 | Project transfer and archive | Missing: import/export `.nic` exists, transfer and archive do not |
| 3 | SSH Git transport | Missing |
| 3 | OIDC/SAML/LDAP sign-in | Missing |
| 3 | Object storage (S3) | Missing; cache and exports live on the data volume |
| 3 | Live job progress | Polling every second, no websocket |
| 3 | Engine compatibility warning | Missing: `/api/instance` exists, the web and app do not compare `engine.rev` |
| 3 | Linked `.nic` projects | Missing (NicBim `docs/08`, NicGit `docs/future-work.md`) |
| 3 | Settings schema | Missing; protected keys are hard-coded |
| 3 | Engine on bare repositories | Missing for several paths; the worker uses a worktree |
| 3 | Files-first diff bundle | Missing (N51/N52) |
| 3 | GPU render service | Missing (N53/N54) |
| 3 | Deliverables worker (PDF/DXF sheets) | Missing (N58) |
| 3 | Webhooks and public API | Missing |
| 3 | 2FA, invitations, email, teams UI | Missing or partial |

## 2. Tier 1 — pages that are shells

### 2.1 CI / Actions — built in this change

The server now exposes:

| Method | Path | Purpose |
|---|---|---|
| `GET` | `/api/projects/{owner}/{slug}/ci/runs?option=&rev=&limit=` | Checks runs, newest first |
| `GET` | `/api/projects/{owner}/{slug}/ci/runs/{id}` | One run with its full result |
| `GET` | `/api/projects/{owner}/{slug}/ci/checks` | Required checks for every option and their latest state |
| `GET` | `/api/projects/{owner}/{slug}/ci/runs/{id}/report.html` | Printable report generated from the job result |

The web page lists runs, filters by option, expands a run, shows each check's counts and issues, and
links to the report. It reads the existing `checks` jobs; it does not run anything.

Still missing in CI:

- A "run now" action. The UI must not invent one until Actions can be queued from the web with a
  budget check and a target version.
- Log streaming; the page polls the job row.
- A comparison of two runs (regressions between versions).
- Scheduled checks and branch protection beyond the existing rules.

### 2.2 Renders

The tab is empty. The GPU render service is specified in
[GPU render service](specs/2026-11-25-gpu-render-service.md) but not built:

- No render job kind, queue, GPU worker or artifact store.
- No recipe validation, cost estimate or budget hold.
- No Jobs / Presets / GPU usage page.
- The NicBim CLI has no `nicbim render` command yet.

### 2.3 Sheets

The tab is empty. The engine can export a sheet headlessly (`kimbim-app --export-sheet`), but the
server has no `sheet_export` job and no deliverables worker with Qt offscreen. Missing:

- `GET /sheets?rev=` list of a version's sheets.
- `POST /sheets/export` job and cached PDF/SVG/PNG artifacts.
- Revision records, revision clouds and transmittals.
- `dependency_missing` handling for linked models and libraries.

### 2.4 Packages

The tab is empty. Missing:

- A package registry (`.nicobj`, office libraries, IFC type libraries, rule packs).
- Upload, versions, scopes, public/private visibility and download.
- A project pin of a library hash and the `library_pinned` / `library_missing` state.
- The Home "Packages" panel always says "No packages yet." because the API returns an empty list.

### 2.5 Agents

The tab is empty. Missing:

- Server-side agent sessions, transcripts and status.
- The tool catalogue and the role each tool needs.
- Review and approval of agent-made options.
- Any agent run on push.

The desktop app already has local agent sessions and worktrees (NicBim `docs/18`); the server side is
the missing half.

### 2.6 Configs → Webhooks

The page is an empty state. The spec and the settings side rail promise webhooks, but there is no
`webhooks` table, no secret storage, no delivery worker and no API. The page must stay empty until the
backend exists (N56: no dead controls).

### 2.7 Configs → Audit log

The page is an empty state. The server records audit rows in `audit_log` for logins, permission
changes, visibility changes, combines, deletions, exports and downloads, but there is no read API and
no page. Missing:

- `GET /api/projects/{owner}/{slug}/audit?limit=&before=`.
- An account-level audit view for admins.
- Filters by actor, action and date, and a CSV export.

## 3. Tier 2 — built but incomplete

### 3.1 Preview

Per [repository navigation](specs/2026-11-25-repository-navigation.md#4-preview) the Preview tab is
deliberately a latest-version summary until the files-first bundle exists. It currently shows the
message, option, author, date, commit and downloads, but no model at all. A user who clicks
"Preview" expects to see the building. Missing:

- The files-first diff bundle (`manifest.json`, `model.glb`, `removed.glb`, `plan.json`).
- A 3D + plan preview of the latest version and of the changed elements.
- A lightweight "open the full viewer" escape hatch for small models.

### 3.2 Issues

The list, create, comment, open/close and BCF import/export work. Missing:

- Editing an issue after creation: title, body, priority, assignee, elements and viewpoint.
- Restoring the saved BCF camera; "Viewpoint" only selects the element and opens the overview.
- Placing or showing the BCF viewpoint in the 3D viewer.
- Bulk actions (assign, close, label), saved filters and a per-assignee view.
- Attaching files or screenshots to an issue.
- A link from an issue to the version it was made on in the version list (only a query parameter).

### 3.3 Proposals

The list, create, review, comment, close, preview, combine and AI summary work. Missing:

- Pinning a comment to elements or a viewpoint in 3D; the API accepts a `pin`, the UI never sends one.
- Showing pinned comments as markers in the comparison viewer.
- A checks tab with each required check, its run, its report and its failure reasons.
- Review dismissal rules beyond "dismiss on push"; no UI to see which review was dismissed by which push.
- Draft proposals, reviewers requested by team and review reminders.
- A diff of the project settings with a plain-language label per key (only the raw key is shown).

### 3.4 Versions

The timeline, summaries, compare link and pagination work. Missing:

- A version detail page (message, author, parents, checks, downloads, comments).
- AI summaries on the timeline (the proposal summary exists; versions do not).
- A graph of options and merges.
- Filtering by author, check status or date.
- A per-version diff against the previous version without leaving the list (the link exists, but no
  inline summary beyond counts).

### 3.5 Files

The tree, raw JSON viewer and revision selector work. Missing:

- File history and blame ("who changed this record and when").
- A raw download and a "copy as JSON" action.
- A line or JSON diff between two versions.
- A search across paths and record contents.
- Large-file handling; a big blob is loaded whole into one `<pre>`.

### 3.6 Organisations and teams

The org profile can create a team, add a team member and grant a team a project. Missing:

- Remove a team member, revoke a project grant, rename or delete a team.
- Change or remove an org member after adding them.
- See team members and grants (only counts are shown).
- Invite by email; members are added by user name only.
- Organisation-level settings page (SSO, billing, default roles).
- Audit of team and role changes.

### 3.7 Account access and security

The Access page creates and revokes personal access tokens. Missing:

- Scopes per token (read-only, push, admin) and an expiry date.
- A session list and "sign out other devices".
- TOTP 2FA and recovery codes.
- OIDC/SAML/LDAP sign-in for studios not on Google.
- A "last used" location or device.
- A token name that is unique per user.

### 3.8 Notifications and inbox

Home shows posts from `/api/me/home`; `/api/me/inbox` and `/api/me/notifications/read` exist but have
no page. Missing:

- A notifications page, unread counts and mark-as-read.
- Email notifications and per-event preferences.
- Webhooks (see 2.6).
- Mentions in comments and reviews.
- A notification when a required check fails or a proposal is combined.

### 3.9 Billing, usage and budgets

The Billing page shows storage and usage totals. Missing:

- A budget editor per account and kind (the API has `PUT /api/accounts/{slug}/budgets/{kind}`).
- Soft and hard limit states in the UI.
- Per-project and per-user usage breakdowns.
- Invoices, plan and payment method.
- A warning before a hard limit blocks an Action.

### 3.10 Project administration

General, People, Rules and Delete work. Missing:

- Rename the project or move it to another owner (transfer).
- Archive a project (read-only, hidden from the active list).
- Fork a public project.
- A project avatar or cover image.
- A README or description rendered on the overview; only a one-line description exists.
- A licence field and a citation file.
- Delete with a grace period or restore.

## 4. Tier 3 — platform and cross-app flow

### 4.1 NicBim ↔ NicGit flow

What works today:

1. `kimbim versions push/pull` to `https://<host>/<owner>/<project>.git` with a personal access token
   as the password (HTTP Basic).
2. The desktop app now has **Versions > Git...**: remotes, the token in the OS credential store,
   fetch/status, push/pull, tags, force push, remote branch deletion, clone and branch management.
3. **File > Clone project from remote...** clones a project without checking the record tree out.
4. NicGit shows the project address and the steps to connect NicBim (this change).

What is still missing in the flow:

- **Open in NicBim** from the browser. There is no registered URL scheme, no `.niclink` file and no
  deep link. The user copies the address by hand.
- **Engine compatibility check.** Both sides can read `/api/instance` (`engine` revision), but neither
  warns when the desktop app and the server disagree about the record format. A push from a newer
  engine can reach an older worker and fail at job time instead of at push time.
- **First-version onboarding.** A newly created project shows the empty state with the address, but
  it does not offer a "Download a `.nic` starter" or a one-click clone link. The empty state is text
  and a copy button.
- **Linked `.nic` projects.** A project connected to NicGit still has to be a full clone. The small
  linked file (manifest, URL, option) from NicBim `docs/08` does not exist.
- **Partial clone.** No `blob:none` or sparse fetch; a client downloads the whole history. The desktop
  clone uses `--no-checkout`, but it still fetches every record.
- **Push-time checks.** Required checks run after the push (post-receive), not before it. A push that
  violates a rule is accepted and then shown as failed. The spec allows both; a pre-receive policy
  with a clear message is missing.
- **Migration from another forge.** A project can be imported from `.nic`/`.ifc`, but not by URL from
  GitHub/GitLab/Forgejo.
- **Server-side branch protection on push.** Protected options are enforced for Combine, but a direct
  `git push` to a protected option is not rejected unless a rule hook exists. This needs a pre-receive
  hook that checks the option and the caller's role.

### 4.2 Git transport and auth

- SSH transport is missing. Only HTTPS smart HTTP is served.
- Token scopes and expiry are missing (see 3.7).
- The desktop app uses the system `git`; if Git is not on `PATH`, every remote action fails. The app
  reports the error but cannot install or bundle Git.
- HTTPS tokens are stored per host. Two accounts on the same host cannot both be stored; switching
  accounts means replacing the token. A token name/account key is missing.
- Self-signed certificates and custom CAs have no setting; the user must configure the system Git.
- Proxy support is environment-based; the desktop Preferences page sets `https_proxy`, but the web
  server has no outbound proxy setting.

### 4.3 Deployment and operations

- Published images, signatures and a release workflow are missing (N44).
- Invitations and an admin "who joined" page are missing.
- SMTP email for self-hosted servers is missing; hosted uses Postmark.
- OIDC, SAML and LDAP are missing.
- Engine compatibility checks at job claim time are missing.
- Per-deployment limits exist in config but not in an admin UI.
- Self-hosted licensing and usage reporting are undecided (N42/N43).
- Postgres row-level security is not the second wall yet; the per-route checks and isolation tests exist.
- Object storage is the data volume, not S3/MinIO.
- There is no Helm/Kubernetes manifest; compose is the only deployment.

### 4.4 Large models and browser performance

- The browser loads a full IFC and builds one mesh per element. A 100k-element model is 100k meshes
  and draw calls; a comparison doubles that.
- The files-first bundle (GLB of changed elements + plan JSON) is specified but not built.
- No level isolation, section box, plan cut, measurement or clipping in the viewer.
- The viewer fits the whole model on load; there is no saved camera, viewpoint or share link.
- No streaming, LOD or progressive loading.
- The web bundle is 4.6 MB before gzip and has no route-level code splitting.

### 4.5 Tests and quality

- Web tests are Vitest unit tests only. The spec calls for Playwright; no end-to-end browser test
  exists in this repo.
- No visual regression tests for the trace-paper layout.
- The CI tab has server tests; the page has no component test.
- The audit doc itself is static; a link checker or route smoke test would catch dead links.

## 5. Bugs found during the audit

| Bug | Where | State |
|---|---|---|
| Import slug stripped `.kim`, not `.nic` | `web/src/pages/ImportProject.tsx` | **Fixed** in this change |
| Empty project showed `kimbim versions remotes/push` CLI instructions, not the app flow | `web/src/pages/Overview.tsx` | **Fixed** in this change |
| The CI report route cannot be `{id}.html` in axum (one parameter per segment) | CI API | **Fixed**: `/ci/runs/{id}/report.html` |
| The repository-navigation spec says teams are not in the API/UI; they are | `docs/future-work.md` | Doc is stale; update when next touched |
| A first `compare` default uses `option~1`, which can be an unborn revision | `web/src/pages/Compare.tsx` | Candidate; needs a real first-commit test |
| `kimbim` still appears in code identifiers and some docs while the product is NicBim | NicBim repo | Known, H1 item 8 in NicBim `docs/22` |

## 6. Suggested order

| Order | Work | Why | Effort |
|---|---|---|---|
| 1 | Audit log API + page | The data exists; admins need it; no new worker | Small |
| 2 | Issue editing + 3D viewpoint | Issues are useless if they cannot be corrected or seen | Small |
| 3 | Pinned proposal comments in 3D | The API already accepts the pin; the viewer needs markers | Medium |
| 4 | Version detail page + AI summaries | Completes the timeline | Medium |
| 5 | Sheets + deliverables worker | Central architectural deliverable | Large |
| 6 | Files-first diff bundle + Preview | Fixes the biggest viewer performance and product gap | Large |
| 7 | Packages registry | Revenue line and object library | Large |
| 8 | Agents on the server | Requires the desktop session contract | Large |
| 9 | Renders + GPU service | Requires the GPU service spec | Large |
| 10 | 2FA, invitations, email, OIDC | Required for real studios | Medium each |
| 11 | SSH, partial clone, linked `.nic` | Makes the desktop flow scale | Medium each |

## 7. What this change implements

- **NicGit:** the CI tab (runs, required checks, detail, printable report), its server API and tests;
  the NicBim connection panel in the project overview and empty state; the import slug fix.
- **NicBim:** a full Git dialog (remotes, token, fetch/status, push/pull, tags, force push, remote
  branch deletion, clone, options), HTTPS token storage in the OS credential store, a Preferences
  dialog with a category sidebar and the new Git, network, appearance, folder, plugin and update
  settings, and tests for the dialog and the preferences.
