# NicGit

The forge for NicBim projects: a git server and a web app where architects, consultants and clients **see, compare, review and combine** versions of a building model. Hosted by us, or on your own server.

- **Git hosting** that works with the desktop app today: `kimbim versions push/pull` (or plain git; the CLI keeps its `kimbim` name until the NicBim code rename) to `https://<host>/<owner>/<project>.git`, with a personal access token as the password.
- **The model in the browser** at any version: 3D, element properties, the history of any element.
- **Compare** any two versions or design options: changed elements coloured in 3D, grouped by level, field by field, plus project settings.
- **Proposals** to combine one design option into another: approvals by role, protected options, required checks, and conflicts settled by keeping one side or the other (whole, or per conflict). Combine runs the engine's own record merge, never git's text merge.
- **Branches** (design options) with create, delete, protected options and a default branch that a clone checks out.
- **Issues** as BCF topics (import and export `.bcfzip`), **releases** from milestones and tags with sign-off and a source archive, `.nic`/`.ifc` **import** and `.nic` **export**.
- **Accounts, organisations and teams**: public or private projects, five roles, org members, teams and team project grants.
- **Actions** (the engine's schema, IDS, clash and model checks) and **AI** summaries through the AI router, both metered and capped by budgets.
- Private by default; a project you cannot read does not exist for you (404 everywhere).

Design: [docs/superpowers/specs/2026-11-03-nicgit-design.md](docs/superpowers/specs/2026-11-03-nicgit-design.md). Decisions: [docs/decisions.md](docs/decisions.md). Next: [docs/future-work.md](docs/future-work.md). AI router contract: [docs/ai-router.md](docs/ai-router.md). Changes: [CHANGELOG.md](CHANGELOG.md).

## Deploy

One build runs two ways ([docs/deployment.md](docs/deployment.md)): our **hosted** service, open to anyone, and **self-hosted** servers that a studio runs on premises or on its own cloud for its own people. Studios: [docs/self-hosting.md](docs/self-hosting.md). Our service: [docs/hosting.md](docs/hosting.md). Reference for both: [docs/ops.md](docs/ops.md).

## Layout

```text
crates/nicgit-core     roles, names, conflict policy, approval rules (pure, unit-tested)
crates/nicgit-server   Rust server (axum, sqlx/Postgres): API, auth, git smart HTTP, hooks, jobs, metering, AI
workers/engine         Python engine worker: NicBim engine jobs (diff, merge, export, import, checks)
web                    React + TypeScript app, 3D with web-ifc + three.js
deploy                 the compose stack both deployments run, env examples, Dockerfiles (deploy/images)
scripts                dev stack, demo data, end-to-end test, release images
engine.rev             the nicbim commit release images are built with
```

The engine is the NicBim checkout next to this one (`../nicbim`; override with `NICBIM_HOME`, or the old `KIMBIM_HOME`). The server links its `kimbim-repo` crate; the worker imports its Python package and runs in its pixi environment. Development uses that checkout as it is; releases use the commit in `engine.rev`.

## Develop

Needs Rust, Node 22, Docker (for Postgres), git, and a nicbim checkout with its pixi environment built (`pixi install && pixi run build` there; `pixi run m5 && pixi run house` for the fixtures).

```bash
cd web && npm ci && npm run build && cd ..
./scripts/preview.sh        # Postgres on :5434, server on :3000 (web/dist), one worker, demo data
./scripts/preview.sh stop   # stop all three
# or the individual pieces: ./scripts/dev.sh, ./scripts/seed-demo.sh, ./scripts/dev-stop.sh
```

Then open http://127.0.0.1:3000. A local server can sign in without Google when `NICGIT_DEV=1`:
`http://127.0.0.1:3000/api/dev/login?slug=ana` (decision N47; never enable it in production).

The macOS NicBim checkout works too: `pixi install && pixi run build` there, then `pixi run m5` for the demo fixture. The engine worker runs in the same pixi environment (`./scripts/engine-python.sh -m nicgit_worker`), so macOS can run the whole stack; release images are Linux.

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
