# Running NicGit

## Pieces

| Piece | What it does | Size it by |
|---|---|---|
| `server` (Rust) | web app, API, git push/pull, permissions, metering | small: measured 23 MB working set after serving the demo (Windows, debug build); CPU mostly git packing during clones |
| `worker` (Python, KimBim engine) | diffs, merges, IFC export, imports, checks | **this is where memory and CPU go.** ~85 MB idle (measured); each worker runs one job at a time and holds one model in memory |
| `db` (Postgres 16) | accounts, projects, proposals, issues, jobs, usage | small |
| `data` volume | `repos/` (bare git repositories), `cache/` (IFC exports, .kim exports), `uploads/` | grows with history; back it up |

**Worker sizing.** A worker's memory follows the largest model it opens. A few thousand elements needs well under 1 GB, while a 100k-element model being decomposed or exported needs several GB. Start with `NICGIT_WORKERS=2` and `NICGIT_WORKER_MEMORY=4g`. For big models, add workers or memory rather than server CPU. Moving the engine's hot paths into Rust (in kimcad) is the lever that brings this down (docs/future-work.md).

## Deploy with compose

```bash
cp deploy/nicgit.env.example deploy/nicgit.env      # set NICGIT_DB_PASSWORD, NICGIT_WORKER_TOKEN, NICGIT_PUBLIC_URL
docker compose -f deploy/compose.yaml --env-file deploy/nicgit.env up -d --build
```

The images build from this repository with the kimcad checkout as a second build context (`../../kimcad` from `deploy/`). Put a TLS-terminating proxy (Caddy, nginx, Traefik) in front of port 3000 and set `NICGIT_PUBLIC_URL` to the `https://` address so session cookies are marked secure.

The first account that signs up becomes the site admin. Site admins get **no** access to customers' private projects.

## Configuration

| Variable | Default | |
|---|---|---|
| `NICGIT_DATABASE_URL` | `postgres://nicgit:nicgit@127.0.0.1:5434/nicgit` | migrations run at start |
| `NICGIT_DATA_DIR` | `data` | repositories, caches, uploads |
| `NICGIT_BIND` | `127.0.0.1:3000` | |
| `NICGIT_PUBLIC_URL` | `http://<bind>` | shown in clone addresses; `https://` turns on secure cookies |
| `NICGIT_WORKER_TOKEN` | required | shared secret between server and workers |
| `NICGIT_MAX_UPLOAD_MB` | 2048 | .kim/.ifc import limit; a .kim may unpack to at most 20× its size |
| `NICGIT_AI_ROUTER_URL`, `NICGIT_AI_ROUTER_KEY` | unset | see docs/ai-router.md |
| `NICGIT_WEB_DIR` | `web/dist` if present | the built web app |
| `NICGIT_GIT` | `git` | git executable |

Workers: `NICGIT_URL`, `NICGIT_WORKER_TOKEN`, optional `NICGIT_WORKER_KINDS` (comma list) to give heavy jobs (`import_ifc`, `export_kim`) their own larger workers.

## Data protection

- Every project belongs to one account and is private unless an admin makes it public. Every project route answers 404 to anyone without access, including git URLs. This is tested route by route (`tests/isolation.rs`).
- Engine caches and exports are stored per project and their keys include the project id, so they are never shared between customers.
- Workers hold no database credentials. They see the job, its project's repository and cache, and nothing else. In compose they sit on an internal network with no internet access, no privileges and a memory limit.
- Passwords are Argon2id. Session cookies and access tokens are stored only as SHA-256 hashes.
- The audit log records sign-ins, failed sign-ins, tokens, membership and visibility changes, rules, combines, milestones, imports, exports and AI settings, per account.
- Encryption at rest: use encrypted volumes for `db` and `data` (cloud disk encryption or LUKS).

## Backups

- Postgres: `pg_dump` nightly plus WAL archiving for point-in-time recovery on the hosted service.
- Repositories: snapshot the `data` volume, or `git clone --mirror` each `repos/<id>/.git`. `cache/` and `uploads/` do not need backing up; they are regenerated.

## Upgrades

Stop workers, deploy the new server (it migrates the database on start), then start the new workers. Jobs left running are handed out again after 30 minutes.
