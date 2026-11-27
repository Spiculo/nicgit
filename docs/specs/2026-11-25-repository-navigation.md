# Repository navigation: GitHub-shaped tabs and settings pages

**Status:** Shell and settings pages implemented; CI is built; other unbuilt tabs show one empty state · **Date:** 2026-11-25 (CI added 2026-11-27)
**Scope:** NicGit web app · **Related:** [files-first diff viewer](2026-11-25-files-first-diff-viewer.md),
[GPU render service](2026-11-25-gpu-render-service.md), decisions N51–N59

GitHub's information architecture, adapted to buildings. A project has a **repository tab bar** and,
only where a tab has several real pages, a **settings side rail**. Code does not get a sidebar;
Configs does. The visual language stays the existing trace-paper/ruled-schedule system (N55): no card
grids, no second design system.

The shell is built. All existing pages were mapped into it. Tabs whose pages are not built show a
single empty state with no fake data, no dead controls and no description of the future page.
Editorial paragraphs are not rendered in the UI; help for a non-obvious field sits behind an `(i)`
control (N59).

---

## 1. Tab bar

Order left to right:

| Tab | Route | Status | Sidebar |
|---|---|---|---|
| **Code** | `/` and code sub-pages | Built | No (horizontal sub-nav) |
| **Pull requests** | `/proposals` | Built (proposals) | No |
| **Issues** | `/issues` | Built | No |
| **Preview** | `/preview` | Built (latest version and downloads) | No |
| **CI** | `/ci` | Built (reads the existing checks jobs) | No |
| **Renders** | `/renders` | Not built | No |
| **Sheets** | `/sheets` | Not built | No |
| **Packages** | `/packages` | Not built | No |
| **Agents** | `/agents` | Not built | No |
| **Configs** | `/settings` and settings sub-pages | Built + placeholders | Yes: General / People / Protected options / Webhooks / Audit log / Delete |

Counts: Pull requests and Issues show their open counts in the tab, like GitHub. Configs is visible
to admins only. No tab is a marketing page; every route stays inside the signed-in project. An
unbuilt tab opens one empty state (`No runs yet.`, `No renders yet.`, and so on) and nothing else.
Tab labels follow the account's wording style (N61): the same tab bar reads Code / Pull requests /
Issues for a Git account, or Project / Proposals / Tasks for an architectural one.

## 2. Code

Code is the repository landing and has a **horizontal sub-nav**, not a sidebar:

`Overview · Files · Versions · Branches · Compare · Releases · Activity`

- **Overview** is the current project page (latest version, options, files, clone address). Its 3D
  panel is the current viewer; it is being replaced by the files-first preview (N51/N52).
- **Files** is the repository file browser and raw record/JSON view.
- **Versions**, **Branches**, **Compare**, **Releases** (milestones + tags) and **Activity** keep their
  existing behaviour.
- The branch selector lives on Files; clone and download actions live on Overview and Preview;
  release links live on Releases. They are not duplicated in a sidebar.
- Code must not grow a left rail. A feature that needs pages gets its own tab, and only
  settings-style pages get a side rail.

## 3. Pull requests and Issues

Existing proposals become the **Pull requests** tab; issues stay Issues. Both use the existing pages,
filters and detail views. No sidebar: the list filters and the detail tabs (Conversation, Changes,
Checks, Conflicts) are enough.

Naming: the nav and page say **Pull requests**; the API and database keep `proposals`. The form
still uses the project language of design options and combining. There is no explanatory paragraph.

## 4. Preview

The Preview tab is the home of the files-first diff direction, not a second full BIM viewer.

- It opens on the latest version: message, option, author, date and commit.
- Actions: open the existing 3D diff (Compare), browse Files, and download IFC, `.nic`, source
  archive and BCF for that version.
- It has no sidebar and no explanatory copy. When the diff bundle exists, the Preview tab becomes
  the 3D + plan diff page and the link to the old Compare is removed.

## 5. CI

The CI tab reads the checks jobs the Actions runner already produced. It lists runs and required
checks, expands a run's issues, and links a printable report. It does not queue work yet.

### Contract

| Method | Path | Purpose |
|---|---|---|
| `GET` | `/api/projects/{owner}/{slug}/ci/runs?option=&rev=&limit=` | Check jobs per version/option |
| `GET` | `/api/projects/{owner}/{slug}/ci/runs/{id}` | One run: kind, commit, status, duration, report |
| `GET` | `/api/projects/{owner}/{slug}/ci/checks` | Required checks per option and their last result |
| `GET` | `/api/projects/{owner}/{slug}/ci/runs/{id}/report.html` | Printable report generated from the job result |

- A run is the existing `jobs` row of kind `checks`, with `args.rev`, `result.passed`, `result.report`.
- The page lists commit, option, checks (schema/IDS/clash/model), pass/fail, duration and a report link.
- A required check that fails blocks a Combine (already enforced); the CI page shows the same result.
- Schedules (nightly checks) and budgets are future work until Actions get their own runner.
- No new execution model: CI displays what Actions already run; the runner stays out of scope.

## 6. Renders

The tab that owns the [GPU render service](2026-11-25-gpu-render-service.md). Today it shows
`No renders yet.`.

- **Jobs**: queued/running/finished, recipe, version, GPU class, progress, actual cost, frames.
- **Presets**: the recipes a studio keeps; a recipe is a file, not a server-side blob.
- **GPU usage**: seconds and cost per account/project/job, from `render_gpu_seconds`.

No render is implemented yet. The CLI (`nicbim render`) is specified in the render service spec; the
web page will read the same API.

## 7. Sheets

This is the missing deliverables space. Sheets are **files generated by the engine**, not browser
drawings. Today the tab shows `No sheets yet.`

### Contract

| Method | Path | Purpose |
|---|---|---|
| `GET` | `/api/projects/{owner}/{slug}/sheets?rev=` | Sheet list for a version: number, title, size, scale, revision |
| `POST` | `/api/projects/{owner}/{slug}/sheets/export` | Job: one sheet or a set to PDF/SVG; returns artifacts |
| `GET` | `/api/projects/{owner}/{slug}/sheets/{number}.pdf?rev=` | Generated PDF (job or cached artifact) |
| `GET` | `/api/projects/{owner}/{slug}/sheets/{number}.svg?rev=` | Generated SVG |
| `GET` | `/api/projects/{owner}/{slug}/sheets/{number}.png?rev=` | Raster preview for the list |

Rules:

- A sheet is generated from **that version's own** `settings.json`, IFC and embedded files. It never
  reads the working tree.
- Linked models are not silently included: a sheet that needs a link is marked `dependency_missing`
  unless the link snapshot is pinned (see the files-first spec, external dependencies).
- Revisions and issue sets are records, not live links; revision clouds and transmittals are later.
- The browser only lists and downloads files; it does not run the drawing generator.
- The engine already knows how to export sheets (`kimbim-app --export-sheet`); the missing piece is
  the worker job and the API.

### Job

`sheet_export`: `{rev, sheets: [numbers], format: pdf|svg, dpi}` → zip of files + `manifest.json`
(sheet number, title, revision, bytes, sha256). Cache by `(project, commit, sheet, format, dpi)`.
The worker needs Qt offscreen in the runtime; that is a separate `deliverables` worker image, not the
current Python-only engine worker. This is why Sheets is a spec, not a button that pretends to work.

## 8. Packages

The object-library and package registry space. Today the tab shows `No packages yet.`

- **Objects**: `.nicobj` definitions and versions, scoped to an account, public or private.
- **Libraries**: office libraries and IFC type libraries, pinned by content hash; a project version
  records which library hash it used, so the web can say `library_pinned` or `library_missing`.
- **Rule packs**: IDS/code/check packs a project can require.
- Backed by the package registry in [future work](../future-work.md) (not built). The first
  implementation can be a list of pinned library hashes and downloads, then grow into
  upload/versioning.

## 9. Agents

The agent space that NicBim already has locally, moved to the repository where a team can see it.
Today the tab shows `No agents yet.`

- **Sessions**: an agent session, the option it works in, transcript and status.
- **Tools**: the tool catalogue and the role each tool needs; read-only tools are safe, write tools
  go through a reviewable option.
- **Approvals**: the review bar for agent-made changes; a person accepts, rejects or combines.
- Not implemented in NicGit. The desktop app (NicBim docs/18) has the tool catalogue and session
  worktrees; the server-side session/tool contract is future work. No agent runs automatically on a
  push.

## 10. Configs

The project settings, split into pages with a side rail. Each page saves on its own; a save does not
throw away fields on another page. The sidebar navigates to routes and marks the current page.

| Page | Route | Status |
|---|---|---|
| **General** | `/settings/general` | Built: description, visibility, default option |
| **People** | `/settings/people` | Built: project members and roles |
| **Protected options** | `/settings/rules` | Built: approvals, conflict modes, required checks, protected settings |
| **Webhooks** | `/settings/webhooks` | Empty state |
| **Audit log** | `/settings/audit` | Empty state |
| **Delete project** | `/settings/danger` | Built: typed confirmation |

`/settings` redirects to `/settings/general`. AI summaries and budgets are **account** settings, not
project settings, and live in the account pages below.

### Account settings

Account settings use the same side rail outside a project:

| Page | Route | Content |
|---|---|---|
| **Profile** | `/settings/profile` | Display name, email, user name |
| **Access** | `/settings/access` | Personal access tokens |
| **Language & wording** | `/settings/appearance` | Language and Git/architectural wording |
| **Organisations** | `/settings/organizations` | Membership and new organisation |
| **AI** | `/settings/ai` | Account AI switch and router state |
| **Billing** | `/settings/billing` | Storage, per-project and per-file limits, monthly usage |

`/account` redirects to `/settings/profile`.

## 11. Compatibility and dependencies

The tab shell does not change the data model, but it exposes two things that must not be guessed:

- **Linked models and libraries** are external dependencies, handled by the files-first policy
  (N57): the browser never resolves a path; a bundle bakes a pinned snapshot or shows a proxy with a
  status; `.nic` full export remains the portable archive.
- **Sheets and renders** consume the same dependency pins. A sheet or render with a missing link or
  library fails with `dependency_missing`, never with a silently incomplete image.

## 12. Permissions

- Viewers see Code, Preview, CI, Renders (own/read), Sheets, Packages and Issues.
- Members can open pull requests, submit renders (budget permitting) and download artifacts.
- Leads manage protected options and releases; admins see Configs and People.
- The sidebar hides nothing that the API would allow; each page still enforces the role server-side.
- A private project remains 404 for anyone without access, including every tab route.
- A non-admin who opens a settings route directly gets the not-found page, not the settings form.

## 13. Mobile and small screens

- The tab bar scrolls horizontally; it never wraps into a second row.
- The Configs side rail becomes a horizontal sub-nav above the content; account settings do the same.
- The Code sub-nav scrolls horizontally; it does not become a drawer.
- Tables keep their horizontal scroll; no card fallback that changes the information.

## 14. Acceptance criteria

1. Every existing page is reachable from the tab/sub-nav; no dead link.
2. Pull requests and Issues show open counts; Configs is admin-only.
3. Code has no sidebar; among project tabs, only Configs has a side rail.
4. Each Configs sidebar link opens its own route and the active page is highlighted.
5. Unbuilt tabs show one empty state: no fake data, no dead controls and no description of the
   future page.
6. A viewer cannot reach Configs; the API still returns 404/403.
7. Account settings are split into pages under `/settings` (Profile, Access, Language & wording,
   Organisations, AI, Billing); `/account` redirects to Profile.
8. Explanatory prose is not rendered in page bodies. Help for a non-obvious field sits behind an
   `(i)` control; empty states keep one short line.
9. The layout holds at 1,280 px and at 800 px; page content is centred at the same maximum width, with
   no unexplained gap on the right.
10. The UI uses the existing `.schedule`, `.tag`, `.btn`, `.form-block` language (N55).

## 15. Implementation sequence

1. **Done:** tab bar, Code sub-nav, settings side rails and page routes, Preview page, empty tabs,
   account settings pages, i18n EN/ES.
2. **Sheets:** `sheets`/`sheet_export` engine job, deliverables worker with Qt offscreen, API, page.
3. **CI:** done: read the existing check jobs, one run list, report downloads.
4. **Renders:** follow the render service spec; Jobs/Presets/Usage pages.
5. **Packages:** pinned library list first, then upload/versioning.
6. **Agents:** server-side sessions and approvals, after the desktop session work.
7. **Preview:** replace the current 3D diff with the files-first bundle (N51/N52).

## 16. Open questions

1. Should **Sheets** be a top-level tab or a subsection of Code next to Releases? GitHub puts
   releases under Code; deliverables are central to architecture, so a tab is proposed.
2. Should **Renders** be a tab or part of CI? Rendering is a paid, customer-facing workflow; a tab
   is proposed.
3. Should **Packages** include the object library immediately, or wait for the registry?
4. Should **Agents** be visible to clients, or only to the studio?
5. Should Configs include **organisation-level** settings (teams, SSO, billing) or only the project?
   The org page already exists; a later “Organisation settings” tab is likely.
6. “CI” vs “Actions”: the existing docs call billable checks *Actions*. The tab is named CI to match
   GitHub; the page copy says Actions.
