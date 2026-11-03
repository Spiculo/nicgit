# NicGit decisions

Numbered `N<n>`. Status: *Decided (owner)*, *Decided (agent)* = Claude's call, open to revision, *Proposed*.

| # | Decision | Status | Where |
|---|---|---|---|
| N1 | **NicGit is one integrated forge** (git server + web app) built for NicBim. **Supersedes kimcad D33** ("Hub as an add-on on unmodified Forgejo"). Reasons: a generic forge's merge button runs git's text merge on records, which can corrupt a model; two UIs and auth systems feel like a developer tool; owner asked for a fully integrated server. The kimcad decision log should record that D33 is superseded by NicGit N1 | Decided (owner, 2026-11-03) | [spec](superpowers/specs/2026-11-03-nicgit-design.md) |
| N2 | **Hosted first, on-prem supported.** The owner runs the main hosted service; the same images deploy on premises. Everything private unless made public; customer data protection is a core requirement. Infra lives in `deploy/` until it gets its own repo | Decided (owner) | spec §5, §7 |
| N3 | **`.kim` is a transport format, not a versioned file.** NicGit versions NicBim project repositories. A `.kim` is imported (history kept) and exported (full / latest only / one option) but never stored as a file in git: it would nest a history inside a history, defeat object sharing, and give no useful diff | Decided (owner + agent) | spec §8, [future work](future-work.md#linked-kim-projects) |
| N4 | **`.ifc` files are viewable, not versioned as files.** An IFC can be imported into a project as a new version through the engine's reconciled import (keeps element history when GlobalIds/Tags match) | Decided (owner + agent) | spec §8 |
| N5 | **Stack: Rust server** (axum, tokio, sqlx, Postgres, `kimbim-repo` crate) for everything always-on, **Python engine workers** (kimbim + IfcOpenShell) for heavy BIM jobs, TypeScript/React SPA with That Open Engine. Owner priority: CPU and memory efficiency on limited hardware | Decided (owner: Rust; agent: the split) | spec §6 |
| N6 | **The browser never edits the model, its settings or conflict values.** Conflicts: automatic merge, then **whole-side** (target or source wins everywhere) or **granular** (per conflict, pick a side). Allowed modes, required approvals by count and role, required checks and **protected settings keys** are rules per protected option. An invalid result blocks Combine; the fix is made in the desktop app | Decided (owner) | spec §9 |
| N7 | **What counts as Actions (billable):** customer-configured checks and workflows. Platform jobs needed to display a project (geometry, IFC export for viewing, summaries, merge previews) are not billed as Actions | Decided (agent) | spec §12 |
| N8 | **Revenue lines shape the core:** metering and budgets for Actions, package hosting and AI (via the owner's cloud AI router) are built in, billing integration later. Certifications exist but are out of NicGit's scope | Decided (owner) | spec §12, §13 |
| N9 | **Postgres everywhere**, including on-prem (shipped in the compose file), instead of a SQLite option: one database to test, row-level security for tenant isolation, queue with `SKIP LOCKED` | Decided (agent) | spec §6, §7 |
| N10 | **License AGPL-3.0-or-later** (kimcad D08) | Decided | spec §5 |
| N11 | **Name: NicGit** (may change during market review; not a POC) | Decided (owner) | — |
| N12 | **No KimBim → NicBim rename in this repo.** NicGit reaches the engine through one adapter module per language | Decided (owner) | spec §3 |
