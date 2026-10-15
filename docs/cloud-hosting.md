# Running Orchester Cloud

`orchester-cloud` (`crates/cloud`) is the one identity service the person runs for everyone
([architecture/server.md](architecture/server.md), rows 41, 47, 48 and 54). It holds accounts, devices and the
signing keys of identity tokens, never project data. Its API is in [cloud-api.md](cloud-api.md). Team servers are
hosted separately ([server-hosting.md](server-hosting.md)).

Supported hosting is Linux or Docker behind the person's domain and a public certificate (Caddy in the example).
The service needs:

- a PostgreSQL database;
- an SMTP relay;
- a key-encryption key from its environment;
- the public half of a root key that stays on an offline machine.

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
| `KEY_ENCRYPTION_KEY` | yes, secret | 64 hex characters (32 random bytes, e.g. `openssl rand -hex 32`). It seals the signing keys' private halves in the database. Losing it means rotating to a new signing key; changing it makes the stored keys unusable |
| `ROOT_PUBLIC_KEY` | yes | The root key's public half, printed by `orchester-cloud root generate` |
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
job. Back up the key-encryption key separately from the database: the database alone does not open the signing
keys.

Tables: `accounts`, `email_links`, `devices`, `signing_keys`, `key_sets`, `audit_events` (append-only) and
`schema_migrations`. A sweep every minute deletes expired links, and accounts that were never confirmed once their
last link has expired.

## The key ceremony

Identity tokens are signed with **signing keys** the cloud holds. A **key set** lists those keys and is signed by
the **root key**, which never leaves an offline machine. Team servers pin the root key's fingerprint and believe
only key sets it signed. Key set rules:

- a key set is valid for 30 days;
- publish a new one at least weekly;
- a signing key signs for 30 days and stays listed 60 days longer.

The cloud signs only with a key the newest published, unexpired key set lists. If no such key set exists,
`POST /identity-tokens` answers `503 keys_unavailable`: a ceremony was missed.

On the **offline machine**, once, with a release build of `orchester-cloud`:

```sh
orchester-cloud root generate root.key   # writes the root key (mode 0600 on Unix), prints ROOT_PUBLIC_KEY and the fingerprint
```

On Windows, keep `root.key` in a folder of your user profile: a new file gets the folder's access list. Put
`ROOT_PUBLIC_KEY` into the cloud's environment.

**Every week**, and with a rotation every 30 days:

```sh
# on the cloud host
orchester-cloud keys rotate                         # before the very first key set, then every 28 days at most
orchester-cloud keys prepare statement.json         # the statement to sign: every key still listed

# on the offline machine
orchester-cloud keys sign statement.json root.key signed.jws   # prints what it signs

# on the cloud host
orchester-cloud keys publish signed.jws             # checks the root signature, the issuer and every key, then publishes
```

With Docker, the container's user writes to `/tmp`:

```sh
docker compose exec cloud orchester-cloud keys prepare /tmp/statement.json
docker compose cp cloud:/tmp/statement.json .
# … sign offline, then:
docker compose cp signed.jws cloud:/tmp/signed.jws
docker compose exec cloud orchester-cloud keys publish /tmp/signed.jws
```

`keys rotate` makes a key the cloud uses only once a published key set lists it. While another key signs, the new
one starts signing two days after the rotation, so team servers (which refresh the key set daily) have it before
any token signed with it arrives. The previous key signs until then. So rotate at least two days before the current
key's 30 days end, which is every 28 days at most; `keys sign` prints every key's dates. The very first key signs at once.

`keys sign` refuses a statement prepared more than a day ago, and one whose lifetimes exceed the rules. Team
servers apply the same limits, so even a stolen root key cannot make a long-lived key set.

## Docker with Caddy

[deploy/cloud](../deploy/cloud) has a `Dockerfile` (built from the repository root) and `compose.yaml` with the
cloud, PostgreSQL 18 and Caddy. Caddy takes the certificate for the host of `ORCHESTER_CLOUD_PUBLIC_URL` and sends
`/api/*` to the cloud. It keeps no access log, since the web front's paths carry one-time tokens. Every other path answers that the web front is not deployed yet. When the Next.js web front is deployed, it
takes those paths. Its server calls `http://cloud:8090/api/v1` with the browser's `X-Forwarded-For`, and its
address joins `ORCHESTER_CLOUD_TRUSTED_PROXIES`.

Put the values into `deploy/cloud/.env` (never commit it):

```sh
ORCHESTER_CLOUD_PUBLIC_URL=https://orchester-cloud.lurq.dev
POSTGRES_PASSWORD=…
ORCHESTER_CLOUD_SMTP_URL=smtps://…:…@…:465
ORCHESTER_CLOUD_SMTP_FROM=Orchester <no-reply@lurq.dev>
ORCHESTER_CLOUD_KEY_ENCRYPTION_KEY=…
ORCHESTER_CLOUD_ROOT_PUBLIC_KEY=…
```

Then start it with `docker compose up -d --build` in `deploy/cloud`, and run the key ceremony.

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
