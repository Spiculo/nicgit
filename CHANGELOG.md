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
