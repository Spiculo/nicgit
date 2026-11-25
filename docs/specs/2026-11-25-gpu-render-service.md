# GPU render service: CLI recipes, per-second billing

**Status:** Proposed for owner approval · **Date:** 2026-11-25 · **Scope:** NicBim CLI, NicGit API,
GPU workers, metering · **Spec only, not implemented**

Users prepare a render job with the NicBim CLI, using their own recipe (cameras, lighting, materials,
samples, resolution, passes, post-processing). NicGit runs it on a GPU worker and bills the account
for the GPU time in seconds. The job is reproducible from a scene hash and a recipe hash, and the
outputs are ordinary files the user downloads.

The owner's words that drive this: *"I want them to be able to prepare stuff for rendering executing
a CLI (with custom stuff that differs from standard ones), we would have GPUs ready, and they would
pay for the process in seconds. This needs to be speced and ready, not implemented."*

---

## 1. Current state and the gap

NicBim already has:

- CPU image export and ray tracing (`D146`, `D147`, `D149`), sun animation and walkthrough video
  (`D249`) through ffmpeg.
- Saved views and cameras, materials, surfaces, sun settings and render settings in the project.
- A CLI and agent tools; a project version is a normal Git repository in NicGit.

It does **not** have:

- a GPU renderer;
- a render queue, job isolation, artifact storage or per-second billing;
- a recipe format that captures "custom stuff" beyond the standard preset;
- any way for a studio to use idle GPUs, its own or ours.

This spec defines the missing product and contracts. It deliberately does not choose the renderer
backend yet; that is decision **N54** below.

## 2. Product

**One sentence:** send a NicBim scene and a custom render recipe; get frames back; pay for the GPU
seconds used.

### Users and jobs

| User | Job |
|---|---|
| Architect | A 4K still for a client, a night view, a material study |
| Studio | A 10-second walkthrough, several camera angles, a batch of options |
| Consultant | Re-render one view after a change, without a render machine |
| Self-hosting studio | The same CLI against its own GPU worker, with its own data |

### Supported jobs (first release)

- Stills and camera sequences (frame range, fps) from a NicGit project **version**.
- A self-contained scene uploaded from a local NicBim project (for custom assets not yet pushed).
- Custom recipe: cameras, resolution, samples, denoise, sun/HDRI, material overrides, passes,
  color management, post (LUT/exposure), output format.
- Multiple frames in one job; one GPU per job; frames are independent.
- Cancel, retry, download a zip, progress and logs.

### Not in the first release

- Arbitrary scripts, plugins or third-party executables in a job.
- Real-time or interactive rendering.
- Multi-GPU frame splitting (later; frames already parallelize across jobs).
- Billing integration/invoices (usage and budgets only, as with Actions).
- Simulation, CFD, structural solving.

## 3. End-to-end flow

```text
NicBim (local)                      NicGit (server)                 GPU worker
  render recipe new/edit   ──┐
  render validate            │
  render estimate            ├─► POST /api/renders/estimate
  render submit ─────────────┘   POST /api/renders
                                 queue + budget check
                                 ├─ job queued ───────────────► claim (GPU class)
                                 │                              fetch scene/recipe
                                 │                              render frames
                                 │◄─ progress, logs, artifacts ─┤
  render status / download ◄─────┤ artifacts + usage events
```

`render submit` is non-blocking by default. `--wait` follows the job and downloads when it finishes.
The CLI can run in CI: `--json` prints machine-readable output and exit codes are stable.

## 4. CLI contract

Command group: **`nicbim render`** (the CLI already owns the project, cameras and materials).
It talks to NicGit with a personal access token, like `kimbim versions push`.

```bash
# A recipe is a JSON file the user owns and can diff/commit.
nicbim render recipe new client-night.json
nicbim render recipe validate client-night.json
nicbim render estimate --project studio/clinic --version main --recipe client-night.json
nicbim render submit   --project studio/clinic --version main --recipe client-night.json \
                       --out ./renders --wait --max-seconds 600
nicbim render submit   --scene ./clinic.nic --recipe client-night.json --upload
nicbim render status   <job-id> [--watch]
nicbim render logs     <job-id>
nicbim render download <job-id> --out ./renders
nicbim render cancel   <job-id>
nicbim render list     [--project studio/clinic]
```

| Flag | Meaning |
|---|---|
| `--project`, `--version` | Render a version already on NicGit (no scene upload) |
| `--scene` | Self-contained `.nic`/IFC upload (custom local assets allowed) |
| `--recipe` | Required; the custom settings |
| `--out` | Where frames are written |
| `--wait` | Follow until done and download |
| `--max-seconds` | Hard stop for this job; the server refuses a longer estimate unless confirmed |
| `--priority` | `normal` (default) or `high` (higher rate, if offered) |
| `--json` | Stable JSON to stdout; progress to stderr |
| `--dry-run` | Validate, estimate and show what would upload; no job |

Environment: `NICGIT_URL`, `NICGIT_TOKEN`. Config: `~/.config/nicbim/render.json`.
Exit codes: `0` success, `2` invalid recipe/scene, `3` budget/estimate refused, `4` auth,
`5` job failed, `6` cancelled, `7` network/server.

## 5. Recipe format

The recipe is the "custom stuff". It is versioned, validated and hashed; unknown keys are rejected
so a job can never smuggle arbitrary behavior.

```json
{
  "version": 1,
  "name": "client-night",
  "engine": "cycles",
  "gpu_class": "auto",
  "resolution": { "width": 3840, "height": 2160, "percent": 100 },
  "samples": 512,
  "denoise": true,
  "frames": { "mode": "still" },
  "camera": { "source": "view", "name": "Camera 01" },
  "lighting": {
    "preset": "night",
    "sun": { "azimuth": 310, "altitude": 8, "strength": 1.2 },
    "environment": { "asset": "sky-dusk.hdr", "strength": 0.8 }
  },
  "materials": {
    "overrides": [
      { "guid": "1pAT02IgP9TebeDcVbf6za", "base_color": [0.62, 0.61, 0.58], "roughness": 0.7 }
    ]
  },
  "passes": ["beauty", "ao", "depth"],
  "color": { "look": "AgX - Medium High Contrast", "exposure": 0.3 },
  "post": { "lut": "client.cube", "bloom": 0.1 },
  "output": { "format": "exr", "bit_depth": 16, "prefix": "clinic" },
  "engine_settings": { "max_bounces": 12, "clamp_indirect": 8.0 },
  "assets": [
    { "path": "textures/wood-floor.exr", "sha256": "...", "bytes": 1843200 }
  ]
}
```

Frame modes:

```json
{ "mode": "range", "start": 1, "end": 120, "step": 1, "fps": 24 }
{ "mode": "cameras", "items": [
  { "name": "Approach", "eye": [12, 1.6, 30], "target": [0, 1.6, 0], "up": [0, 1, 0], "fov": 45 }
] }
```

Rules:

- `engine_settings` is validated against a per-backend allowlist (type, range). A recipe with an
  unknown key is rejected before upload.
- Materials/cameras refer to element GUIDs, view names or explicit coordinates; they do not contain
  code.
- Assets are hashed and uploaded; the server stores them under the job prefix and deletes them with
  the job.
- `standard` recipes ship as examples; a user's recipe is a normal JSON file that can be committed.
- The recipe hash covers the canonical JSON, the scene hash, the backend version and the engine
  commit. A job records all four.

## 6. API

All routes are authenticated with a PAT (or session); project access is checked like every other
NicGit route. Sizes are limited; uploads use signed URLs, not a giant multipart request.

| Method | Path | Purpose |
|---|---|---|
| `POST` | `/api/renders/estimate` | Validate, estimate GPU seconds and cost |
| `POST` | `/api/renders` | Create a draft job; returns upload targets or a project reference |
| `POST` | `/api/renders/{id}/files` | Signed upload target for scene/assets (hosted storage) |
| `POST` | `/api/renders/{id}/start` | Confirm the estimate and queue |
| `GET` | `/api/renders/{id}` | State, progress, estimate, actual cost, error |
| `GET` | `/api/renders/{id}/events` | SSE: progress, frame, log line, state |
| `POST` | `/api/renders/{id}/cancel` | Stop; bill only GPU time used |
| `GET` | `/api/renders/{id}/artifacts` | Manifest of outputs |
| `GET` | `/api/renders/{id}/artifacts/{path}` | Signed download |
| `GET` | `/api/render-prices` | GPU classes, rates, minimum, regions |

### Create

```json
{
  "project": "studio/clinic",
  "version": "main",
  "recipe": { "...": "the validated recipe, or a hash + upload" },
  "asset_manifest": [{ "path": "sky-dusk.hdr", "sha256": "...", "bytes": 2048000 }],
  "idempotency_key": "client-night-main-2026-11-25",
  "max_seconds": 600,
  "priority": "normal"
}
```

The server returns `job_id`, `state: draft`, `estimate`, `upload` targets, and `expires_at`. Starting
without confirming an estimate above the account threshold returns `402 estimate_confirmation`.
Idempotency: the same key and hashes return the existing job, never a double charge.

## 7. Job state machine

```text
draft ──start──► queued ──claim──► running ──► succeeded
  │                │                  ├────► failed
  │                └──cancel──────────┼────► cancelled
  └──expire───────────────────────────┴────► expired
```

- `draft` holds uploads; it expires (24 h) and deletes them.
- `queued` is not billed.
- `running` is billed from container start to container stop, per GPU class.
- `succeeded` artifacts are retained (default 30 days hosted, configurable self-hosted).
- `failed`: billed only for GPU time actually used; a failure caused by the service (worker crash,
  spot eviction) is retried once and not billed again.
- `cancelled`: billed for time used; frames already produced are kept.
- Retries preserve the same job id and `attempt` counter; no double billing.

## 8. Billing, budgets, usage

**The unit is the GPU-second.** Prices are integer micro-USD per second, per GPU class and region;
the price snapshot is stored on the job so a later price change cannot rewrite history.

| GPU class | Memory | Use | Rate (placeholder) | Minimum |
|---|---|---|---|---|
| `gpu-small` | 16 GB | Stills, low samples | $0.00035/s | 10 s |
| `gpu-medium` | 24 GB | 4K stills, short sequences | $0.00060/s | 10 s |
| `gpu-large` | 48 GB | Heavy scenes, many samples | $0.00120/s | 10 s |

- `billable_seconds = max(min_seconds, ceil(gpu_active_seconds))`.
- Estimate = backend throughput table × pixels × samples × frames × scene factor, rounded up; shown
  as a range (`±30%`) until the backend has calibration data, then a learned factor per recipe shape.
- On start, the estimate is **reserved** from the account's hard budget; on finish the actual is
  charged and the difference released.
- Hard budget: `start` returns `402 budget_exceeded`; a running job is cancelled when the budget is
  exhausted (frames kept).
- Soft budget: warning event and CLI warning; the job continues.
- New `usage_events` kinds: `render_gpu_seconds`, `render_storage_bytes`, `render_egress_bytes`.
  The existing metering, budgets and account pages display them.
- A failed/cancelled job shows the exact seconds and cost; the CLI prints them.
- Invoices/payment are out of scope; usage is the source for later billing.

## 9. GPU workers

### Hosted

- One container per job, one GPU, no network, read-only scene, scratch disk, memory/time limits.
- The worker image carries the chosen render backend, the NicBim scene converter and ffmpeg.
- The supervisor claims with a pool token; the job gets a job token, exactly like the engine worker
  (N30). The job can fetch only its own scene/assets and upload only its own artifacts.
- Heartbeat every 30 s; a lost worker's job is requeued once, then failed.
- Cancel is checked between frames/tiles; the container is stopped at the deadline.
- Outputs are uploaded with hashes; the server verifies size/hash and records them.
- A GPU worker reports its class, driver, backend version and free memory on claim, so a job is not
  placed on a class it cannot run.

### Self-hosted

- A `nicgit-render-worker` binary/container joins a studio's NicGit with a join token.
- The studio selects GPU classes and concurrency; jobs stay on its machines.
- Usage is recorded on its server; reporting to us depends on N43 (open). No data leaves the studio.
- The same recipe and API, so a job can move between hosted and self-hosted only by changing
  `NICGIT_URL`.

### Isolation

A scene is untrusted. The worker has no host mounts, no outbound network, a read-only root, a
writable scratch folder, no capabilities and a GPU memory limit. The renderer parses files, so assume
a crafted scene can take over the job: it must not reach another job, the database or the host.
Stronger separation (MIG, gVisor, separate VMs) is required before arbitrary customer assets on the
hosted service, as with engine workers (N30).

## 10. Renderer backend (decision N54)

The job contract is backend-agnostic. The first backend must support GPU rendering, cameras,
instances, textures, HDR environment, denoise and EXR/PNG.

| Option | For | Against |
|---|---|---|
| **A. Blender Cycles worker** (recommended for a first service) | GPU, mature, custom materials/lights, EXR, denoise, camera animation; open source | A converter from NicBim/IFC to the render scene; material fidelity needs work; large image |
| **B. GPU port of the NicBim path tracer** | Exact NicBim look; one engine; no conversion | A large engineering project; GPU backends (CUDA/Metal/Vulkan) are platform-specific |
| **C. Commercial renderer** (Octane/Redshift/V-Ray) | Quality/speed, familiar to visualisers | Licence, per-node cost, redistribution, data terms, closed source |

Recommendation: **A for the first paid service**, with the NicBim engine producing the scene, and
**B** as the long-term accurate path if the owner wants pixel parity with the desktop app. Keep the
recipe's `engine` field so a job can choose a backend; do not promise identical pixels across
backends.

## 11. Data model

```sql
CREATE TABLE render_jobs (
  id uuid PRIMARY KEY,
  project_id uuid NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  account_id uuid NOT NULL REFERENCES accounts(id),
  created_by uuid NOT NULL REFERENCES accounts(id),
  version text,
  scene_hash text NOT NULL,
  recipe jsonb NOT NULL,
  recipe_hash text NOT NULL,
  backend text NOT NULL,
  gpu_class text NOT NULL,
  state text NOT NULL CHECK (state IN ('draft','queued','running','succeeded','failed','cancelled','expired')),
  estimate_seconds integer NOT NULL,
  estimate_micros bigint NOT NULL,
  reserved_micros bigint NOT NULL DEFAULT 0,
  gpu_seconds numeric(12,3),
  cost_micros bigint,
  price_snapshot jsonb,
  priority text NOT NULL DEFAULT 'normal',
  idempotency_key text,
  attempts integer NOT NULL DEFAULT 0,
  error text,
  created_at timestamptz NOT NULL DEFAULT now(),
  started_at timestamptz,
  finished_at timestamptz,
  retention_until timestamptz
);
CREATE UNIQUE INDEX render_jobs_idem ON render_jobs (account_id, idempotency_key) WHERE idempotency_key IS NOT NULL;

CREATE TABLE render_files (
  id uuid PRIMARY KEY, job_id uuid NOT NULL REFERENCES render_jobs(id) ON DELETE CASCADE,
  path text NOT NULL, bytes bigint NOT NULL, sha256 text NOT NULL,
  content_type text, created_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE (job_id, path)
);
CREATE TABLE render_artifacts (
  id uuid PRIMARY KEY, job_id uuid NOT NULL REFERENCES render_jobs(id) ON DELETE CASCADE,
  path text NOT NULL, bytes bigint NOT NULL, sha256 text NOT NULL,
  content_type text, created_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE (job_id, path)
);
CREATE TABLE render_events (
  id bigserial PRIMARY KEY, job_id uuid NOT NULL REFERENCES render_jobs(id) ON DELETE CASCADE,
  at timestamptz NOT NULL DEFAULT now(), kind text NOT NULL, message text, data jsonb
);
CREATE TABLE render_prices (
  gpu_class text NOT NULL, region text NOT NULL, micros_per_second bigint NOT NULL,
  min_seconds integer NOT NULL, valid_from timestamptz NOT NULL,
  PRIMARY KEY (gpu_class, region, valid_from)
);
```

Storage: scene/assets and artifacts live in object storage (S3-compatible), keyed by account and
job; the database holds metadata and hashes. Self-hosted uses the `data` volume until S3 is added.

## 12. Security and abuse

- Recipe validation rejects unknown fields and unsupported backend settings; no code, no scripts.
- Asset allowlist: images/HDR/EXR/cube LUTs/textures; max file and job sizes; magic-byte checks.
- Outputs are downloaded through signed, expiring URLs; no directory traversal.
- Rate limits per account; max concurrent jobs; max frames/resolution; GPU quota for new accounts.
- Budgets and estimates stop runaway spend; a hard budget always wins.
- Data retention: job inputs and outputs deleted after retention; a user can delete immediately.
- Hosted service: no customer scene is used for training; logs keep hashes/paths, not pixels.
- Self-hosted: no data leaves the studio; metering is local unless the owner chooses otherwise (N43).

## 13. Acceptance criteria

1. Clinic still, 4K, 256 samples, custom night recipe: estimate shown, job submitted, GPU runs,
   frames downloaded, `render_gpu_seconds` recorded; actual cost = rate × billed seconds.
2. Five-second animation (120 frames) runs; cancel at frame 40 stops billing and keeps frames 1–39.
3. Retry after a worker crash does not double-charge.
4. Hard budget: start is refused; a running job stops at the budget and says why.
5. Recipe with an unknown `engine_settings` key is rejected before upload.
6. Self-hosted worker joins and renders a job with no data leaving the studio.
7. A job's manifest records scene hash, recipe hash, backend version, engine commit, GPU class and
   price snapshot; the same inputs reproduce the job record.
8. No arbitrary code execution path exists in the recipe, assets or CLI.
9. The CLI `--json` output and exit codes are stable; CI can submit and download.

## 14. Open questions (owner)

1. **Backend**: Cycles, a GPU port of the NicBim path tracer, or a commercial renderer? (N54)
2. **GPU provider and regions**: hosted on which cloud, which GPU classes, which regions?
3. **Pricing**: the placeholder rates, minimum charge and currency; is per-second billing prepaid,
   monthly, or both?
4. **Self-hosted metering**: does a studio's own GPU usage report to us, need a licence, or neither?
   (N43)
5. **Retention**: 30 days hosted? Can a studio configure it? Are renders private forever?
6. **Output**: EXR + PNG + MP4? Which passes and color looks are first-class?
7. **Custom stuff**: is the recipe enough, or do users need custom shaders/plugins? (Recommend no
   arbitrary code; add a reviewed plugin list later.)
8. **Priority/spot**: offer a cheaper interruptible class and a high-priority class?
9. **Data residency**: must EU studios keep scenes and frames in the EU?
10. **Brand**: "NicGit Render" vs "NicBim Render" in the UI and CLI.

## 15. Implementation plan (after approval)

1. **Decide** N54 backend, GPU provider, prices and retention.
2. **Recipe/CLI**: schema, validation, local estimate, `render recipe/validate/estimate`,
   `--dry-run`, tests; no GPU yet.
3. **Server**: tables, create/start/cancel, signed uploads, queue, usage events, budgets; workers
   mocked in tests.
4. **First GPU backend**: image, converter, still frame on Clinic, progress/cancel, artifacts.
5. **Billing**: price snapshot, reserve/charge, estimate calibration, account display.
6. **Self-hosted worker**: join token, claim protocol, local metering.
7. **UI/CLI polish**: job list, logs, artifact browser, download all; `nicbim render list`.

## 16. Risks

- **Material fidelity**: a converter may not match NicBim's look; set expectations and offer a
  "NicBim look" backend later.
- **GPU cost/abuse**: estimates can be wrong; reserve budgets, cap frames, monitor actual vs
  estimate, and start with invited accounts.
- **Scene size**: a `.nic` with textures can be GBs; signed uploads, resumable uploads and hashing
  are required.
- **Backend lock-in**: keep the job format backend-neutral and store the backend/version.
- **Trust**: a render is not a compliance deliverable; keep the "design aid" wording.
