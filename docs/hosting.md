# Running the hosted service

Our NicGit service, open to anyone (`NICGIT_DEPLOYMENT=hosted`). Only what follows from the code belongs here; the service's machines, secrets, DNS, monitoring and billing live in its private infrastructure (decision N41). The reference for every setting is [ops.md](ops.md); how this differs from a studio's server is in [deployment.md](deployment.md).

## Start

The same stack as a self-hosted server: `cp deploy/hosted.env.example deploy/nicgit.env`, fill it in, `docker compose -f deploy/compose.yaml --env-file deploy/nicgit.env up -d`, behind a TLS proxy. `/api/instance` answers `"deployment": "hosted"`, `"signup": "open"`.

## Because anyone can join

- **Storage limits** are what stops one person filling the disk (decision N33). Keep the N33 defaults until the pilot says otherwise. Several Google accounts get several allowances, so **alert on free space** on the `data` volume.
- **Engine workers parse untrusted files from strangers.** Run them as compose isolates them at least, and under gVisor or in separate VMs before running anyone's own code ([ops.md](ops.md), What separates customers in the workers).
- Public projects are readable by anyone, but anonymous visitors cannot start engine work.

## Revenue lines

- **Metering**: every Action and AI call is recorded in `usage_events` per account, and budgets cap them (decisions N19, N20). Billing integration reads from there and is not built.
- **AI**: set `NICGIT_AI_ROUTER_URL` and `NICGIT_AI_ROUTER_KEY` to our router ([ai-router.md](ai-router.md)).

## Backups

`pg_dump` nightly plus WAL archiving for point-in-time recovery, and snapshots of the `data` volume ([ops.md](ops.md), Backups).

## Scaling, when it is needed

Today one server holds the `data` volume, so there is one server process. The planned steps, in order: more and larger workers (`NICGIT_WORKERS`, `NICGIT_WORKER_KINDS` for heavy jobs), object storage for caches and exports, then git hosting split off with a role flag on the same binary (decision N32).
