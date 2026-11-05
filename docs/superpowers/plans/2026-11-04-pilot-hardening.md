# Pilot hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make NicGit safe to host for a private pilot: Google-only sign-in, workers that can reach only their own job's project, email through Postmark, row-level security, and a worker image and full stack that have actually run.

**Architecture:** Workers lose the shared data volume. A supervisor process claims jobs with the pool token and runs each job in a fresh child that holds only a per-job token; the child fetches git data over internal HTTP into a temporary folder, runs the existing engine handlers unchanged, and sends back commits and artifacts. Row-level security is applied at one place, the connection pool, from a request-scoped task-local.

**Tech Stack:** Rust (axum, sqlx, reqwest), Python worker, git smart HTTP, Postgres RLS, Google OIDC authorization code flow with PKCE, Postmark HTTP API.

**Spec:** `docs/superpowers/specs/2026-11-03-nicgit-design.md` sections 7 and 13, and `docs/decisions.md` N26-N32.

## Global Constraints

- Google is the only sign-in, for hosted and on-prem, for now (N28). Git keeps personal access tokens.
- The site admin is the account whose verified Google email equals `NICGIT_SITE_ADMIN_EMAIL` (N28).
- No password reset, no 2FA (N29). Email goes through Postmark only.
- A job reaches only its own project's files (N30). Workers hold no database credentials and mount no shared data.
- A project a user cannot read answers 404 everywhere (spec section 7).
- Every user-facing string exists in EN and ES.
- Run `cargo test -j 2` from Git Bash on this machine; engine tests need `NICGIT_TEST_ENGINE=1`.
- No Playwright (N27). Engine changes go in `../kimcad`; avoid them here if possible.

## Review Focus

- A Google account with `email_verified: false` must not sign in, or claim an existing account or the admin role. Test in Task 1.
- An email local part that is reserved, invalid as a slug, or already taken (`api@x.com`, `a+b@x.com`, two `ana@` domains) must still produce a working account. Test in Task 1.
- A job token used after its job finished or was reclaimed, or against another job's URL, must be refused. Test in Task 2.
- A job pushing refs it does not own (combine to another option, a read-only job pushing at all, an import overwriting an existing ref) must be refused. Test in Task 2.
- An artifact path with `..`, or a kind that does not produce artifacts, must be refused and write nothing. Test in Task 2.

---

### Task 1: Google-only sign-in

**Files:**
- Create: `crates/nicgit-server/src/api/google.rs`, `crates/nicgit-server/migrations/0004_google.sql`, `crates/nicgit-server/tests/google.rs`
- Modify: `src/config.rs`, `src/api/auth.rs`, `src/auth/*`, `src/main.rs`, `tests/common/mod.rs`, `tests/auth.rs`, `scripts/seed-demo.sh`, `scripts/e2e.py`, `web/src/pages/Auth.tsx`, `web/src/App.tsx`, `web/src/i18n/{en,es}.ts`, `crates/nicgit-server/Cargo.toml`

**Interfaces:**
- Produces: config `google_client_id`, `google_client_secret`, `google_auth_url`, `google_token_url`, `site_admin_email: Option<String>`; routes `GET /api/auth/google/start`, `GET /api/auth/google/callback`; `TestApp::user(slug)` keeps its signature; CLI `nicgit-server dev-user --slug S --email E [--name N]` printing a PAT, refused unless `NICGIT_DEV=1`.

- [ ] **Step 1: Write failing tests in `tests/google.rs` against a mock Google** (an axum server returning a token response with an unsigned id_token): `new_user_signs_in`, `state_mismatch_refused`, `nonce_mismatch_refused`, `wrong_audience_refused`, `wrong_issuer_refused`, `expired_refused`, `unverified_email_refused`, `sub_is_identity_not_email` (same sub, changed email, same account), `verified_email_links_unclaimed_account`, `email_claimed_by_other_sub_refused`, `admin_email_grants_and_revoking_config_demotes`, `admin_email_is_case_insensitive`, `slug_from_awkward_email` (`api@x.com`, `a+b@x.com`, `ana@a.com` + `ana@b.com`), `password_routes_are_gone` (404 on `/api/auth/signup`, `/api/auth/login`), `not_configured_explains_on_the_login_page` (redirects to `/login?error=not_configured`, because this is a browser navigation).
- [ ] **Step 2: Run them, expect failure:** `cargo test -j 2 -p nicgit-server --test google`
- [ ] **Step 3: Migration:** `users` gains `google_sub text UNIQUE`, drops `password_hash`.
- [ ] **Step 4: Implement `google.rs`:** authorization code with PKCE (S256); `state`, `nonce` and verifier in one HttpOnly, SameSite=Lax, 10 minute cookie; scopes `openid email profile`; redirect URI `{public_url}/api/auth/google/callback`. The id_token arrives directly from the token endpoint over TLS, so check claims without signature verification (OIDC Core 3.1.3.7): `iss` is `accounts.google.com` or `https://accounts.google.com`, `aud` equals the client id, `exp` in the future, `nonce` matches, `email_verified` true. Failures redirect to `/login?error=<code>`, never echoing provider text.
- [ ] **Step 5: Account rule:** look up by `google_sub`; else link an account with that verified email and null `google_sub`; else create one. Slug: lowercase local part, invalid characters become `-`, trimmed to a valid slug (fallback `user`), with `-2`, `-3`... on reserved or taken names. `is_site_admin` is set from config on every sign-in.
- [ ] **Step 6: Remove passwords:** signup and login routes, `hash_password`, `verify_password`, `DUMMY_HASH`, `MIN_PASSWORD`, and the `argon2` dependency and profile entry.
- [ ] **Step 7: Rework test harness and scripts:** `TestApp::user` inserts the account and a session directly and sets the cookie through a `reqwest` cookie jar. Port `tests/auth.rs`. `seed-demo.sh` and `e2e.py` use `dev-user` and a PAT. Add a dev-only way to view the app as the demo user: `dev-user --session` prints a session cookie value.
- [ ] **Step 8: Web:** login page is one "Continue with Google" link to `/api/auth/google/start`; `/signup` redirects to `/login`; `?error=` shows a translated message; remove unused auth strings from EN and ES.
- [ ] **Step 9: Verify:** `cargo test -j 2 -p nicgit-server`, `cd web && npm test && npm run build`. Expected: all pass.
- [ ] **Step 10: Commit** `feat(auth): Google-only sign-in; site admin from config; passwords removed`

### Task 2: Per-job access on the server

**Files:**
- Create: `crates/nicgit-server/src/api/job_files.rs`, `crates/nicgit-server/migrations/0005_job_tokens.sql`, `crates/nicgit-server/tests/job_access.rs`
- Modify: `src/api/internal.rs`, `src/git/http.rs`, `src/git/hook.rs`, `src/repos.rs`, `src/events.rs`, `src/api/transfer.rs`, `src/api/views.rs`, `src/api/proposals.rs`, `tests/jobs.rs`, `tests/actions.rs`, `tests/proposals.rs`

**Interfaces:**
- Consumes: `jobs` table, `git::http` service spawning.
- Produces, for Task 3:
  - `POST /internal/jobs/claim` (pool token `X-Worker-Token`) returns `{id, project, kind, args, token, fetch}` and no paths. `fetch` is `{"mode":"none"}`, `{"mode":"full"}`, or `{"mode":"commits","commits":[...],"depth":n}`.
  - With `Authorization: Bearer <token>`: `POST /internal/jobs/{id}/progress`, `POST /internal/jobs/{id}/finish`, `GET /internal/jobs/{id}/git/info/refs`, `POST .../git/git-upload-pack`, `POST .../git/git-receive-pack`, `GET /internal/jobs/{id}/upload`, `GET|PUT /internal/jobs/{id}/artifacts/{*path}`.
  - A job result names artifacts as `"artifact": "ifc/<commit>.ifc"` or `"exports/<name>.kim"`, relative to the project cache.
  - `repos::cache_dir(cfg, project)`, `repos::upload_path(cfg, project, name)`, `repos::set_head(cfg, project, option)`.

- [ ] **Step 1: Write failing tests in `tests/job_access.rs`:** `claim_returns_token_and_no_paths`; `pool_token_cannot_finish_or_fetch`; `token_refused_on_other_job`; `token_dead_after_finish`; `token_dead_after_reclaim`; `fetch_plan_by_kind` (export_ifc and checks: commit depth 1; compare and settings_diff: both commits depth 1; summary: depth 2; merge_preview, combine, element_history, export_kim, import_ifc: full; import_kim: none; a non-hash rev: full); `git_fetch_is_limited_to_job_project`; `readonly_job_cannot_push`; `combine_pushes_only_its_target_fast_forward`; `import_cannot_overwrite_existing_ref`; `invalid_record_push_refused`; `artifact_traversal_refused`; `artifact_kind_not_allowed`; `artifact_size_limit`; `upload_only_for_import_jobs`; `upload_stored_per_project_and_removed_on_finish`; `combine_result_must_match_target_head`.
- [ ] **Step 2: Run them, expect failure:** `cargo test -j 2 -p nicgit-server --test job_access`
- [ ] **Step 3: Migration and auth:** `jobs.token_hash text`, set at claim (a new token each claim), cleared at finish. Extractor `JobAuth { id, project, kind, args }` matches id, SHA-256 of the bearer token, and `status = 'running'`, else 401.
- [ ] **Step 4: Refactor `git/http.rs`:** extract advertise and RPC spawning so user and job routes share them. Job upload-pack runs with `-c uploadpack.allowReachableSHA1InWant=true`. Job pushes do not run `after_push`; proposal and import flows already act on the finished job.
- [ ] **Step 5: Hook:** add `NICGIT_JOB_REFS`. Combine: exactly `refs/heads/<target>`, with the target in `NICGIT_PROTECTED` and role lead, so fast-forward only and no delete. Imports: `refs/heads/*` and `refs/tags/*`, create-only (old id must be zero), except `import_ifc` on its own `args.option`, which may fast-forward. Other kinds get 403 before git starts. JSON record validation stays on for all.
- [ ] **Step 6: Artifacts and uploads in `job_files.rs`:** allowed `ifc/*.ifc` for `export_ifc` and `checks`, `exports/*.kim` for `export_kim`, GET of `ifc/*.ifc` for `checks`. Streamed, capped at `max_upload_mb * 20`, written to a temp name then renamed. Uploads move to `uploads/<project>/`, are served only for import kinds, and are deleted when the job finishes.
- [ ] **Step 7: Consumers:** `views.rs` and `transfer.rs` read `result.artifact` under `repos::cache_dir` with the path validated again. `events.rs` calls `repos::set_head` for `import_kim` after checking the option exists. `proposals.rs` refuses a combine result whose commit is not the target's head.
- [ ] **Step 8: Port fake-worker tests** (`jobs.rs`, `actions.rs`, `proposals.rs`) to use the returned token.
- [ ] **Step 9: Verify:** `cargo test -j 2 -p nicgit-server` without the engine. Expected: all pass. Engine tests are expected to fail until Task 3.
- [ ] **Step 10: Commit** `feat(jobs): per-job tokens; git, artifacts and uploads scoped to the job's project`

### Task 3: Worker supervisor, child and workspace; compose isolation

**Files:**
- Create: `workers/engine/nicgit_worker/child.py`, `workers/engine/nicgit_worker/workspace.py`, `workers/engine/tests/test_workspace.py`
- Modify: `workers/engine/nicgit_worker/__main__.py`, `workers/engine/nicgit_worker/transfer.py`, `workers/engine/tests/test_worker.py`, `deploy/compose.yaml`, `deploy/Dockerfile.worker`, `docs/ops.md`

**Interfaces:**
- Consumes: the Task 2 API.
- Produces: `workspace.prepare(job, client, root) -> (repo: Path, ctx: Context)` and `workspace.send_back(job, client, repo, ctx, result) -> dict`. `handlers.py` is unchanged.

- [ ] **Step 1: Write failing tests:** `test_child_env_has_no_pool_token`; `test_each_job_gets_a_new_process`; `test_workspace_removed_after_success_and_failure`; `test_supervisor_reports_crashed_child`; `test_push_only_for_writing_kinds`; `test_result_path_becomes_artifact`; `test_supervisor_not_dumpable_on_linux` (skipped elsewhere).
- [ ] **Step 2: Run them, expect failure:** `cd workers/engine && ../../scripts/engine-python.sh -m pytest -q tests`
- [ ] **Step 3: Implement `workspace.py`:** temp root holding `repo/.git`, `cache/`, `uploads/`. `full`: `git clone --bare`. `commits`: `git init --bare`, fetch each commit with `--depth`, point `refs/heads/nicgit-job` and HEAD at the first. `none`: `git init --bare`. Auth through `GIT_CONFIG_COUNT` environment `http.extraHeader`, never the URL or argv. `checks` first tries GET on the cached IFC. `send_back`: combine pushes its target unless `up_to_date`; imports push heads and tags; `path` in a result becomes an uploaded `artifact`.
- [ ] **Step 4: Implement `child.py` and the supervisor:** the child reads one job as JSON on stdin and runs the existing heartbeat logic with the job token. The supervisor calls `prctl(PR_SET_DUMPABLE, 0)` on Linux, spawns `sys.executable -m nicgit_worker.child` with `NICGIT_WORKER_TOKEN` removed from its environment, waits, and on a non-zero exit finishes the job with a generic error. `transfer.py` stops touching server-side state.
- [ ] **Step 5: Compose:** remove `data` from `worker`; add `read_only: true`, an anonymous volume at `/work`, `TMPDIR=/work`. Split networks: `db` (server, db) and `workers` (server, worker), both internal, so workers cannot reach Postgres. The server does not get the workers' memory limit.
- [ ] **Step 6: Verify with the real engine, from Git Bash:** `NICGIT_TEST_ENGINE=1 cargo test -j 2 --workspace --no-fail-fast`, the pytest command, and `./scripts/e2e.sh`. Expected: all pass, including `golden_m5_push_roundtrip`. Combine's tree equality with the engine's merge is pinned by `workers/engine/tests/test_handlers.py`, which calls the handlers directly, so also add a real-engine Rust test that compares the tree after a Combine through the new push path with the tree from `Project.merge` on the same inputs.
- [ ] **Step 7: Measure** job latency before and after on the M5 fixture (export, compare, combine). Record the numbers in `docs/ops.md`.
- [ ] **Step 8: Correct `docs/ops.md`:** the isolation section describes what is now true, including what remains shared (the server, the kernel). Fix the stale 30-minute reclaim line (10 minutes without a heartbeat).
- [ ] **Step 9: Commit** `feat(worker): one process per job, fetch over HTTP, no shared volume; isolated networks`

### Task 4: Worker image and full stack, proven

**Files:**
- Modify: `deploy/Dockerfile.worker`, `deploy/Dockerfile.server`, `deploy/compose.yaml`, `deploy/nicgit.env.example`, `docs/ops.md`
- Create: `scripts/stack-check.sh`

- [ ] **Step 1: Build:** `docker compose -f deploy/compose.yaml --env-file <test env> build`. Fix whatever fails. Record image sizes.
- [ ] **Step 2: Write `scripts/stack-check.sh`:** brings the stack up, creates a user with `dev-user` inside the server container, pushes the M5 fixture, requests an IFC export and a compare, and confirms results arrive and the IFC downloads. It also asserts from inside a worker: `/var/lib/nicgit/data` does not exist, the db host does not resolve or connect, and an outside address is unreachable.
- [ ] **Step 3: Run it.** Expected: all checks pass.
- [ ] **Step 4: Measure** idle and in-job worker memory. Update `docs/ops.md` and the env example with `NICGIT_GOOGLE_*`, `NICGIT_SITE_ADMIN_EMAIL`, `NICGIT_POSTMARK_*`.
- [ ] **Step 5: Commit** `deploy: worker image and full stack verified by stack-check`

### Task 5: Email through Postmark

**Files:**
- Create: `crates/nicgit-server/src/mail.rs`, `crates/nicgit-server/migrations/0006_email_pref.sql`, `crates/nicgit-server/tests/mail.rs`
- Modify: `src/notify.rs`, `src/config.rs`, `src/api/auth.rs`, the web account settings page, `web/src/i18n/{en,es}.ts`

**Interfaces:**
- Produces: config `postmark_token`, `postmark_url` (default `https://api.postmarkapp.com`), `mail_from`; `mail::send(state, to, subject, text)`; `email_notifications` in `GET`/`PATCH /api/me`.

- [ ] **Step 1: Write failing tests against a mock Postmark:** `notification_sends_email` (headers `X-Postmark-Server-Token`, body `From`, `To`, `Subject`, `TextBody`, `MessageStream: outbound`), `language_follows_user`, `opt_out_stops_email_keeps_inbox`, `no_token_sends_nothing`, `postmark_failure_keeps_inbox_and_request`, `email_has_no_model_content` (project name, title and link only).
- [ ] **Step 2: Run them, expect failure:** `cargo test -j 2 -p nicgit-server --test mail`
- [ ] **Step 3: Implement** `mail.rs`; call it from `notify.rs` in a spawned task after the inbox row is written. Failures are logged without the recipient address. Templates EN and ES per notification kind, linking through `public_url`.
- [ ] **Step 4: Add the setting** to `PATCH /api/me` and the account page.
- [ ] **Step 5: Verify:** `cargo test -j 2 -p nicgit-server`, `cd web && npm test && npm run build`. Expected: all pass.
- [ ] **Step 6: Commit** `feat(mail): notifications by email through Postmark, with opt-out`

### Task 6: Row-level security

**Files:**
- Create: `crates/nicgit-server/src/rls.rs`, `crates/nicgit-server/migrations/0007_rls.sql`, `crates/nicgit-server/tests/rls.rs`
- Modify: `src/app.rs`, `src/auth/*`, `src/git/http.rs`, `src/api/internal.rs`, `src/events.rs`, `tests/common/mod.rs`

**Interfaces:**
- Produces: task-local `rls::CONTEXT` (`User(Uuid)`, `Anonymous`, `System`), `rls::system(fut)`, `rls::layer` middleware; `TestApp.db` remains an unrestricted pool, separate from the server's.

- [ ] **Step 1: Spike, one test first:** `context_reaches_new_and_reused_connections`. Confirm the task-local is visible inside `after_connect` and `before_acquire` under concurrency with a small pool. **If sqlx runs them outside the caller's task, stop and report: this design is out, and hand-wrapping 157 call sites needs its own plan.**
- [ ] **Step 2: Write failing tests:** `query_without_scope_sees_nothing` (fail closed); `foreign_project_rows_invisible_without_where` for `projects`, `proposals`, `issues`, `comments`, `jobs`, `activity`, `milestones`, `reviews`, `option_rules`, `project_members`; `foreign_insert_refused`; `public_project_readable_anonymously`; `org_and_team_access_still_works`; `system_scope_sees_all`.
- [ ] **Step 3: Migration:** create NOLOGIN role `nicgit_app` if missing, with table grants. `app_can_see_project(uuid)` is a STABLE SECURITY DEFINER function covering owner, org member, project member, team access and public visibility. Enable and force RLS with policies on the project-scoped tables above. The pool runs `SET ROLE nicgit_app` after connect, since a superuser or owner would otherwise bypass RLS.
- [ ] **Step 4: Wire in:** `rls::layer` sets the context per request from the session or PAT; `/internal/*`, spawned tasks (`after_push`, mail, AI), account lookups and signup enter `rls::system`.
- [ ] **Step 5: Verify:** `NICGIT_TEST_ENGINE=1 cargo test -j 2 --workspace --no-fail-fast` from Git Bash and `./scripts/e2e.sh`. Expected: all pass, `tests/isolation.rs` included. Measure request latency on a list route before and after, and record it.
- [ ] **Step 6: Commit** `feat(db): row-level security as a second wall behind the route checks`

### Task 7: Finish

**Files:**
- Modify: `README.md`, `docs/decisions.md`, `docs/future-work.md`, `docs/ops.md`; in kimcad, `docs/decisions.md`

- [ ] **Step 1: Docs:** README sign-in and dev flow; decisions for choices made here; remove finished items from future work; add a note in kimcad's log that D33 is superseded by N1 (that commit is in the kimcad repo).
- [ ] **Step 2: Verify everything once more** and report the real output: Rust with engine, pytest, web, `e2e.sh`, `stack-check.sh`.
- [ ] **Step 3: Commit** `docs: pilot hardening`
