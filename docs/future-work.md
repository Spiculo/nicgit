# NicGit — future work

Things that are deliberately not in the first build, with what they depend on. Items marked **(nicbim)** are engine or desktop app changes made in `../nicbim`, not in NicGit.

## Linked `.nic` projects (nicbim)

**Problem.** A `.nic` is a zip of the whole project folder, including `.git` with all history (nicbim D72). That is a good offline and archive format, but once NicGit is the main way people work:

1. **It only grows.** Every Ctrl+S re-zips the full history. A large, long-lived project becomes a multi-GB file that cloud-sync tools re-upload on every save, and they produce conflicting copies.
2. **Two sources of truth.** A full `.nic` and the NicGit project both claim to be the project, so stale copies get emailed and work gets lost.
3. **No partial access.** Clients and consultants must take the whole history to see one version.
4. **The lock is local.** `<file>.nic.lock` means nothing across cloud sync or offices.

**Direction.** Keep `.nic` as the offline/archive format and add a **linked project** mode in the app:

- A project connected to NicGit keeps its repository in an app-managed local cache (a clone; partial clone where useful).
- A *linked* `.nic` is small: the manifest, the remote URL, the option, and optionally the working copy. Opening it fetches what is needed.
- "Save version" pushes when online (or queues). "Export full `.nic`" still makes the self-contained file.
- Consider packing the history inside a full `.nic` as a single git bundle/packfile instead of a `.git` folder.
- Locks become server-side "who is working on which option" presence in NicGit (advisory).

NicGit already supports the server side from phase 5 (`.nic` import, and export as full / latest only / one option).

## Engine paths in Rust (nicbim)

Move into `kimbim-repo`, in order of server benefit: content digests, `diff_details`, merge preview and record merge, IFC export from records. Each one moved shrinks the Python worker pool. Golden tests must keep the same hashes.

## Engine on bare repositories (nicbim), needed in phase 2

`Project` assumes a project folder. Add read, diff and merge on a bare repository (or a worktree over it, cf. D111 `Repo::open_worktree`), plus a Combine API that accepts whole-side resolutions and settings-key resolutions.

## Settings resolutions in the engine (nicbim)

`Project.merge` keeps the target's value for every settings conflict. NicGit's worker repeats the merge's last steps to apply "take the source" for chosen keys (decision N18). Accept `settings_resolutions` in `merge_preview`/`merge` and drop the worker's copy.

## A slim engine environment (nicbim)

The worker image uses nicbim's whole default pixi environment (Qt, CMake, compilers included). An `engine` environment with only Python, IfcOpenShell, the Rust core and the check dependencies would cut the image and start-up time a lot.

## Settings schema (nicbim)

A declared schema of `settings.json` keys (type, label EN/ES, "affects deliverables" flag), so NicGit can show settings diffs in plain language and derive the protected keys list instead of hard-coding it.

## Deliverables: PDF/DXF sheet sets per milestone

The app exports sheets headlessly (`kimbim-app --export-sheet`, `versions export-dxf`). A deliverables worker image with Qt offscreen produces permit and tender sets from a milestone's own settings, stores them as immutable artifacts with a transmittal record, and supports revision clouds/registers later.

## Package registry (revenue line)

Hosting for NicBim packages: parametric objects (`.nicobj`), office libraries, IDS rule packs, title-block templates, attribute sets (pens, materials, surfaces), scripts/plugins once the app has a plugin system. Versioned, scoped per account, public or private, metered storage and transfer. Needs a package format and an install flow in the app (nicbim).

## Actions beyond checks

Customer workflows defined in the project (file location inside the tree to be agreed with the engine's layout rules): clash runs, exports, quantity reports, scripts. Runner isolation stronger than containers (gVisor/Firecracker) before running customer code on the hosted service.

## Not built yet in NicGit (from the spec)

CI is now built from the existing checks jobs (decision N63); the rest below is unchanged.

- **Ask the model**: questions about a version answered with the engine's read-only agent tools (needs an inventory job and tool calls through the router).
- **AI version summaries** on the timeline (the proposal summary exists; versions show the engine's counts).
- **Postgres row-level security** as a second wall behind the per-route checks (the per-route checks and the isolation test exist).
- **Object storage (S3)** for caches and exports; today they live on the data volume.
- **Live job progress over websockets**; the web app polls every second.
- **Large models in the browser**: replaced in direction by [N51/N52](decisions.md) and the [files-first diff viewer](specs/2026-11-25-files-first-diff-viewer.md). The web app will download only the changed/selected elements as an engine-generated GLB + plan bundle; the full model stays in NicBim. Not implemented yet.
- **2FA (TOTP)** and **teams UI** (teams exist in the schema and in permissions, not in the API/UI yet).
- **Notifications by email** and **webhooks**.
- **Comments pinned in 3D** on proposals: comments accept a pin (elements, viewpoint) through the API; the web app does not place them yet.

## Sheets and deliverables (spec)

The Sheets tab lists a version's sheets and downloads PDF/SVG artifacts generated by a deliverables
worker; the browser never regenerates drawings. The contract is in
[repository navigation](specs/2026-11-25-repository-navigation.md#7-sheets-sidebar-sheets--drawings-and-pdfs--revisions)
(N58). Not implemented.

## GPU render service (spec)

A metered GPU render service prepared by the NicBim CLI, with custom recipes and per-second billing.
The implementation-ready spec is [specs/2026-11-25-gpu-render-service.md](specs/2026-11-25-gpu-render-service.md)
(decisions N53/N54). Not implemented.

## Two deployments (docs/deployment.md)

What is still missing for a studio to run its own server, and for the hosted service to grow, in rough order:

- **Published images**: a registry, a release workflow that runs the tests against `engine.rev` and pushes tagged images, signatures (N44).
- **Invitations**, so a closed server can admit one consultant without opening its domain list; an admin page for who joined.
- **Email by SMTP** on self-hosted servers next to Postmark on ours (N29).
- **OIDC sign-in** (Microsoft Entra, Google Workspace, Keycloak) for studios not on Google (N45).
- **Engine compatibility checks**: workers report their engine commit when claiming and the server refuses a mismatch; the desktop app reads `/api/instance` and warns when its record format differs.
- **Per-deployment limits**: `0` for "no limit" so a self-hosted server need not invent numbers.
- **Self-hosted licensing and usage reporting**, once N42/N43 are decided.
- **Hosted at scale**: Postgres row-level security (N9), object storage, a split git role (N32), Helm/Kubernetes manifests in the private infrastructure.

## Platform

- SSH git transport; OIDC/SAML SSO for offices; LDAP for on-prem.
- Billing integration on top of `usage_events`.
- Webhooks and a public REST API with tokens; NicGit MCP endpoint for agents (open proposals, read reviews).
- App integration: "Open in NicGit", proposals and reviews inside the desktop app, sign in once.
- Federation of discipline models (nicbim `docs/08`), real-time co-editing presence.
- The hosted service's private infrastructure repository (N41).
- Mirroring to/from other git hosts.
