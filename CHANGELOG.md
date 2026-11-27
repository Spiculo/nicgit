# Changelog

One entry per release, newest first. Every entry says which engine commit (`engine.rev`) the release is built with, and whether the engine's record format changed since the last release: when it did, studios upgrade the NicBim desktop app together with the server (docs/deployment.md).

## Unreleased

Engine: nicbim `0d602c4` (the `.nic` format; supersedes the `f093285` first pin).

- Two deployments from one build: `NICGIT_DEPLOYMENT=hosted|self-hosted`, required (decision N38). Existing setups must add it.
- Sign-up follows the deployment: open on the hosted service, closed on a self-hosted server except for the site admin and `NICGIT_SIGNUP_DOMAINS` (N39).
- `GET /api/instance`: deployment, sign-up, version and engine commit.
- Release images built from a pinned engine commit with `scripts/build-images.sh` (N40). `deploy/compose.yaml` now runs released images (`NICGIT_VERSION`, `NICGIT_IMAGE_PREFIX`); building from source moved to `deploy/compose.build.yaml`. Dockerfiles moved to `deploy/images/`, and the env example is now one per deployment (N41).
- NicBim renamed its project container from `.kim` to `.nic` (NicBim D251). NicGit now imports and exports `.nic`, and the worker has an adapter for older pinned engines; `import_nic`/`export_nic` replace the old job kinds (N46).
- The engine checkout is `../nicbim` (`NICBIM_HOME`, with `KIMBIM_HOME` still read); the Rust dependency and Docker build context follow the rename (N46).
- Branches: create, delete, protected options and a default branch that updates the repository HEAD (N48).
- Organisations: project list, teams, team members and team project grants (N49).
- Releases: milestones and tags together, with IFC, model and source `.zip` downloads (`GET /projects/{owner}/{slug}/archive?rev=...`) (N50).
- Local preview sign-in at `/api/dev/login?slug=...`, only when `NICGIT_DEV=1` (N47).
- GitHub-shaped navigation: centred page content, a repository header with tabs, account settings split into pages under `/settings` (Profile, Access, Language & wording, Organisations, AI, Billing) and project Configs split into pages under `/<owner>/<repo>/settings` (General, People, Protected options, Webhooks, Audit log, Delete). Field help sits behind an `(i)` control; unbuilt tabs show one empty state (N56, N59).
- Home is a three-column dashboard: accessible repositories by last activity, posts and recent releases, pull requests, issues and checks in the centre, change log and news on the right (`GET /api/me/home`) (N60).
- Profiles are at `/{slug}`; `/orgs/{slug}` redirects and the API keeps `/api/orgs`. Explore gains search, an owner-kind filter and sort. Private remains the default; public is the exception for university/coursework, examples and demos (N60).
- Each account chooses Git or architectural wording in Settings → Language & wording. Git says repository, pull request, issue, branch, CI, package, release and tag; architectural says project, proposal, task, option, workflow, library, milestone and snapshot. Both have English and Spanish strings (`users.style`, `PATCH /api/me`) (N61).
- New accounts answer a three-step welcome card before the app opens: language, wording, field, years in the field, site experience, project focus, company, team size and company size (`/welcome`, `POST /api/me/onboarding`). Existing accounts are marked onboarded and can reopen it from Settings → Profile (N62).
- The CI tab reads the existing `checks` jobs: runs per option or version, required checks, pass/fail, duration, each check's issues and a printable report (`GET /api/projects/{owner}/{slug}/ci/runs`, `/ci/runs/{id}`, `/ci/checks`, `/ci/runs/{id}/report.html`) (N63). No new execution model: Actions still run the jobs.
- The project overview and the empty project state show how to connect NicBim: the project address, a copy button, the Versions > Git and File > Clone steps, and the token page. The import form strips `.nic` (not the old `.kim`) when it suggests a project name.
- `docs/missing-functionality.md` audits the web app and the NicBim flow, with priorities and evidence.
