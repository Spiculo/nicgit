# NicGit

The forge for NicBim (KimBim) projects: a git server and a web app where architects, consultants and clients **see, compare, review and combine** versions of a building model. Hosted by us, or on your own server.

- **Git hosting** that works with the desktop app today: `kimbim versions push/pull` (or plain git) to `https://<host>/<owner>/<project>.git`, with a personal access token as the password.
- **The model in the browser** at any version: 3D, element properties, the history of any element.
- **Compare** any two versions or design options: changed elements coloured in 3D, grouped by level, field by field, plus project settings.
- **Proposals** to combine one design option into another: approvals by role, protected options, required checks, and conflicts settled by keeping one side or the other (whole, or per conflict). Combine runs the engine's own record merge, never git's text merge.
- **Issues** as BCF topics (import and export `.bcfzip`), **milestones** for project phases with sign-off, `.kim`/`.ifc` **import** and `.kim` **export**.
- **Actions** (the engine's schema, IDS, clash and model checks) and **AI** summaries through the AI router, both metered and capped by budgets.
- Private by default; a project you cannot read does not exist for you (404 everywhere).

Design: [docs/superpowers/specs/2026-11-03-nicgit-design.md](docs/superpowers/specs/2026-11-03-nicgit-design.md). Decisions: [docs/decisions.md](docs/decisions.md). Next: [docs/future-work.md](docs/future-work.md). Operating it: [docs/ops.md](docs/ops.md). AI router contract: [docs/ai-router.md](docs/ai-router.md).

## Layout

```text
crates/nicgit-core     roles, names, conflict policy, approval rules (pure, unit-tested)
crates/nicgit-server   Rust server (axum, sqlx/Postgres): API, auth, git smart HTTP, hooks, jobs, metering, AI
workers/engine         Python engine worker: KimBim engine jobs (diff, merge, export, import, checks)
web                    React + TypeScript app, 3D with web-ifc + three.js
deploy                 container images and compose files
scripts                dev stack, demo data, end-to-end test
```

The engine is the KimBim checkout next to this one (`../kimcad`; override with `KIMBIM_HOME`). The server links its `kimbim-repo` crate; the worker imports its Python package and runs in its pixi environment.

## Develop

Needs Rust, Node 22, Docker (for Postgres), git, and a kimcad checkout with its pixi environment built (`pixi install && pixi run build` there; `pixi run m5 && pixi run house` for the fixtures).

```bash
cd web && npm ci && npm run build && cd ..
./scripts/dev.sh            # Postgres on :5434, server on :3000 (serves web/dist), one engine worker
./scripts/seed-demo.sh      # user ana / "correct horse battery", project studio/sample-house with a proposal
./scripts/dev-stop.sh
```

Then open http://127.0.0.1:3000.

## Test

```bash
cargo test --workspace                                   # server + core (needs the dev Postgres)
NICGIT_TEST_ENGINE=1 cargo test -p nicgit-server         # also the tests that run the real engine worker
(cd workers/engine && ../../scripts/engine-python.sh -m pytest -q tests)
(cd web && npm test)
./scripts/e2e.sh                                         # the whole flow on a fresh database
```

## License

AGPL-3.0-or-later.
