# Running NicGit on your own server

For a studio's IT person: NicGit on one machine, on premises or on your own cloud, for your own people. What the pieces are, every setting, and how the workers are kept apart is in [ops.md](ops.md); how this differs from our hosted service is in [deployment.md](deployment.md).

## What you need

- A Linux machine with Docker and the compose plugin. Start with 4 CPUs, 16 GB of memory and disk for your projects' history; memory goes to the engine workers, and grows with your largest model ([ops.md](ops.md), Pieces).
- A DNS name for it, and a TLS certificate (the proxy below gets one for you).
- **Internet access to Google**, for the server and for the people who sign in: Google is the only sign-in for now (decisions N28, N37). Nothing else needs to reach out, except an AI router if you are given one.
- The NicGit images, `nicgit-server:<version>` and `nicgit-worker:<version>`, from us or built with `scripts/build-images.sh`.

## Install

1. **Settings.** `cp deploy/self-hosted.env.example deploy/nicgit.env` and fill it in:
   - `NICGIT_VERSION` (the release you were given) and `NICGIT_IMAGE_PREFIX` (where the images come from).
   - `NICGIT_DB_PASSWORD` and `NICGIT_WORKER_TOKEN`: long and random, e.g. `openssl rand -hex 32`.
   - `NICGIT_PUBLIC_URL`: the `https://` address people will use.
   - `NICGIT_SITE_ADMIN_EMAIL`: the Google address of whoever runs the server.
   - `NICGIT_SIGNUP_DOMAINS`: your studio's email domain(s), e.g. `studio.example,studio-branch.example`.
2. **Google sign-in.** In Google Cloud Console create an OAuth client of type "Web application" allowing `<NICGIT_PUBLIC_URL>/api/auth/google/callback`, and put its id and secret in `NICGIT_GOOGLE_CLIENT_ID` and `NICGIT_GOOGLE_CLIENT_SECRET`. On Google Workspace, make it an "Internal" app so only your organisation sees it.
3. **Start.** `docker compose -f deploy/compose.yaml --env-file deploy/nicgit.env up -d`
4. **TLS.** Put a proxy in front of port 3000. With Caddy, which gets and renews the certificate itself, the whole `Caddyfile` is:

   ```text
   nicgit.studio.example {
       reverse_proxy 127.0.0.1:3000
   }
   ```

   Publish only the proxy. Port 3001, the workers' address, is never published (decision N35).
5. **Check.** `curl https://nicgit.studio.example/api/instance` answers `"deployment": "self-hosted"`, `"signup": "closed"`, your version and the engine commit. Then sign in as the site admin.

## Who can join

Sign-up is closed: only the site admin and verified Google addresses at `NICGIT_SIGNUP_DOMAINS` get an account the first time they sign in. Everyone else is told the server only admits its own people. To admit someone outside those domains, a consultant say, add their domain, or set `NICGIT_SIGNUP=open` while they join, then restart. Invitations are future work.

People then work through organisations and projects as on the hosted service, and connect NicBim to a project with a personal access token (account settings) as the git password.

## Backups and restore

Back up two things, together:

- The database: `docker compose -f deploy/compose.yaml --env-file deploy/nicgit.env exec -T db pg_dump -U nicgit -Fc nicgit > nicgit-$(date +%F).dump`
- The `data` volume, which holds every project's history under `repos/`. A volume snapshot, or a copy of the volume with the server stopped. `cache/` and `uploads/` need no backup.

To restore on a new machine: install as above with the same settings, stop the server, restore the volume, then `pg_restore -U nicgit -d nicgit --clean` the dump inside the `db` container, and start the server.

## Upgrades

Read the [CHANGELOG](../CHANGELOG.md) for the new version first: when the engine's record format changes, upgrade the NicBim desktop app at the same time. Then set `NICGIT_VERSION` and follow [ops.md](ops.md), Upgrades: stop the workers, start the new server (it migrates the database), start the new workers. Back up before.
