# Repository navigation: GitHub-shaped tabs and contextual sidebars

**Status:** Shell implemented; pages marked “not built” are specified here · **Date:** 2026-11-25
**Scope:** NicGit web app · **Related:** [files-first diff viewer](2026-11-25-files-first-diff-viewer.md),
[GPU render service](2026-11-25-gpu-render-service.md), decisions N51–N58

GitHub's information architecture, adapted to buildings. A project has a **repository tab bar** and,
only where a tab has several spaces, a **contextual sidebar**. Code does not get a sidebar; Configs
does. The visual language stays the existing trace-paper/ruled-schedule system (N55): no card grids,
no second design system.

The shell is built. All existing pages were mapped into it. Tabs whose pages are not built show what
they will contain and link to their spec; they have no fake data and no dead controls.

---

## 1. Tab bar

Order left to right:

| Tab | Route | Status | Sidebar |
|---|---|---|---|
| **Code** | `/` and code sub-pages | Built | No (horizontal sub-nav) |
| **Pull requests** | `/proposals` | Built (proposals) | No |
| **Issues** | `/issues` | Built | No |
| **Preview** | `/preview` | Shell + current diff link | Yes: Current / Next / Files |
| **CI** | `/ci` | Specified, not built | Yes: Runs / Checks / Schedules / Budgets |
| **Renders** | `/renders` | Specified, not built | Yes: Jobs / Presets / GPU usage |
| **Sheets** | `/sheets` | Specified, not built | Yes: Sheets / Drawings and PDFs / Revisions |
| **Packages** | `/packages` | Specified, not built | Yes: Objects / Libraries / Rule packs |
| **Agents** | `/agents` | Specified, not built | Yes: Sessions / Tools / Approvals |
| **Configs** | `/settings` | Built + placeholders | Yes: General / People / Protected options / AI / Budgets / Webhooks / Audit / Delete |

Counts: Pull requests and Issues show their open counts in the tab, like GitHub. Configs is visible
to admins only. No tab is a marketing page; every route stays inside the signed-in project.

## 2. Code

Code is the repository landing and has a **horizontal sub-nav**, not a sidebar:

`Overview · Files · Versions · Branches · Compare · Releases · Activity`

- **Overview** is the current project page (latest version, options, files, clone address). Its 3D
  panel is the current viewer; it is being replaced by the files-first preview (N51/N52).
- **Files** is the repository file browser and raw record/JSON view.
- **Versions**, **Branches**, **Compare**, **Releases** (milestones + tags) and **Activity** keep their
  existing behaviour.
- The branch selector, clone/download actions and release links live in the page header, as in
  GitHub; they are not duplicated in a sidebar.
- Code must not grow a left rail. If a feature needs sections, it gets its own tab with a sidebar.

## 3. Pull requests and Issues

Existing proposals become the **Pull requests** tab; issues stay Issues. Both use the existing pages,
filters and detail views. No sidebar: the list filters and the detail tabs (Conversation, Changes,
Checks, Conflicts) are enough.

Naming: the nav says **Pull requests**; the page still explains that a pull request is a *proposal*
that combines one design option into another. The API and database keep `proposals`.

## 4. Preview (sidebar: Current / Next / Files)

The Preview tab is the home of the files-first diff direction, not a second full BIM viewer.

- **Current**: a link to the existing 3D diff (Compare) for small projects, with the measured
  limitation (two full IFCs, one mesh per element).
- **Next**: the render set from N51/N52 — changed and selected elements only; linked models and
  libraries as proxies or pinned snapshots.
- **Files**: downloads for the latest version (IFC, `.nic`, source zip, BCF).

When the diff bundle exists, the Preview tab becomes the 3D + plan diff page and the link to the old
Compare is removed.

## 5. CI (sidebar: Runs / Checks / Schedules / Budgets)

The CI tab gives engine checks a home. Today required checks appear only inside a proposal.

### Contract

| Method | Path | Purpose |
|---|---|---|
| `GET` | `/api/projects/{owner}/{slug}/ci/runs?option=&limit=` | Check jobs per version/option |
| `GET` | `/api/projects/{owner}/{slug}/ci/runs/{id}` | One run: kind, commit, status, duration, report |
| `GET` | `/api/projects/{owner}/{slug}/ci/checks` | Required checks per protected option and their last result |
| `GET` | `/api/projects/{owner}/{slug}/ci/report/{id}.html` | Stored report (IDS/clash/model) |

- A run is the existing `jobs` row of kind `checks`, with `args.rev`, `result.passed`, `result.report`.
- The page lists commit, option, checks (schema/IDS/clash/model), pass/fail, duration and a report link.
- A required check that fails blocks a Combine (already enforced); the CI page shows the same result.
- Schedules (nightly checks) and budgets are placeholders until Actions get their own runner.
- No new execution model: CI displays what Actions already run; the runner stays out of scope.

## 6. Renders (sidebar: Jobs / Presets / GPU usage)

The tab that owns the [GPU render service](2026-11-25-gpu-render-service.md).

- **Jobs**: queued/running/finished, recipe, version, GPU class, progress, actual cost, frames.
- **Presets**: the recipes a studio keeps; a recipe is a file, not a server-side blob.
- **GPU usage**: seconds and cost per account/project/job, from `render_gpu_seconds`.

No render is implemented yet. The CLI (`nicbim render`) is specified in the render service spec; the
web page will read the same API.

## 7. Sheets (sidebar: Sheets / Drawings and PDFs / Revisions)

This is the missing deliverables space. Sheets are **files generated by the engine**, not browser
drawings.

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

## 8. Packages (sidebar: Objects / Libraries / Rule packs)

The object-library and package registry space.

- **Objects**: `.nicobj` definitions and versions, scoped to an account, public or private.
- **Libraries**: office libraries and IFC type libraries, pinned by content hash; a project version
  records which library hash it used, so the web can say `library_pinned` or `library_missing`.
- **Rule packs**: IDS/code/check packs a project can require.
- Backed by the package registry in [future work](../future-work.md) (not built). The tab exists so
  object libraries are not hidden inside settings; the first implementation can be a list of pinned
  library hashes and downloads, then grow into upload/versioning.

## 9. Agents (sidebar: Sessions / Tools / Approvals)

The agent space that NicBim already has locally, moved to the repository where a team can see it.

- **Sessions**: an agent session, the option it works in, transcript and status.
- **Tools**: the tool catalogue and the role each tool needs; read-only tools are safe, write tools
  go through a reviewable option.
- **Approvals**: the review bar for agent-made changes; a person accepts, rejects or combines.
- Not implemented in NicGit. The desktop app (NicBim docs/18) has the tool catalogue and session
  worktrees; the server-side session/tool contract is future work. No agent runs automatically on a
  push.

## 10. Configs (sidebar: General / People / Protected options / AI / Budgets / Webhooks / Audit / Delete)

The existing project settings, given the “more configs space” that was missing.

- **General**: description, visibility, default option. Built.
- **People**: project members and roles. Built.
- **Protected options**: approvals, allowed conflict modes, required checks, protected settings. Built.
- **AI**: per-account, not per-project; the section explains this and links to Account. Placeholder.
- **Budgets**: monthly Action/AI limits on the account; links to Account. Placeholder.
- **Webhooks**: outgoing notifications. Placeholder.
- **Audit log**: the server records it; the viewer is not built. Placeholder.
- **Delete**: typed confirmation. Built.

The sidebar is anchors on one page. New sections get an id and an entry; the page stays a single
scroll so a save does not throw away the other fields.

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

## 13. Mobile and small screens

- The tab bar scrolls horizontally; it never wraps into a second row.
- A contextual sidebar becomes a horizontal sub-nav above the content.
- The Code sub-nav scrolls horizontally; it does not become a drawer.
- Tables keep their horizontal scroll; no card fallback that changes the information.

## 14. Acceptance criteria

1. Every existing page is reachable from the tab/sub-nav; no dead link.
2. Pull requests and Issues show open counts; Configs is admin-only.
3. Code has no sidebar; Configs, CI, Renders, Sheets, Packages, Agents and Preview do.
4. Each sidebar link reaches a section with that id; the page scrolls and highlights the section.
5. Unbuilt tabs have no fake data: they describe the planned data and the spec.
6. A viewer cannot reach Configs; the API still returns 404/403.
7. The layout holds at 1,280 px and at 800 px; no horizontal page scroll outside tables.
8. The UI uses the existing `.schedule`, `.tag`, `.btn`, `.form-block` language (N55).

## 15. Implementation sequence

1. **Done:** tab bar, Code sub-nav, sidebars, Preview shell, Configs anchors, placeholder tabs,
   i18n EN/ES.
2. **Sheets:** `sheets`/`sheet_export` engine job, deliverables worker with Qt offscreen, API, page.
3. **CI:** read the existing check jobs, one run list, report downloads.
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
