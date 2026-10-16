# Running Orchester Cloud

`orchester-cloud` (`crates/cloud`) is the one identity service the person runs for everyone
([architecture/server.md](architecture/server.md), rows 41, 47, 48 and 54). It holds accounts, devices, its root key and the
signing keys of identity tokens, never project data. Its API is in [cloud-api.md](cloud-api.md). Team servers are
hosted separately ([server-hosting.md](server-hosting.md)).

Supported hosting is Linux or Docker behind the person's domain and a public certificate (Caddy in the example).
The service needs:

- a PostgreSQL database;
- an SMTP relay;
- a key-encryption key from its environment.

It makes its root key at its first start and keeps its signing keys and key sets itself
([Keys](#keys)): deploying is setting the environment and starting it.

The binary also builds and runs on Windows and macOS.

## Configuration

Every setting comes from the environment as `ORCHESTER_CLOUD_<KEY>`. A TOML file (`--config cloud.toml`, or
`cloud.toml` in the working directory) may give values, and the environment wins over it
([deploy/cloud/cloud.example.toml](../deploy/cloud/cloud.example.toml)). Nothing is hard-coded: the person's
deployment sets `ORCHESTER_CLOUD_PUBLIC_URL=https://orchester-cloud.lurq.dev` in its environment.

| Key | Required | Meaning |
| --- | --- | --- |
| `LISTEN` | no (`127.0.0.1:8090`) | Address and port to listen on; `0.0.0.0:8090` in the container |
| `PUBLIC_URL` | yes | Where people reach the cloud and its web front, without a trailing `/`. Email links point to `<it>/confirm/<token>` and `<it>/reset/<token>`, and identity tokens name it as their issuer |
| `DATABASE_URL` | yes, secret | A PostgreSQL connection string that names its `sslmode`. `sslmode=require` (TLS verified against the OS trust store) for a database across a network. `sslmode=disable` is accepted only for a host on the local network: a loopback or private address, a Unix socket, or a single-label name such as the compose service `postgres`. Without `sslmode`, or with `prefer`, the cloud refuses to start |
| `SMTP_URL` | yes, secret | The relay, as lettre reads it: `smtps://user:pass@host:465` (TLS), `smtp://user:pass@host:587?tls=required` (STARTTLS required), or plain `smtp://host:25` only for a relay on the same host or a private network. URL-encode the user and password |
| `SMTP_FROM` | yes | The sender, e.g. `Orchester <no-reply@lurq.dev>` |
| `KEY_ENCRYPTION_KEY` | yes, secret | 64 hex characters (32 random bytes, e.g. `openssl rand -hex 32`). It seals the root key's and the signing keys' private halves in the database. Set it once and keep it: with another key the cloud refuses to start ([Losing the key-encryption key](#losing-the-key-encryption-key)) |
| `TRUSTED_PROXIES` | no | Comma-separated IPs or CIDRs whose `X-Forwarded-For` hop is believed: Caddy, and the web front's server once it calls the API |

Secrets never reach the log, `Debug` output or error answers. The log has one record per request: method, route
template, status, account id and duration. It never has emails, passwords, tokens, link tokens or keys. The log
filter is fixed to the cloud's own records (`orchester_cloud=info`). Do not widen it with a debug build:
`tokio-postgres` logs every statement's parameters at debug level, and those include emails and password hashes.

## Database

The migrations are compiled into the binary (`crates/cloud/migrations/`). They are applied at every start, each in
its own transaction under a PostgreSQL advisory lock, so two instances starting together apply each one once. They
are recorded in `schema_migrations`. A database at a newer version than the binary knows is refused at start; run
the newer binary or restore a backup. Backups, replication and point-in-time recovery are the database server's
job. Back up the key-encryption key separately from the database: the database alone does not open the root key or
the signing keys.

Tables: `accounts`, `email_links`, `devices`, `root_key`, `signing_keys`, `key_sets`, `servers`, `server_accounts`,
`invites`, `audit_events` (append-only) and `schema_migrations`. A sweep every minute deletes expired links and
invites, and accounts that were never confirmed once their last link has expired.

## Keys

Identity tokens are signed with **signing keys** the cloud holds. A **key set** lists those keys and is signed by
the cloud's **root key**. Team servers pin the root key when they link and believe only key sets it signed. There is
nothing to run by hand:

- **The root key** (Ed25519) is made at the cloud's first start and stored in the `root_key` table, its private
  half sealed with `KEY_ENCRYPTION_KEY` (AES-256-GCM, as the signing keys are). Every later start loads it; the log
  names it at every start (`root key ready root_public_key=… root_fingerprint=…`): the fingerprint is what team
  servers show once linked, and the public key is what their admins pin should the root ever change. The table holds
  one row at most, and the root is made under a PostgreSQL advisory lock: replicas that start together make one root,
  and the others load it.
- **Signing keys** sign for 30 days and stay listed 60 days longer. A new one is made once no key would sign a week
  from now. While another key signs, the new one starts two days later, so team servers (which refresh the key set
  daily) have it before any token signed with it arrives. The very first key signs at once.
- **Key sets** are valid for 30 days. The cloud signs a new one after each rotation, at least weekly, and whenever
  the newest no longer verifies with its root or names another `PUBLIC_URL`. `GET /keys` serves the newest.

The cloud keeps this schedule at its start, before it listens, and every minute while it serves, under the same
lock, so each step happens once however many replicas run. It signs tokens only with a key the newest unexpired key
set lists. A cloud stopped for more than 30 days answers `POST /identity-tokens` with `503 keys_unavailable` only
until its next start, which signs a fresh key set.

### Losing the key-encryption key

The root key and the signing keys open only with the key-encryption key they were sealed with. Started with another
one (lost, mistyped or changed), the cloud stops at once with `error_kind=WrongKeyEncryptionKey` and *does not
open the root key in the database*. It never makes a new root in its place: a new root would silently break every
team server that pinned the old one.

- **Changed or mistyped:** set the key the database was first started with and start again.
- **Lost for good:** the root cannot be recovered. To start over with a new root, delete it and the keys it sealed
  (`DELETE FROM key_sets; DELETE FROM signing_keys; DELETE FROM root_key;`) and start the cloud with a new
  key-encryption key; it makes a new root. Accounts, devices and linked servers stay, but every linked team server
  stops accepting cloud sign-ins until its admin pins the new root's public key from the log (the server admin's
  Orchester Cloud settings, *Pin another key…*, or `orchester-server cloud pin <root public key>`) or links the
  server again. Devices that already joined keep working.

### Upgrading from an offline root

Builds before 2026-10-16 used a root key kept offline and an `ORCHESTER_CLOUD_ROOT_PUBLIC_KEY` setting, now gone (a
`root_public_key` left in `cloud.toml` is refused as an unknown setting; the environment variable is ignored). On
its first start, the new build makes its own root and removes the key sets the offline root signed, and the log
warns that team servers linked before must pin the new root. The signing keys stay. Re-pin or re-link those servers
as above.

## Docker with Caddy

[deploy/cloud](../deploy/cloud) has a `Dockerfile` (built from the repository root), `compose.yaml` with the cloud,
PostgreSQL 18 and Caddy, and `compose.web.yaml`, which adds the Next.js web front. Started together, Caddy takes the
certificate for the host of `ORCHESTER_CLOUD_PUBLIC_URL`, sends `/api/*` to the cloud and every other path to the web
front, whose server calls `http://cloud:8090/api/v1` with the browser's `X-Forwarded-For`. Caddy keeps no access
log, since the web front's paths carry one-time tokens. [deploy/cloud/README.md](../deploy/cloud/README.md) has the
environment, DNS, TLS, the SMTP relay, the first start, backups and upgrades.

## Running the tests

`cargo test -p orchester-cloud --test cloud <filter>` runs the cloud against a real PostgreSQL (`::` runs every
test). Without a filter it runs every test when `ORCHESTER_CLOUD_TEST_DATABASE_URL` is set. Otherwise it runs
nothing and says so, so a plain `cargo test --workspace` needs no database. CI runs the tests in a Linux job with a
PostgreSQL service. With
`ORCHESTER_CLOUD_TEST_DATABASE_URL` set (a server URL whose user may create databases, e.g.
`postgres://postgres:pw@127.0.0.1:5432`), it uses that server. Otherwise it starts a throwaway `postgres:18-alpine`
container on 127.0.0.1 (`orchester-cloud-test-…`) and removes it when the run ends. Each test creates and drops
its own database. Without Docker or the variable, the run fails; it never passes without a database. At the end
the run checks its whole log for every password, token, link, key and email the tests used.
`cargo test -p orchester-api --features identity --test identity identity` checks tokens and key sets over explicit
keys and times, and `cargo test -p orchester-cloud --test limits limits` the bounded limiters and hashing pool,
both with no database.

## Clippy runs in two parts, on purpose

`tools/check.ps1` and CI run clippy twice: `cargo clippy --workspace --exclude orchester-cloud …`, then
`cargo clippy -p orchester-cloud …`. A single `cargo clippy --workspace --all-targets --all-features -- -D warnings`
stays red.

The reason: actix-web's crates (actix-codec, actix-http, actix-router) depend on `tracing` with its `log` feature,
and nothing can turn that off. In one build with the cloud, Cargo turns that feature on for every crate. Every
`tracing::…!` macro then expands into more branches, and clippy's `cognitive_complexity` counts them. That pushes
19 existing functions in the server, runner, workspace, app and the frozen desktop over the limit. `cargo clippy -p
orchester-server --features tracing/log` reproduces it without the cloud.

The split is deliberate. Do not refactor those functions to make the combined run pass: they are within the limit
in every build that ships them.
