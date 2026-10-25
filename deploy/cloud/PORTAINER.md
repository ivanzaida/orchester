# Orchester Cloud on Portainer behind Traefik

How to run Orchester Cloud at `orchester.cloud` as a Portainer stack on a Docker host whose ingress is Traefik, with
images that GitHub Actions builds and redeploys. While app builds and team servers from before 2026-10-25 still call
the former address `orchester-cloud.lurq.dev`, the stack answers there too ([README.md](README.md), "Moving to another
domain"). For a host without Traefik, use the Caddy compose files in [README.md](README.md). What the service is, its
configuration and how it keeps its keys are in [docs/cloud-hosting.md](../../docs/cloud-hosting.md). Deploying is
setting the variables and pressing *Deploy*: the cloud makes its own root key at its first start and rotates and signs
its keys itself.

## What runs

| Piece | What |
| --- | --- |
| `.github/workflows/cloud-images.yml` | On every push to `master` that touches the cloud, its crates, `web/` or this folder (or by hand, *Run workflow*): builds `ghcr.io/ivanzaida/orchester-cloud` (backend, `deploy/cloud/Dockerfile`) and `ghcr.io/ivanzaida/orchester-cloud-web` (web front, `web/Dockerfile.cloud`) for `linux/amd64`, tags both `sha-<first 7 characters of the commit>` and `latest`, pushes them to GHCR, then has Portainer redeploy the stack |
| `compose.portainer.yaml` | The stack: `postgres` (PostgreSQL 18, volume `postgres_data`), `cloud` and `web`, from GHCR. Nothing is published; Traefik reaches `cloud` and `web` over its external network |

Traefik routes the hosts `orchester.cloud` (`ORCHESTER_CLOUD_HOST`) and `orchester-cloud.lurq.dev`
(`ORCHESTER_CLOUD_FORMER_HOST`): `/api/*` to `cloud` (port 8090), every other path to `web` (port 3000), as
`Caddyfile.web` does, and the web front takes forms on both (`ORCHESTER_CLOUD_EXTRA_ORIGINS`, set by the compose file
from the two hosts). Both routers send `Strict-Transport-Security: max-age=31536000`, plain HTTP on the hosts is
redirected to HTTPS, and neither router writes Traefik's access log. The web front sets its own security headers and
compresses its own pages, as it does behind Caddy.

The stack's own network `backend` is internal (no route out) with the fixed subnet `10.231.0.0/24`: `cloud` `.3`,
PostgreSQL `.4`, `web` `.5`. PostgreSQL is only on that network. `cloud` reaches the SMTP relay through the Traefik
network. The backend runs as one instance; its rate-limit counters are in memory, so do not scale `cloud`.

## 1. Check Traefik

Look these up once, in Traefik's own stack or its static configuration:

- **Its version: 3.3 or later.** The routers turn the access log off with `observability.accesslogs`, which Traefik
  knows from 3.3. An older Traefik refuses both containers' labels (its log says `field not found, node:
  observability`) and the whole host answers 404. On an older Traefik, either upgrade it, or delete the four
  `observability.accesslogs` labels from the compose file and keep Traefik's access log off server-wide (see
  *The access log* below).
- **The network** the other stacks join (the `traefik.docker.network` label of, for example, `pwstudio_web`); by
  default `traefik`.
- **The HTTPS entry point** (`websecure`), **the HTTP entry point** (`web`) and **the certificate resolver**
  (`letsencrypt`). Set the variables of step 6 if any name differs.

### The access log

The web front's paths carry one-time tokens (`/confirm/<token>`, `/reset/<token>`), so they must never reach a log.
Caddy keeps no access log; with Traefik, the three routers of this stack carry
`observability.accesslogs: 'false'`, which keeps their requests out of Traefik's access log whatever its server-wide
setting. One request is not covered: if Traefik's HTTP entry point redirects to HTTPS server-wide
(`entryPoints.web.http.redirections`), that redirect is Traefik's own router and follows the server-wide access log.
The emails' links are `https://`, so only a token someone types with `http://` would pass it. If Traefik's access
log is on, either accept that or turn it off server-wide.

## 2. DNS in Cloudflare

In the `orchester.cloud` zone, add:

- `A` `@` (the apex, `orchester.cloud`) → the server's public IPv4 address;
- `AAAA` `@` → its public IPv6 address, only if the server has one and Traefik listens on it.

Keep the records `orchester-cloud` of the `lurq.dev` zone (the former host) pointing at the same server while it still
answers. Set all of them to **DNS only (grey cloud)**. The Cloudflare proxy must be off for these hosts:

- With the proxy on, Traefik sees Cloudflare's addresses, not the visitors'. The backend and the web front would
  count every visitor under a few Cloudflare addresses, so the per-address limits on sign-up, sign-in and password
  resets would block everyone at once. Neither believes Cloudflare's headers.
- With DNS only, Traefik's certificate resolver works with any challenge type (an HTTP or TLS challenge through
  the proxy can fail).

## 3. The first images

The stack pulls the images, so they must exist first. Once the workflow is on `master`, open the repository on
GitHub → *Actions* → *Cloud images* → *Run workflow* on `master`. Both *Image* jobs push to GHCR. *Redeploy the
Portainer stack* is skipped, and the run shows the notice *Deploy skipped: repository secrets not set*, until step 7.

The repository is private, so the two packages are private too. They are linked to the repository and appear under
its *Packages*.

## 4. GHCR in Portainer

Portainer needs a read-only login to pull private images:

1. On GitHub: *Settings* → *Developer settings* → *Personal access tokens* → *Tokens (classic)* → *Generate new
   token (classic)*. Scope: only `read:packages`. Give it an expiry and write it down when it expires. (GHCR takes
   only classic tokens.)
2. In Portainer: *Registries* (in the side bar's administration section) → *Add registry* → *Custom registry*. Name `ghcr`, URL
   `ghcr.io`, *Authentication* on, username: the GitHub user, password: the token.
3. When the token expires, generate a new one and put it into this registry.

## 5. Trusted proxies: Traefik's address

The backend believes the client address in `X-Forwarded-For` only from the addresses in
`ORCHESTER_CLOUD_TRUSTED_PROXIES`. The compose file sets it to `TRAEFIK_ADDRESS` and the web front (`10.231.0.5`,
which forwards the address Traefik gave it). Requests from any other address are counted under their own address,
whatever they claim.

Find Traefik's address on its network: in Portainer, *Networks* → `traefik` → the table of containers lists each
one's IPv4 address, for example `172.18.0.2/16`. On the host, the same comes from
`docker network inspect traefik --format '{{range .Containers}}{{.Name}} {{.IPv4Address}}{{println}}{{end}}'`.

Set `TRAEFIK_ADDRESS` to the address **without** the `/16`: `172.18.0.2`. With `/16` the backend would trust every
container on that network.

**When Traefik's container is recreated** (an upgrade, a redeploy of its stack), Docker may give it another address.
Then the backend counts every API client (the desktop apps) under Traefik's new address, so they share one set of
limits, and whatever container gets the old address is trusted. Check the address after every Traefik redeploy and
update the variable. To make it stable, give Traefik a fixed address in its own stack (`ipv4_address`, which Docker
allows on a network created with an explicit subnet).

**What the shared network allows.** The web front believes the last `X-Forwarded-For` entry from whoever reaches its
port. Behind Caddy only Caddy could reach it; on the shared `traefik` network every container of every other stack
can call the web front's port 3000 directly and make the web front report any client address for its sign-up, sign-in and reset
forms, which gets around the per-address limits. The API itself is not affected. If that is not acceptable, give
this stack a network of its own with Traefik:

1. *Networks* → *Add network*: name `orchester-cloud-edge`, driver `bridge`, a free subnet such as
   `10.231.1.0/24`.
2. Add that network to Traefik's own stack (as an external network of its service), so it stays attached when
   Traefik is recreated, and redeploy Traefik.
3. In this stack set `TRAEFIK_NETWORK=orchester-cloud-edge` and `TRAEFIK_ADDRESS=10.231.1.0/24`. Trusting that whole
   subnet is right as long as nothing else joins it: its only members are Traefik, `cloud` and `web`.

## 6. Create the stack

In Portainer: *Stacks* → *Add stack*.

1. Name: `orchester-cloud`. The containers are then `orchester-cloud-cloud-1`, `orchester-cloud-web-1` and
   `orchester-cloud-postgres-1`.
2. *Build method*: *Web editor*. Paste `deploy/cloud/compose.portainer.yaml`.
3. *Environment variables* → *Advanced mode*, and paste the lines below with the values filled in:

   ```sh
   ORCHESTER_CLOUD_PUBLIC_URL=https://orchester.cloud
   POSTGRES_PASSWORD=<openssl rand -hex 24>
   ORCHESTER_CLOUD_SMTP_URL=smtps://<user>:<password>@<relay host>:465
   ORCHESTER_CLOUD_SMTP_FROM=Orchester <no-reply@lurq.dev>
   ORCHESTER_CLOUD_KEY_ENCRYPTION_KEY=<openssl rand -hex 32>
   ORCHESTER_CLOUD_WEB_SECRET=<openssl rand -base64 32>
   TRAEFIK_ADDRESS=<step 5>
   ```

   The stack that ran at `orchester-cloud.lurq.dev` before the move keeps `ORCHESTER_CLOUD_PUBLIC_URL` at that
   address until step 5 of "Moving the cloud" in [docs/cloud-hosting.md](../../docs/cloud-hosting.md).

4. *Deploy the stack*. The backend applies its database migrations when it starts, makes its root key on the very
   first start (its log names it: `root key ready root_public_key=… root_fingerprint=…`) and publishes its first key
   set before it listens. The web pages answer 404 for
   about ten seconds, until `web`'s health check passes (Traefik does not route to a container that is starting or
   unhealthy).

Keep a copy of the values in a password manager: Portainer is then the only other place they live.

| Variable | Default | Meaning |
| --- | --- | --- |
| `ORCHESTER_CLOUD_PUBLIC_URL` | required | The public address, without a trailing `/`. Email links start with it, identity tokens name it as issuer, the web front accepts forms from it and from both routed hosts. Its host must be `ORCHESTER_CLOUD_HOST` or `ORCHESTER_CLOUD_FORMER_HOST` |
| `POSTGRES_PASSWORD` | required | The database password, put into the backend's connection URL as it is, so URL-safe characters only (hex). PostgreSQL reads it only when it creates the volume; changing it later needs `ALTER USER` as well |
| `ORCHESTER_CLOUD_SMTP_URL` | required | The SMTP relay: `smtps://user:pass@host:465` or `smtp://user:pass@host:587?tls=required`, user and password URL-encoded ([README.md](README.md), "The SMTP relay") |
| `ORCHESTER_CLOUD_SMTP_FROM` | required | The sender of every email |
| `ORCHESTER_CLOUD_KEY_ENCRYPTION_KEY` | required | 64 hex characters. Seals the cloud's root key and signing keys in the database. Set it once and keep it: with another key the backend refuses to start ([docs/cloud-hosting.md](../../docs/cloud-hosting.md), "Losing the key-encryption key") |
| `ORCHESTER_CLOUD_WEB_SECRET` | required | 32 random bytes in standard base64. Signs the forms' CSRF tokens and seals the session cookie. Changing it signs every browser out |
| `TRAEFIK_ADDRESS` | required | Traefik's address on its network (step 5) |
| `ORCHESTER_CLOUD_HOST` | `orchester.cloud` | The host Traefik routes |
| `ORCHESTER_CLOUD_FORMER_HOST` | `orchester-cloud.lurq.dev` | The host the cloud ran at before, which Traefik routes too while app builds and team servers from before the move call it. Set it to the value of `ORCHESTER_CLOUD_HOST` to stop answering there |
| `TRAEFIK_NETWORK` | `traefik` | Traefik's external network |
| `TRAEFIK_ENTRYPOINT` | `websecure` | Traefik's HTTPS entry point |
| `TRAEFIK_HTTP_ENTRYPOINT` | `web` | Traefik's HTTP entry point, for the redirect to HTTPS |
| `TRAEFIK_CERT_RESOLVER` | `letsencrypt` | Traefik's certificate resolver |
| `CLOUD_SUBNET_PREFIX` | `10.231.0` | The first three numbers of the stack's own `/24`. Change it if another network on the host already uses `10.231.0.0/24` (the deploy then fails with *Pool overlaps*) |
| `CLOUD_TAG` | `latest` | The image tag of both images; set it to a `sha-` tag to roll back (step 9) |

If a required variable is missing, the deploy fails and names it. If a value is wrong, the backend or the web front
exits at once and logs the variable (*Containers* → the container → *Logs*), and Docker restarts it in a loop.

## 7. Automatic deploys

The deploy job updates the stack through Portainer's API: it reads the stack's compose file and variables and sends
them back unchanged with *re-pull images*, so Portainer pulls the new `latest` and recreates `cloud` and `web`.
(Portainer CE's stack webhooks redeploy only Git-backed stacks, and this one is not.) It refuses a Git-backed stack.

1. **An access token.** In Portainer, the user menu (top right) → *My account* → *Access tokens* → *Add access
   token*, description `github-orchester-cloud`. Portainer shows the token once. It has the rights of the user who
   made it; an administrator's token can change everything in Portainer, so prefer a user who can only manage this
   stack, if your Portainer setup has one.
2. **The stack id.** Open the stack in Portainer; the address bar ends with `…/stacks/orchester-cloud?id=<number>&…`.
3. **The secrets.** On GitHub: the repository → *Settings* → *Secrets and variables* → *Actions* → *New repository
   secret*, three times:

   | Secret | Value |
   | --- | --- |
   | `PORTAINER_URL` | `https://portainer.lurq.dev` |
   | `PORTAINER_TOKEN` | the access token |
   | `PORTAINER_STACK_ID` | the stack id |

   The environment comes from the stack itself. GitHub's runners must be able to reach `PORTAINER_URL` from the
   internet.
4. *Actions* → *Cloud images* → *Run workflow* on `master`. *Redeploy the Portainer stack* ends with *Stack
   orchester-cloud redeployed.* On an error it prints Portainer's message, for example `401` for a wrong token or
   `404` for a wrong stack id.

The job never prints the token or the stack's variables: it masks every variable's value before anything else and
hands the token to `curl` in a file.

## 8. Check it works

On each host, `orchester.cloud` and, while it answers, `orchester-cloud.lurq.dev`:

- `https://orchester.cloud/signup` shows the sign-up page, a sign-up gets its email, and a sign-in on the page
  works (the form is taken on that host).
- `curl -s -o /dev/null -w '%{http_code}\n' https://orchester.cloud/api/v1/keys` answers `200` from the first start.
- `curl -sI https://orchester.cloud/signin` shows `strict-transport-security: max-age=31536000`.
- `curl -sI http://orchester.cloud/` answers with a redirect to `https://`.
- Traefik's access log, if it is on, has no line for either host.
- *Containers* shows `orchester-cloud-web-1` as *healthy*. If it is *unhealthy* or restarting, Traefik stops
  routing to it: the pages answer `404` while `/api/*` (the desktop apps, team servers' invites) keeps working.
  Watch that state in your monitoring.

## 9. Roll back

Every build keeps its `sha-` tags in GHCR (the repository → *Packages* → `orchester-cloud` → the versions; the tag is
the commit's first 7 characters).

1. Take a database dump first (see *Backups* below).
2. The stack → *Editor* → *Environment variables*: add `CLOUD_TAG=sha-<7 characters>`. Both images take that tag.
3. *Update the stack* with *Re-pull image and redeploy* on.

While `CLOUD_TAG` is set, the automatic deploys keep that tag, because they send the stack's own variables back.
Delete the variable and update the stack again to follow `latest`.

An older backend refuses to start on a database a newer one has migrated (the migrations are forward-only). If the
version you go back to is older than a migration, restore the dump taken before the upgrade
([README.md](README.md), "Backups" and "Upgrades").

## Backups and logs

[README.md](README.md), "Backups" and "Logs", applies as written; with a shell on the host, `dc exec` becomes
`docker exec` with the container name, for example
`docker exec orchester-cloud-postgres-1 pg_dump -U orchester_cloud -Fc orchester_cloud > orchester-cloud-$(date +%F).dump`.
The values of step 6 take the place of `.env` in the backups.
