# Files-first NicGit and selective diff rendering

**Status:** Proposed for owner approval · **Date:** 2026-11-25 · **Scope:** NicGit web app and one engine job · **Not implemented**

This replaces the assumption that the browser must show the whole building. NicGit becomes a
file-and-review portal. The browser renders **only the elements that need review**: the changed
elements of a comparison, plus anything the reviewer selects. The full model stays in NicBim, which
remains the accurate authoring and viewing tool.

The owner's words that drive this: *"I may want to keep it files only, with the diff only rendering
the parts that need to be rendered, in plan and 3d view, but only of clear or selected elements."*
The word **clear** needs one decision (see Open questions); this spec assumes it means **changed**,
with selected elements added.

---

## 1. Measured baseline (Clinic)

The Medical-Dental Clinic from the test corpus, imported through NicGit on this machine:

| Measure | Result |
|---|---|
| File | `Clinic_Architectural.ifc`, IFC2X3, **13.0 MB** |
| Products / walls / doors / windows / spaces | **3,298 / 1,080 / 254 / 58 / 269** |
| Import through the engine worker | **28.7 s**, 41,068 records, 22 MB repository |
| IFC export from records | **5.2 s**, 13 MB |
| web-ifc parse + mesh extraction (one model) | **258 ms**, 2,586 elements, 4,896 geometries, 422,719 vertices, **174,325 triangles**, **214 MB RSS** |
| 100k synthetic model (one model) | 92 MB IFC, **100,000 elements**, 1,400,000 triangles, **2.46 s** parse + extraction, **495 MB RSS** (Node; three.js adds more) |

The browser's Compare page loads **two full IFCs** (`from` as ghost and `to` as the model), so a
Clinic comparison is roughly 2× the parse, 2× the memory, 2,586+ three.js meshes and draw calls, plus
GPU buffers. The Overview and Versions pages load the full model again.

The 100k parse itself is not the wall: 2.46 s and 495 MB in Node is tolerable for a one-off. The wall
is the **scene graph**: one mesh and one draw call per element, 100,000 of them, twice for a
comparison, plus materials, GPU buffers and UI updates. That is what lags and what a laptop or an
integrated GPU cannot sustain. Clinic is still usable; 100k is not a browser scene.

The problem is:

1. **The full model is downloaded and built** even when the reviewer looks at one changed wall.
2. **One mesh and one draw call per element**, so a large model is a large scene graph.
3. **Two full models** for every comparison.
4. **A second, less accurate model engine** in the browser. Reviewers cannot tell which engine is
   authoritative, and the web viewer will always lag the desktop app.
5. **Download and memory cost** for clients and consultants who need to look at one changed wall.

## 2. The decision to approve

**NicGit is files-first.** The web app provides files, versions, diffs, reviews, issues and history.
It does not become a general BIM viewer.

**The browser renders a RenderSet.** For a comparison, that set is:

- every element the engine reports as **added, modified or removed**;
- plus every element the reviewer **selects** from the change list, plan or search;
- optionally, a light **context** of unchanged elements (boxes, not full geometry).

The full model is available as a file: IFC, `.nic`, source archive and sheets. "Open in NicBim" is
the accurate path. A full browser viewer remains an **opt-in** mode for small models on self-hosted
servers (`NICGIT_FULL_VIEWER=off|small|on`, default `off`).

### Terms

| Term | Meaning |
|---|---|
| **Changed set** | Elements whose record differs between `from` and `to`, as the engine's `diff_details` reports |
| **Selected set** | Elements the reviewer picks; always rendered |
| **RenderSet** | Changed set ∪ selected set (the default), optionally with context |
| **Clear** | Assumed **changed** for this spec; see Open questions |
| **Context** | Optional unchanged elements as boxes or low-poly, off by default |
| **Diff bundle** | The server-generated zip with the manifest, small GLB(s) and plan JSON |
| **Plan view** | 2D polylines/fills for the RenderSet, not the whole floor plan |

## 3. Data flow

```text
NicBim engine worker (NicGit)
  diff_details(from, to)                    changed GUIDs + field changes
  export_subset(rev, GUIDs)                 GLB of just those elements
  plan_subset(from, to, GUIDs, cutoff)      plan polylines/fills of the RenderSet
  -> diff-bundle.zip                        manifest.json + model.glb + plan.json + removed.glb

Browser
  fetch bundle -> GLTFLoader -> three.js    changed/selected elements only
  plan.json -> canvas/SVG                   changed/selected plan geometry
  no web-ifc, no full IFC in the default path
```

The engine remains the only authority for records, geometry, plan cuts and merges. The browser is a
viewer of a prepared bundle, never a second model engine.

## 3.1 External dependencies: linked models, object libraries, assets

A version is **not automatically self-contained**. It contains the project records, settings and the
files the project copied into `files/`; it does not contain a linked IFC, a hotlinked project, an
office library `.nicobj` or an external texture. The web must not pretend otherwise.

| Dependency | What the repository stores | Browser default | Opt-in |
|---|---|---|---|
| Linked IFC / project / hotlink | The link record (`KIMBIM_Link`): path, transform, kind, source hash if present | **Proxy/bbox + status badge**, not the linked geometry | Pinned, read-only snapshot baked at a known hash (size/licence limits) |
| Copy/monitor elements | The copied elements are normal records; the monitor link is a record | Copied geometry renders normally; source changes are a dependency event | Source snapshot |
| Office library `.nicobj` / IFC type | The placed object's evaluated representation + a library reference/hash | **Baked placed geometry**; definition not executed | Library definition download |
| Bundled library | The app version; content hash | Baked placed geometry | — |
| External texture / underlay / HDR | The path and, if copied, a `files/` entry | Only if the asset is embedded; otherwise a material warning | Pinned asset upload |
| Sheets / deliverables | The version settings + embedded assets | Generated as files by the engine, never by the browser | — |

### Bundle manifest addition

```json
"dependencies": {
  "links": [
    { "guid": "2x9...", "path": "structure.ifc", "kind": "ifc",
      "sha256": "…", "status": "resolved|missing|changed|excluded",
      "mode": "proxy|baked" }
  ],
  "libraries": [
    { "ref": "office:stool", "definition_sha256": "…",
      "source": "bundled|office|project", "status": "resolved|missing" }
  ],
  "files": [
    { "path": "textures/wood.exr", "sha256": "…", "status": "resolved|missing" }
  ]
}
```

`status` is never guessed: `resolved` requires a hash match, `changed` means the recorded hash and the
available source differ, `missing` means the file/definition is not available, `excluded` means the
mode is `proxy`.

### Rules

1. **The browser never resolves a path.** It only consumes the bundle. Resolution happens in the
   engine/bundle job, which has access to the project's repository and the server's pinned library
   snapshot. No `../`, no absolute host paths, no URL fetch from an element record.
2. **Proxy is the default** for linked models. A reviewer sees the link's bbox/placeholder, its
   transform, its status and a "linked model changed/not included" badge. The linked building is not
   silently rendered stale.
3. **Baking is an explicit opt-in** (`include_links=true` on a bundle request). The snapshot is
   read-only, size-limited, stored as an artifact keyed by source hash, and labelled "snapshot at
   <hash>". It is not a new source of truth and never writes back.
4. **Object libraries are baked, not executed.** Placed objects render from their evaluated
   representation; the `.nicobj` definition is not run in the browser or on the server as code. The
   manifest records the definition hash and parameter values. A definition is declarative data, but
   treat a crafted one as untrusted input.
5. **Missing dependencies fail visibly.** `library_missing` and `link_missing` are badges and a list
   entry; the affected element gets a bbox, not a plausible but wrong mesh. A bundle never silently
   drops a dependency.
6. **Sheets and renders use the same pins.** A sheet or render job that needs a missing link or
   library fails with `dependency_missing` rather than producing an incomplete deliverable.
7. **The full `.nic` remains the portable archive.** A `.nic` that includes the working copy and the
   embedded files is the way to move a project offline; the web bundle is a review aid.

### Diff semantics

- A link record change is **one dependency change**, not a geometry diff of the linked model. The
  change list says "linked model changed" with the old/new hash/path.
- A library definition change lists the placements that use it. If both sides can be evaluated, the
  bundle compares the baked geometry and reports a real geometry change; if not, it reports
  `library_changed` and a placeholder.
- A copied/monitored element is a normal record and diffs normally; the monitor link is a dependency
  event.
- An external texture change is a material/dependency event; if the texture is not embedded, the
  bundle marks the material as `texture_missing` rather than showing the wrong colour.

### Acceptance criteria additions

- A project with a linked IFC: default bundle contains a link proxy and `status: excluded`; no linked
  geometry; the full IFC download still has the link record (not the source).
- With `include_links=true`: the snapshot is pinned by hash, labelled, and size-limited; a missing
  source returns `link_missing`.
- A placed office-library object: geometry is baked; the `.nicobj` definition is not executed; a
  missing definition returns `library_missing` with a bbox.
- A changed library definition with both sides resolvable: the affected placements are listed and
  compared; without both sides, the result is `library_changed`.
- No request from the browser contains a filesystem path from a record.

## 4. Diff bundle contract

One zip, content-addressed by `(project, from, to, cutoff, context, engine commit, bundle version)`.

```text
diff-bundle.zip
  manifest.json
  model.glb          # `to` elements: added and modified
  removed.glb        # `from` geometry for removed elements (ghosts)
  plan.json          # the RenderSet in plan
  context.glb        # optional, boxes/low-poly of unchanged elements
```

### `manifest.json`

```json
{
  "version": 1,
  "project": "studio/clinic",
  "from": { "rev": "d50d8e1", "label": "main" },
  "to":   { "rev": "7a91c02", "label": "Scheme B" },
  "engine": { "commit": "603210a", "schema": "IFC2X3" },
  "units": "m",
  "cut_height": 1.2,
  "render_set": { "default": "changed", "context": false, "truncated": false },
  "counts": { "added": 2, "modified": 7, "removed": 1, "selected": 0 },
  "levels": [{ "guid": "...", "name": "Level 1", "elevation": 0.0 }],
  "bounds": [0, 0, 0, 42.0, 12.0, 18.0],
  "elements": [
    {
      "guid": "1pAT02IgP9TebeDcVbf6za",
      "ifc_class": "IfcWallStandardCase",
      "name": "Wall 102",
      "level": "Level 1",
      "kind": "modified",
      "geometry": "model.glb#1pAT02IgP9TebeDcVbf6za",
      "bbox": [1.2, 0.0, 0.0, 1.4, 6.0, 2.8],
      "fields": [
        { "name": "Pset_WallCommon.FireRating", "old": "EI30", "new": "EI60" }
      ]
    }
  ],
  "files": { "model": "model.glb", "removed": "removed.glb", "plan": "plan.json", "context": "context.glb" }
}
```

Rules:

- A GLB node carries `extras.guid`, `extras.kind` and `extras.ifc_class`; one node per element (or one
  merged primitive per material, with a `guid` attribute) so selection is exact.
- A **property-only** change has no geometry; `bbox` lets the UI draw an outline/placeholder and the
  change list still explains the fields.
- A **removed** element is rendered from `removed.glb` as a red ghost; it is not part of the `to` model.
- Colors: added green, modified orange, removed red (the existing `colors.ts` palette).
- Materials are generated by the engine; no textures unless the source element has one and it is
  needed. Small, flat materials are preferred for review.
- The bundle is limited to the RenderSet; **no unchanged element geometry** unless `context=true`.

### `plan.json`

```json
{
  "version": 1,
  "cut_height": 1.2,
  "units": "m",
  "levels": [{ "guid": "...", "name": "Level 1", "elevation": 0.0 }],
  "polylines": [
    { "guid": "1pAT...", "kind": "modified", "points": [[1.2, 0.0], [1.4, 0.0], [1.4, 6.0]],
      "closed": false, "fill": null, "layer": "walls" }
  ]
}
```

Plan geometry comes from the engine's plan cut, not from projecting meshes in the browser. Door
swings, window symbols and fills are included only for the RenderSet. Grids and level outlines are
optional context, not full drawings.

## 5. Engine job: `diff_bundle`

New job kind in `workers/engine/nicgit_worker`:

```json
{
  "kind": "diff_bundle",
  "args": {
    "from": "main",
    "to": "Scheme B",
    "cut_height": 1.2,
    "context": false,
    "plan": true,
    "bundle_version": 1
  }
}
```

Steps:

1. Resolve both revisions with the engine, not the browser.
2. `diff_details(from, to)` gives changed GUIDs and field changes.
3. Export the `to` subset (added + modified) and the `from` subset (removed) as GLB.
4. Generate plan polylines/fills for the RenderSet at `cut_height` (per level if needed).
5. Write `manifest.json`, zip the files, return `{path, bytes, counts, truncated}`.
6. Cache by content key; the server trims old bundles like other cache entries.

Limits and failure modes:

- More than 5,000 changed elements: set `truncated=true`, generate a plan/list-only bundle and
  return per-level chunks on request. Do not build a 100 MB GLB.
- Unsupported geometry: write a bbox placeholder and a warning in the manifest.
- Engine failure: no bundle; the UI keeps the change list and offers full downloads.

**Engine API to add:** `Project.export_subset(paths, rev)` (or equivalent) and a plan-generation entry
point that accepts a GUID filter. Keep both in the engine, never in NicGit.

## 6. Server API and cache

| Method | Path | Result |
|---|---|---|
| `GET` | `/api/projects/{owner}/{slug}/diff-bundle?from=&to=&cut_height=1.2&context=0` | `202` job, or `200` zip |
| `GET` | `/api/jobs/{id}` | progress, result, error |
| `GET` | `/api/projects/{owner}/{slug}/versions/{rev}/model.ifc` | full IFC download (kept) |
| `GET` | `/api/projects/{owner}/{slug}/versions/{rev}/sheet/{name}.png` | static sheet preview (new) |
| `GET` | `/api/projects/{owner}/{slug}/element/{guid}/geometry?rev=` | optional single-element GLB (later) |

- Same project authorization as the other routes: 404 for anyone who cannot read the project.
- `diff-bundle` is a job with a content cache; repeat calls are a cache hit.
- `Content-Disposition` and `Content-Type: application/zip`; no directory listing.
- Store bundles under `data/cache/<project>/diffs/<key>.zip`; count them against the project cache
  allowance, trim the oldest first (existing `limits::trim_cache`).
- A self-hosted server can set `NICGIT_FULL_VIEWER=small` to keep the current viewer for projects
  under a small element count; the hosted default is `off`.

## 7. Web UI

### Compare

Three panes, all driven by one bundle:

1. **3D diff** — RenderSet only. Changed colours; removed ghosts. Click selects; list scrolls to the
   element.
2. **Plan diff** — the same RenderSet at the chosen cut height; level switch; no full floor plan
   unless context is on.
3. **Change list** — existing grouped list (level → class → element), field tables.

Controls: **Changed only** (default) / **Add context** (boxes) / **Add selection** / **Level** /
**Class** / **Download full IFC** / **Open in NicBim**.

### Overview and Versions

- No 3D by default. Show the latest preview image or sheet thumbnail, counts, schema, branches,
  downloads (IFC, `.nic`, source zip, BCF) and "Open in NicBim".
- Versions list keeps Compare buttons; a version opens the diff against its parent or the current
  option.

### Large diffs

- RenderSet > 5,000 or bundle > 50 MB: show plan + list first; offer "by level" and "by class"
  subsets; never block the page on a full-scene download.
- No WebGL or low memory: plan + list only; downloads still work.

## 8. Performance budgets

| Case | Budget |
|---|---|
| One changed wall in Clinic | < 1 s to first diff frame, < 30 MB JS heap |
| 100 changed elements | < 2 s, < 50 MB |
| 5,000 changed elements | < 5 s, < 150 MB, bundle < 50 MB |
| 100k model, 10 changed walls | < 2 s, bundle < 1 MB, no full-model request |
| Cache hit | < 300 ms |
| Full IFC parse in the browser | **never** in the default path |

These are review aids, not rendering benchmarks; the browser does not need 60 fps on the whole model.

## 9. Accuracy and trust

- Every geometry and plan line comes from the engine at the exact revisions being compared.
- The browser does not decompose, merge or re-export records.
- Property-only changes are shown as fields even when there is no geometry.
- The UI says "diff rendered from the engine; the full model opens in NicBim" where it matters.
- A missing/truncated bundle is visible; it never silently looks like the whole model.

## 10. Acceptance criteria

1. Clinic, one wall changed: the bundle has one element (+ selected); the browser never requests
   `model.ifc`; added/modified/removed colours and the plan outline match the engine.
2. Property-only change: no geometry, fields shown, selection draws the bbox.
3. Removed element: ghost from `from`; it disappears from the `to` plan.
4. Context toggle adds boxes only; off by default.
5. 100k model, 10 changed walls: bundle < 1 MB, first render < 2 s.
6. Level/class filters work; large diffs fall back to plan + list.
7. Full IFC, `.nic`, source zip and BCF downloads still work.
8. Engine and server tests cover the bundle; web tests cover the RenderSet, selection and fallbacks.

## 11. Implementation plan (after approval)

1. **Engine** (the long pole): `export_subset`, plan subset, `diff_bundle` job, Clinic test.
2. **Server**: endpoint, job, cache, limits, tests.
3. **Web**: GLB/plan renderer, Compare page, Overview/Versions cleanup.
4. **Feature flag**: `NICGIT_FULL_VIEWER=off|small|on`; default `off`, `small` for existing small
   projects during the transition.
5. **Measure**: Clinic and 100k; tune the 5,000/50 MB limits.

## 12. Open questions

1. **What does "clear" mean?** Changed, selected, changed-geometry-only, or unobscured? This spec
   assumes changed + selected. Owner answer needed.
2. **Is context ghosting needed?** Boxes only, per level, or no context at all?
3. **Plan cut height**: one project height, per level, or per view?
4. **Full viewer**: keep it opt-in for small self-hosted projects, or remove it entirely?
5. **Bundle format**: one GLB + plan JSON (recommended) or a custom binary pack?
6. **Sheets**: static PNG previews, PDFs, or both?
7. **Element-level downloads**: does a reviewer need a single element's geometry, or only the diff?
8. **Linked models**: is a proxy enough, or must reviewers see a pinned snapshot? If snapshot, what
   size/licence limit and who may create it?
9. **Libraries**: should a version require a `library.lock` (hashes of every office library used), or
   is a best-effort manifest enough for review?
10. **Textures and underlays**: embed them in the bundle for changed elements, or always show a
    material warning when external?
