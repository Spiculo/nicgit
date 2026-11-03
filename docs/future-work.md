# NicGit — future work

Things that are deliberately not in the first build, with what they depend on. Items marked **(kimcad)** are engine or desktop app changes made in `../kimcad`, not in NicGit.

## Linked `.kim` projects (kimcad)

**Problem.** A `.kim` is a zip of the whole project folder, including `.git` with all history (kimcad D72). That is a good offline and archive format, but once NicGit is the main way people work:

1. **It only grows.** Every Ctrl+S re-zips the full history. A large, long-lived project becomes a multi-GB file that cloud-sync tools re-upload on every save, and they produce conflicting copies.
2. **Two sources of truth.** A full `.kim` and the NicGit project both claim to be the project, so stale copies get emailed and work gets lost.
3. **No partial access.** Clients and consultants must take the whole history to see one version.
4. **The lock is local.** `<file>.kim.lock` means nothing across cloud sync or offices.

**Direction.** Keep `.kim` as the offline/archive format and add a **linked project** mode in the app:

- A project connected to NicGit keeps its repository in an app-managed local cache (a clone; partial clone where useful).
- A *linked* `.kim` is small: the manifest, the remote URL, the option, and optionally the working copy. Opening it fetches what is needed.
- "Save version" pushes when online (or queues). "Export full `.kim`" still makes the self-contained file.
- Consider packing the history inside a full `.kim` as a single git bundle/packfile instead of a `.git` folder.
- Locks become server-side "who is working on which option" presence in NicGit (advisory).

NicGit already supports the server side from phase 5 (`.kim` import, and export as full / latest only / one option).

## Engine paths in Rust (kimcad)

Move into `kimbim-repo`, in order of server benefit: content digests, `diff_details`, merge preview and record merge, IFC export from records. Each one moved shrinks the Python worker pool. Golden tests must keep the same hashes.

## Engine on bare repositories (kimcad), needed in phase 2

`Project` assumes a project folder. Add read, diff and merge on a bare repository (or a worktree over it, cf. D111 `Repo::open_worktree`), plus a Combine API that accepts whole-side resolutions and settings-key resolutions.

## Settings schema (kimcad)

A declared schema of `settings.json` keys (type, label EN/ES, "affects deliverables" flag), so NicGit can show settings diffs in plain language and derive the protected keys list instead of hard-coding it.

## Deliverables: PDF/DXF sheet sets per milestone

The app exports sheets headlessly (`kimbim-app --export-sheet`, `versions export-dxf`). A deliverables worker image with Qt offscreen produces permit and tender sets from a milestone's own settings, stores them as immutable artifacts with a transmittal record, and supports revision clouds/registers later.

## Package registry (revenue line)

Hosting for NicBim packages: parametric objects (`.kimobj`), office libraries, IDS rule packs, title-block templates, attribute sets (pens, materials, surfaces), scripts/plugins once the app has a plugin system. Versioned, scoped per account, public or private, metered storage and transfer. Needs a package format and an install flow in the app (kimcad).

## Actions beyond checks

Customer workflows defined in the project (file location inside the tree to be agreed with the engine's layout rules): clash runs, exports, quantity reports, scripts. Runner isolation stronger than containers (gVisor/Firecracker) before running customer code on the hosted service.

## Platform

- SSH git transport; OIDC/SAML SSO for offices; LDAP for on-prem.
- Billing integration on top of `usage_events`.
- Webhooks and a public REST API with tokens; NicGit MCP endpoint for agents (open proposals, read reviews).
- App integration: "Open in NicGit", proposals and reviews inside the desktop app, sign in once.
- Federation of discipline models (kimcad `docs/08`), real-time co-editing presence.
- Separate infra repository once `deploy/` grows.
- Mirroring to/from other git hosts.
