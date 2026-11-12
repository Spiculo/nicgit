# Deploying NicGit

NicGit runs two ways, from **one codebase, one binary and one pair of images** (decision N2):

| | **Hosted** (`NICGIT_DEPLOYMENT=hosted`) | **Self-hosted** (`NICGIT_DEPLOYMENT=self-hosted`) |
|---|---|---|
| Who runs it | us, as a service (SaaS) | a studio, on premises or on its own cloud |
| Who may join | any verified Google account | the site admin, and verified addresses at the studio's domains (`NICGIT_SIGNUP_DOMAINS`) |
| Storage limits | guard against strangers (N33 defaults) | guard against mistakes: sized to the studio's disk |
| Metering and budgets | the base for billing | recorded the same way; what they mean on a studio's own server is open (below) |
| AI summaries | through our AI router | only if the studio is given a router URL and key (open, below) |
| Guide | [hosting.md](hosting.md) | [self-hosting.md](self-hosting.md) |

What both share, and must keep sharing: the code, the images, the database schema and its migrations, the compose stack in `deploy/`, the reference in [ops.md](ops.md), and the data protection in it. Anything that differs is a configuration default chosen by `NICGIT_DEPLOYMENT`, never a separate build. A feature that only one of them needs still ships in both, switched by configuration.

## The switch

`NICGIT_DEPLOYMENT` has no default: the server does not start without it, because the two differ in who may join (decision N38). Today it sets:

- **Sign-up** (decision N39). `NICGIT_SIGNUP` is `open` or `closed`; hosted defaults to `open`, self-hosted to `closed`. On a closed server a *new* account is made only for the site admin (`NICGIT_SITE_ADMIN_EMAIL`) and for verified addresses at a domain in `NICGIT_SIGNUP_DOMAINS` (comma list, exact domain, no subdomains). Anyone else gets "this server only admits its own people". Existing accounts always sign in.

`GET /api/instance` reports it to clients: `{deployment, signup, version, engine}`.

New differences go here first, as a row in the table above and a line in this list, with a decision number.

## Layout

```text
deploy/
  compose.yaml              the stack on one machine (db, server, workers), from released images; both deployments
  compose.build.yaml        overlay: build the images from source instead (development, scripts/stack-check.sh)
  self-hosted.env.example   a studio's settings
  hosted.env.example        our service's settings (the real ones live in the hosted service's private infra)
  images/Dockerfile.server  the Rust server, the web app and git
  images/Dockerfile.worker  the engine (kimcad) and the job runner
  compose.dev.yaml          the development database only
engine.rev                  the kimcad commit releases are built with
scripts/build-images.sh     builds the release images
```

The hosted service's own infrastructure (machines, DNS, secrets, backups, monitoring, billing) does **not** live in this repository: it is AGPL and public, and a studio must be able to run everything here without it (decision N41). That private infrastructure consumes the same images.

## Versions and the engine

A NicGit release is one version number (`version` in `Cargo.toml`) for both images, `nicgit-server:<version>` and `nicgit-worker:<version>`. Always run a server and workers of the same version.

Both images carry the same engine: the kimcad commit in [`engine.rev`](../engine.rev) (decision N40). The engine defines the record format, the merge and the checks, so the engine commit, not the NicGit version, decides whether a desktop app and a server understand each other. `scripts/build-images.sh` builds both images from this repository as committed and kimcad at that commit (both through `git archive`, so neither checkout's uncommitted state gets in), and compiles the commit into the server, which reports it at `/api/instance` and in the image label `dev.nicgit.engine.revision`.

Development is different on purpose: `cargo build` and `scripts/dev.sh` use whatever `../kimcad` has checked out, so engine and forge can change together. `scripts/dev.sh` says when that is not the pinned commit, and the dev server reports `engine: "dev"`.

**Releasing**, until there is CI for it:

1. Point `engine.rev` at the kimcad commit the release was tested with, and run the tests against it (README, Test).
2. Set `version` in `Cargo.toml` and add the release to [CHANGELOG.md](../CHANGELOG.md), saying when the engine commit changed and whether the record format did.
3. Commit, tag `v<version>`, run `scripts/build-images.sh` (`PUSH=1` with `NICGIT_IMAGE_PREFIX` set to push to a registry).

**Compatibility with the desktop app.** Not checked by anything yet. Until it is: a desktop app and a server work together when they are on the same engine record format; a studio upgrading NicGit across an engine format change upgrades the desktop app with it. A version check (the app reading `/api/instance`) is future work.

## Open questions for the owner

These shape the self-hosted offer and are not decided (see decisions.md, N42–N45):

- **AI on a self-hosted server**: our router with a per-studio key, the studio's own router, or none.
- **Metering and licensing on a self-hosted server**: whether usage is reported to us, whether there is a licence key, or only a support contract.
- **Where images are published** (which registry, public or per-customer) and whether they are signed.
- **Sign-in beyond Google** for studios on Microsoft 365 or without Google access: OIDC first, then SAML/LDAP.
