# Running NicGit

## Pieces

| Piece | What it does | Size it by |
|---|---|---|
| `server` (Rust) | web app, API, git push/pull, permissions, metering | small: CPU mostly git packing during clones |
| `worker` (Python, KimBim engine) | diffs, merges, IFC export, imports, checks | **this is where memory and CPU go.** Each worker runs one job at a time and holds one model in memory |
| `db` (Postgres 16) | accounts, projects, proposals, issues, jobs, usage | small |
| `data` volume | `repos/` (bare git repositories), `cache/` (IFC exports, .kim exports), `uploads/` | grows with history; back it up. The server is the only thing that mounts it |

**Worker sizing.** A worker's memory follows the largest model it opens. A few thousand elements needs well under 1 GB, while a 100k-element model being decomposed or exported needs several GB. Start with `NICGIT_WORKERS=2` and `NICGIT_WORKER_MEMORY=4g`. For big models, add workers or memory rather than server CPU. Moving the engine's hot paths into Rust (in kimcad) is the lever that brings this down (docs/future-work.md).

## Deploy with compose

The same stack runs both deployments; follow [self-hosting.md](self-hosting.md) for a studio's server or [hosting.md](hosting.md) for ours, and [deployment.md](deployment.md) for what differs between them and how releases are built.

```bash
cp deploy/self-hosted.env.example deploy/nicgit.env   # or deploy/hosted.env.example; fill it in
docker compose -f deploy/compose.yaml --env-file deploy/nicgit.env up -d
```

`deploy/compose.yaml` runs released images (`NICGIT_VERSION`, `NICGIT_IMAGE_PREFIX`). To build them from this working tree and a kimcad checkout instead, add `-f deploy/compose.build.yaml` and `--build`. Put a TLS-terminating proxy (Caddy, nginx, Traefik) in front of port 3000 and set `NICGIT_PUBLIC_URL` to the `https://` address so session cookies are marked secure.

**Sign-in.** Create an OAuth client in Google Cloud Console (type "Web application") that allows `<NICGIT_PUBLIC_URL>/api/auth/google/callback`, and set `NICGIT_GOOGLE_CLIENT_ID` and `NICGIT_GOOGLE_CLIENT_SECRET`. Google is the only sign-in, on premises too, so the server and its users must be able to reach Google (decision N28). Who may create an account depends on the deployment (`NICGIT_SIGNUP`, decision N39). Git and NicBim use personal access tokens.

The Google account whose verified email equals `NICGIT_SITE_ADMIN_EMAIL` is the site admin; removing it from the configuration demotes them at the next start. Site admins get **no** access to customers' private projects.

`scripts/stack-check.sh` proves a stack end to end under its own throw-away compose project.

## Configuration

| Variable | Default | |
|---|---|---|
| `NICGIT_DEPLOYMENT` | required | `hosted` or `self-hosted` ([deployment.md](deployment.md)) |
| `NICGIT_SIGNUP` | `open` when hosted, `closed` when self-hosted | `closed`: new accounts only for the site admin and `NICGIT_SIGNUP_DOMAINS` |
| `NICGIT_SIGNUP_DOMAINS` | unset | comma list of email domains that may join a closed server (exact domain) |
| `NICGIT_DATABASE_URL` | `postgres://nicgit:nicgit@127.0.0.1:5434/nicgit` | migrations run at start |
| `NICGIT_DATA_DIR` | `data` | repositories, caches, uploads |
| `NICGIT_BIND` | `127.0.0.1:3000` | the public address |
| `NICGIT_INTERNAL_BIND` | `127.0.0.1:3001` | where workers connect. **Never publish it.** Compose binds it inside the workers' network only |
| `NICGIT_PUBLIC_URL` | `http://<bind>` | shown in clone addresses; `https://` turns on secure cookies |
| `NICGIT_WORKER_TOKEN` | required | lets workers claim jobs |
| `NICGIT_GOOGLE_CLIENT_ID`, `NICGIT_GOOGLE_CLIENT_SECRET` | unset | without them nobody can sign in |
| `NICGIT_SITE_ADMIN_EMAIL` | unset | |
| `NICGIT_MAX_FILE_MB` | 512 | one upload, or one file in a push |
| `NICGIT_MAX_REPO_MB` | 2048 | one project's repository; also what its cached exports may take, and what a .kim may unpack to |
| `NICGIT_MAX_ACCOUNT_MB` | 5120 | all of an account's repositories and waiting uploads |
| `NICGIT_MAX_PROJECTS` | 100 | per account |
| `NICGIT_MAX_ORGS` | 3 | organisations one person may administer |
| `NICGIT_AI_ROUTER_URL`, `NICGIT_AI_ROUTER_KEY` | unset | see docs/ai-router.md |
| `NICGIT_WEB_DIR` | `web/dist` if present | the built web app |
| `NICGIT_GIT` | `git` | git executable |

Workers: `NICGIT_URL` (the internal address), `NICGIT_WORKER_TOKEN`, `NICGIT_JOB_TIMEOUT_MINUTES` (default 60), `NICGIT_WORKER_CONTAINED=1` when alone in a container (compose sets it), optional `NICGIT_WORKER_KINDS` (comma list) to give heavy jobs (`import_ifc`, `export_kim`) their own larger workers.

## Limits

What one person can store is limited (decision N33): on the hosted service because anyone can sign up, on a self-hosted one against mistakes. When a project or account is full:

- Pushes are refused, with the reason shown by git. A push that does not fit in the room left fails with git's `pack exceeds maximum allowed size`.
- Reading, cloning and finishing a Combine still work.
- Deleting a branch does not free space, because history is kept. To free space, delete a project. To give everyone more, raise the limits and restart. There is no per-account override yet.

Cached exports do not count against the account: they can be remade, so a project over its allowance loses the oldest.

**Watch the disk.** Several Google accounts get several allowances, and uploads being received are only counted once they finish. Alert on free space on the `data` volume.

## Data protection

- Every project belongs to one account and is private unless an admin makes it public. Every project route answers 404 to anyone without access, including git URLs. This is tested route by route (`tests/isolation.rs`).
- Engine caches and exports are stored per project and their keys include the project id, so they are never shared between customers.
- User names are public, and come from the Google display name, not the email address. Commits NicGit makes use a no-reply address. Commits people push carry whatever their own git is set to (decision N34).
- Session cookies and access tokens are stored only as SHA-256 hashes. There are no passwords.
- The audit log records sign-ins, tokens, membership and visibility changes, rules, combines, milestones, imports, exports, renames and AI settings, per account.
- Encryption at rest: use encrypted volumes for `db` and `data` (cloud disk encryption or LUKS).

### What separates customers in the workers

Engine workers parse untrusted IFC and .kim files, so assume one can be taken over by a crafted model. In compose (decision N30):

- A worker has no data volume, no database access (a separate network) and no internet, a read-only disk, no capabilities and a memory limit. It reaches only the server's internal address.
- Its supervisor holds the pool token, which can only claim jobs. Each job runs in a new process that gets only its own job's token, which reaches only that project's files and dies when the job ends or is handed out again.
- The supervisor's memory and environment cannot be read by the job.
- When a job ends, every other process in the container is stopped and the writable folders are emptied, so nothing of one job can watch the next.

**What remains shared:** the server, which is reached by every job and is the wall that enforces all this, and the host's kernel, which containers share. Compose isolates by container, so a kernel escape would reach other workers. For stronger separation run workers under gVisor or in separate VMs.

**What a taken-over job can still do:** lie about its own project (decision N36). It can report a wrong check or compare result, or make a wrong Combine commit when a lead combines, and it can read that one project. It cannot reach another customer's.

## Backups

- Postgres: `pg_dump` nightly plus WAL archiving for point-in-time recovery on the hosted service.
- Repositories: snapshot the `data` volume, or `git clone --mirror` each `repos/<id>/.git`. `cache/` and `uploads/` do not need backing up; they are regenerated.

## Upgrades

Stop workers, deploy the new server (it migrates the database on start), then start the new workers. A running job whose worker stops reporting for 10 minutes is handed out again, up to 3 times in all. A job stops after 60 minutes.
