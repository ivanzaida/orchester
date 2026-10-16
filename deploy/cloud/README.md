# Deploying Orchester Cloud

How to run Orchester Cloud (the backend `crates/cloud` and its web front `web/cloud`) with Docker at
`orchester-cloud.lurq.dev`. What the service is, its configuration and how it keeps its keys are in
[docs/cloud-hosting.md](../../docs/cloud-hosting.md); its API is in [docs/cloud-api.md](../../docs/cloud-api.md).

**On a host whose ingress is Traefik, managed with Portainer** (the server of `orchester-cloud.lurq.dev`), follow
[PORTAINER.md](PORTAINER.md) instead: `compose.portainer.yaml` runs the images GitHub Actions builds
(`.github/workflows/cloud-images.yml`) behind that Traefik, and the workflow redeploys the stack. The Caddy compose
files below remain for hosts without Traefik; the sections on the SMTP relay, backups, upgrades and logs apply to
both.

## What runs

| File | What |
| --- | --- |
| `compose.yaml` | PostgreSQL 18, the backend (`cloud`, built from `Dockerfile` at the repository root) and Caddy with `Caddyfile`, which serves only `/api/*` and answers 503 on every other path |
| `compose.web.yaml` | Adds the web front (`web`, built from `web/Dockerfile.cloud`) and switches Caddy to `Caddyfile.web`: `/api/*` to the backend, every other path to the web front |
| `cloud.example.toml` | The backend's optional TOML file. Compose does not use it; everything comes from the environment |
| `compose.portainer.yaml` | The Portainer stack behind Traefik, from the GHCR images ([PORTAINER.md](PORTAINER.md)); not combined with the files above |

Always start both files together. Without `compose.web.yaml` there are no sign-up, sign-in or account pages:

```sh
cd deploy/cloud
docker compose -f compose.yaml -f compose.web.yaml up -d --build
```

Below, `dc` stands for `docker compose -f compose.yaml -f compose.web.yaml`, run in `deploy/cloud`.

Only Caddy is published, on ports 80 and 443 (TCP) and 443 (UDP, HTTP/3). The services talk on a compose network with the fixed subnet
`172.30.0.0/24`: Caddy `.2`, cloud `.3`, PostgreSQL `.4`, web `.5`. The backend believes the forwarded client
address only from Caddy and the web front. If another Docker network on the host already uses that subnet, compose
refuses to start; free it.

The backend runs as one instance. Its rate-limit counters are in memory, so do not scale `cloud`.

## The host, DNS and TLS

- A Linux host with Docker Engine and the compose plugin, reachable from the internet on TCP 80 and 443, and UDP 443 for HTTP/3.
- DNS: an `A` record `orchester-cloud.lurq.dev` → the host's IPv4 address, and an `AAAA` record if the host has
  public IPv6. Set them before the first start.
- TLS: Caddy takes the site address from `ORCHESTER_CLOUD_PUBLIC_URL` and gets a public certificate for that host
  by ACME on its first start, and renews it. It needs ports 80 and 443 for that. It redirects plain HTTP to HTTPS,
  and `Caddyfile.web` sends `Strict-Transport-Security: max-age=31536000`. The certificates and Caddy's ACME account
  live in the `caddy_data` volume. Caddy keeps no access log, because the web pages' paths carry one-time tokens.

## The environment

Put the values into `deploy/cloud/.env`, beside the compose files, readable only by the operator. Never commit it.

```sh
ORCHESTER_CLOUD_PUBLIC_URL=https://orchester-cloud.lurq.dev
POSTGRES_PASSWORD=<openssl rand -hex 24>
ORCHESTER_CLOUD_SMTP_URL=smtps://<user>:<password>@<relay host>:465
ORCHESTER_CLOUD_SMTP_FROM=Orchester <no-reply@lurq.dev>
ORCHESTER_CLOUD_KEY_ENCRYPTION_KEY=<openssl rand -hex 32>
ORCHESTER_CLOUD_WEB_SECRET=<openssl rand -base64 32>
```

| Variable | Used by | Meaning |
| --- | --- | --- |
| `ORCHESTER_CLOUD_PUBLIC_URL` | cloud, web, Caddy | The public address, without a trailing `/`. Email links start with it, identity tokens name it as issuer, the web front accepts forms only from it, and Caddy serves and certifies its host |
| `POSTGRES_PASSWORD` | PostgreSQL, cloud | The database password. Compose puts it into the backend's connection URL as it is, so use only URL-safe characters (the hex above). PostgreSQL reads it only when it creates the `postgres_data` volume; changing it later needs `ALTER USER` as well |
| `ORCHESTER_CLOUD_SMTP_URL` | cloud | The SMTP relay: `smtps://user:pass@host:465` (TLS) or `smtp://user:pass@host:587?tls=required` (STARTTLS required). URL-encode the user and password |
| `ORCHESTER_CLOUD_SMTP_FROM` | cloud | The sender of every email |
| `ORCHESTER_CLOUD_KEY_ENCRYPTION_KEY` | cloud | 64 hex characters. It seals the cloud's root key and signing keys in the database, and keys the hashes of deleted owners' emails that keep their abuse limits. Set it once and keep it: with another key the backend refuses to start ([docs/cloud-hosting.md](../../docs/cloud-hosting.md), "Losing the key-encryption key") |
| `ORCHESTER_CLOUD_WEB_SECRET` | web | 32 random bytes in standard base64. It signs the forms' CSRF tokens and seals the session cookie. Changing it signs every browser out |

Compose sets the rest: the backend's `ORCHESTER_CLOUD_DATABASE_URL` (PostgreSQL on the compose network,
`sslmode=disable`), `ORCHESTER_CLOUD_LISTEN`, `ORCHESTER_CLOUD_TRUSTED_PROXIES`, and the web front's
`ORCHESTER_CLOUD_API_URL`. If one of the variables above is unset, `docker compose` refuses to start and names it.
If a value is wrong, the backend or the web front exits at once and logs the variable, for example
`detail="ORCHESTER_CLOUD_SMTP_URL is required"` or `Orchester Cloud web cannot start: ORCHESTER_CLOUD_WEB_SECRET: …`.
Compose restarts it in a loop, which `dc ps` shows as *Restarting*.

**When the web front fails.** Caddy does not wait for the web front, so `/api/*` (the desktop app, team servers'
invites) keeps working while it is down. The pages then answer Caddy's `502 Bad Gateway`. `dc ps` shows `web` as
*Restarting* (a bad configuration, see its log) or *unhealthy*: its health check asks its own `/signin` every 10
seconds and marks it after 3 failures. Watch that state, or `docker inspect --format '{{.State.Health.Status}}'` on
the container, in your monitoring.

## The SMTP relay

The backend sends the sign-up confirmation (its link works for 24 hours), *you already have an account*, the
password reset (its link works for 1 hour), and the invite emails: one to a person with an account, telling them to
open Orchester, and one to an address with no account, asking them to sign up (`/signup?invite=…`). Invite emails
carry only the cloud's own words, never a name a team server chose. Any relay that takes SMTP with a password works. Use a
port with TLS, 465 or 587 with `tls=required`; plain `smtp://` is only for a relay on the same host or private
network. The relay must be allowed to send for the domain of `ORCHESTER_CLOUD_SMTP_FROM`; set up the SPF and DKIM
records the relay asks for in `lurq.dev`'s DNS, or the emails land in spam. A failed send is logged with the
email's purpose only, never its address.

## First start

1. Write `.env` as above.
2. `docker compose -f compose.yaml -f compose.web.yaml up -d --build`. The backend applies its database migrations
   when it starts, makes its root key on the very first start, and publishes its first key set before it listens;
   from then on it rotates its signing keys and signs key sets itself ([docs/cloud-hosting.md](../../docs/cloud-hosting.md),
   "Keys"). `dc logs cloud` names the root key (`root key ready root_public_key=… root_fingerprint=…`).
3. Check it: `https://orchester-cloud.lurq.dev/signup` shows the sign-up page,
   `https://orchester-cloud.lurq.dev/api/v1/keys` answers `200`, and a sign-up gets its email.

## Backups

| What | How | Why |
| --- | --- | --- |
| The database | `dc exec -T postgres pg_dump -U orchester_cloud -Fc orchester_cloud > orchester-cloud-$(date +%F).dump`, daily, kept off the host | Accounts, devices, links, the sealed root and signing keys, key sets and the audit log |
| `ORCHESTER_CLOUD_KEY_ENCRYPTION_KEY` | Keep it apart from the database dumps, for example in a password manager | Without it, the dumps' root and signing keys cannot be opened and the backend refuses to start on them. If it is lost, the cloud needs a new root, which every linked team server pins again ([docs/cloud-hosting.md](../../docs/cloud-hosting.md), "Losing the key-encryption key") |
| `.env` | With the operator's secrets | It holds every other secret |
| `caddy_data` volume | Optional | Caddy gets a new certificate if it is lost |

**Encrypt the database dumps** wherever they are kept, for example with `age` or `gpg` before they leave the host,
or on encrypted storage: they hold every account's email, password hash and display name, the devices' token
hashes, the invites in flight and the sealed root and signing keys (sealed with the key-encryption key, which must not
travel with them). Keep the key that opens them apart from the dumps.

**Rotating the key-encryption key resets the limits of deleted accounts.** A deleted account's team servers keep a
keyed hash of its email (`servers.last_owner`), so signing up again with that email gets no new server or invite
limits (`docs/cloud-api.md`, "Deleting the account"). The hash is keyed by a key derived from
`ORCHESTER_CLOUD_KEY_ENCRYPTION_KEY`; after the key changes, no email matches the stored hashes any more, and those
servers' limits start over for whoever signs up. The hashes stay until each server is linked again.

**Deleted accounts live on in old dumps.** Deleting an account removes it from the database, not from the dumps
taken before. Rotate the dumps (for example, keep 30 days) and delete older ones, so a deleted account is gone from
every copy once its last dump expires; a restore brings back accounts deleted after that dump was taken.

To restore a dump into the running stack:

```sh
dc stop cloud web
dc exec -T postgres dropdb -U orchester_cloud orchester_cloud
dc exec -T postgres createdb -U orchester_cloud orchester_cloud
dc exec -T postgres pg_restore -U orchester_cloud -d orchester_cloud --no-owner < orchester-cloud-<date>.dump
dc start cloud web
```

## Upgrades

1. Take a database dump.
2. Check out the new version of the repository, then `docker compose -f compose.yaml -f compose.web.yaml up -d --build`.
   The new backend applies new migrations when it starts. They are forward-only, each in its own transaction under an
   advisory lock.
3. To go back, restore the dump from step 1 and run the old version. An older backend refuses to start on a
   database a newer one has migrated.

A restart clears the backend's rate-limit counters. Browser sessions survive restarts and upgrades as long as
`ORCHESTER_CLOUD_WEB_SECRET` stays the same. A PostgreSQL major version upgrade (the `postgres:18-alpine` image) is
not automatic: dump, start the new major version on a new volume, and restore.

## Logs

`dc logs cloud` has one record per request: method, route template, status, account id and duration. It never has
emails, passwords, tokens or links. The web front logs only its start and its errors, and Caddy only its own
events.
