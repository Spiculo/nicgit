# Changelog

One entry per release, newest first. Every entry says which engine commit (`engine.rev`) the release is built with, and whether the engine's record format changed since the last release: when it did, studios upgrade the NicBim desktop app together with the server (docs/deployment.md).

## Unreleased

Engine: kimcad `f093285` (first pin).

- Two deployments from one build: `NICGIT_DEPLOYMENT=hosted|self-hosted`, required (decision N38). Existing setups must add it.
- Sign-up follows the deployment: open on the hosted service, closed on a self-hosted server except for the site admin and `NICGIT_SIGNUP_DOMAINS` (N39).
- `GET /api/instance`: deployment, sign-up, version and engine commit.
- Release images built from a pinned engine commit with `scripts/build-images.sh` (N40). `deploy/compose.yaml` now runs released images (`NICGIT_VERSION`, `NICGIT_IMAGE_PREFIX`); building from source moved to `deploy/compose.build.yaml`. Dockerfiles moved to `deploy/images/`, and the env example is now one per deployment (N41).
