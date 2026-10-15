# Hosting the shared server

A team server holds a team's shared projects (`docs/architecture/server.md`). Since API 3 (S1c) nobody has a password
on it: a device joins once, with an Orchester Cloud identity token or, without the internet, with an 8-character code,
and then works with the device token the server issued it. Supported production hosts are Linux x86_64 and aarch64,
including Docker. The binary also runs on Windows and macOS for development and tests. The server needs no internet,
no public DNS and no public certificate authority.

## First start

On its first start the server makes its identity (an id, an Ed25519 key for what it signs for the cloud, the key of
its codes, all in its database) and, with the default `tls = "self_signed"`, its own certificate. It prints to its
console (stdout, never the log):

```text
Certificate check code: 3F7A-92C1-KD8M-QX4T
Setup code: ABCD-EFGH (valid 24 hours; open the server admin and enter it to make the first admin)
```

The **check code** is the certificate's SHA-256 in 16 Crockford characters. People compare it when they connect a
desktop with a code; a cloud invite carries the full fingerprint, so then nobody compares anything. It is printed at
every start, and `orchester-server tls show` prints it again.

The **setup code** makes the first admin. It is printed at every start until the server has an admin; a new one ends
the one before. Redeem it with `POST /api/v1/auth/code { code, device, username, display_name }`: the answer is the
first admin's account and a device token (the server admin, phase SA, does this from its setup page). With an admin,
the server prints no setup code; `orchester-server setup-code` makes a fresh one while it has none.

`GET /api/v1/server` answers `{ name, api_version: 3, needs_setup, cloud }`.

## Certificates (`tls`)

| `tls` | Certificate | How desktops trust it |
| --- | --- | --- |
| `self_signed` (default) | Generated on the first start: ECDSA P-256, valid 20 years, kept in `<data_dir>/tls/` (`cert.pem`, `key.pem`; the key mode 0600 on Unix). Names: `localhost`, the loopback addresses, the listen address when it names one, the computer's name when `HOSTNAME`/`COMPUTERNAME` gives it, and `tls_names` | Pinned: exactly this certificate, whatever its names or dates |
| `pem` | The team's own `tls_cert` and `tls_key`, e.g. from an internal CA | The OS trust store, or pinned |
| `off` | None: plain HTTP behind a proxy that holds the certificate (the Docker example) | The proxy's certificate through the OS trust store |

`orchester-server tls regenerate` replaces the generated certificate (after a key leak) and prints the new check code;
a running server presents it after its next start. Every pin breaks on purpose: each desktop confirms the new check
code or rejoins through an invite. Names in `tls_names` are read only when the certificate is generated.

## Codes, invites and devices

| Code | Made by | Valid | Gives |
| --- | --- | --- | --- |
| Setup | the server, while it has no admin | 24 hours | the first admin account and a device token |
| Invite | an admin, or a project's owner: `POST /invites { kind, delivery: "code", … }` | 72 hours | a new account (`server`), or a new account that is a member of one project with a role (`project`) |
| Account | an admin, for an account that is not an admin (`POST /invites { kind: "account", user, delivery: "code" }`), or `orchester-server account-code <username>` on the host for any account | 24 hours | a device token for an account that lost every device |
| Pairing | a joined device: `POST /auth/pairings` | 10 minutes | another device of the same account (at most 3 open per account) |

Codes are 8 characters shown `ABCD-EFGH`, read without regard to case or the hyphen. They work once. The server keeps
only their HMAC and compares each in constant time; one answer (`401 invalid_code`) covers unknown, used and expired
codes. 10 failed redemptions or cloud joins from one client network (an IPv4 address, an IPv6 /48) in 15 minutes
block that network for 15 minutes. 200 failed redemptions on the whole server in any hour pause redemption until fewer
than 200 remain in the last hour, at least 15 minutes (`429` with `Retry-After`); failed cloud joins have a budget of
their own. Redemptions from the host itself (loopback) are exempt from the server-wide pause, so the owner can always
redeem the setup code there. A server holds at most 50 open codes (`409 too_many_codes`, also for a pairing code).
Failed joins are counted per address and hour in `failed_joins`, kept 90 days.

Invites by email (`delivery: "cloud"`, open 7 days) answer with `signed`: the invite signed with the server's key, for
the server admin's page to carry to Orchester Cloud. Invites list without their codes (`GET /invites`) and are revoked
with `DELETE /invites/{id}`; an ended invite forgets its email.

Device tokens are `orc_` and 64 hex characters, stored as SHA-256, and expire after 90 days without use. A device signs
itself out with `DELETE /auth/sessions/current`; an admin signs all of a person's devices out with
`DELETE /admin/users/{id}/sessions`, and disabling an account signs it out too.

## Orchester Cloud

A server is linked to its owner's Orchester Cloud account with `PATCH /server/cloud` (admins): `{ "link": "start" }`
returns the server's signed link statement (server id, name, address, key, and `fingerprint`: the SHA-256 of its own
certificate in hex, which apps pin, or none without one) for the cloud, and
`{ "link": { "root_key", "key_set", "identity_token"? } }` pins the cloud's root key and caches the key set it signed.
From then on `POST /auth/cloud { identity_token, key_set?, device }` joins a device with an identity token verified on
the server, without the internet: the account linked to the token's cloud account, or an open email invite for its
confirmed email; otherwise `403 not_invited`. Each token works once. A newer key set an app carries replaces the cached
one; once the cached key set expires (30 days after it was issued) cloud joins answer `409 key_set_stale` until a newer
one arrives, while joined devices and codes keep working. With a device token, the same request links the caller's
code account to the identity.

The owner controls the link on the host at once, also while the server runs:

```sh
orchester-server cloud show
orchester-server cloud off            # stop accepting identity tokens; joined devices keep working
orchester-server cloud on
orchester-server cloud pin <root public key>   # after a root compromise; cloud joins wait for a key set it signed
orchester-server cloud unlink
```

**Join approval** (`PATCH /server/settings { join_approval: true }`, off by default): a cloud join of an existing
account then answers `202 { approval }`. One of that account's devices or an admin approves or refuses it
(`GET /auth/approvals`, `POST /auth/approvals/{id} { approve }`); the joining app asks again with `approval: <id>` and
gets its device token once approved (`403 approval_refused` when refused, used or expired; approvals wait an hour).

Not built yet: the server does not reach the cloud itself (it neither fetches key sets nor sends or revokes invites
there); the server admin and the apps carry both.

## Projects

Admins create projects by name and slug (`POST /projects { idempotency_key, name, slug, key, remote_hint?, tasks }`);
the creator is the project's owner. A slug is unique, fixed once created, and not `admin`, `api` or `auth`. A
project's address is the server's address and its slug: set `address` (`https://orchester.example.com`) when the
server is behind a proxy, otherwise the address a request was sent to is used. `GET /projects?slug=` finds a project
for its members (`403 not_member` with its name and owners for others). `POST /projects/{id}/link` uploads a linked
checkout's tasks: into a project with no tasks they keep their numbers, later ones get new numbers. The repository
remote is only a hint (`remote_hint`).

## Docker

From `deploy/server`, set `ORCHESTER_HOST` to a DNS name pointing to this host, then run `docker compose up -d --build`.
Caddy obtains and renews HTTPS certificates; ports 80 and 443 must be reachable. The server serves plain HTTP on the
compose network only (`ORCHESTER_SERVER_TLS=off`) and knows its public address from `ORCHESTER_SERVER_ADDRESS`. The
example reserves subnet `172.29.0.0/24`; if it overlaps your network, change the subnet, both service addresses and
`ORCHESTER_SERVER_TRUSTED_PROXIES` together.

The setup code is in the server's console output: `docker compose logs server` shows it (it is the container's
stdout, not a log record), or run `docker compose exec server orchester-server setup-code`.

## Upgrading from API 2

The first start of this version migrates the database (schema 3) in one transaction: accounts keep their ids, names,
admin and disabled flags and memberships; password hashes are dropped; sessions become device tokens (shown as
`before_3`), except those still limited to a password change, which are revoked. Projects made by repository get a
slug from the repository's name (`github.com/acme/kontur` → `kontur`, then `kontur-2`; reserved names get a suffix)
and keep the repository as their hint and their id. A server that already has an admin prints no setup code. A person
who has no signed-in device left gets an account code: `orchester-server account-code <username>`.

The old desktop app's password sign-in no longer works against this server, by design.

What else changes for an existing installation:

- **TLS is on by default.** `tls` defaults to `self_signed`: a server that served plain HTTP before now generates a
  certificate on its first start and serves HTTPS. Behind a proxy that holds the certificate (the Docker example),
  set `tls = "off"` (`ORCHESTER_SERVER_TLS=off`) explicitly.
- **Your own certificate needs `tls = "pem"`.** `tls_cert` and `tls_key` are refused unless `tls = "pem"` is set
  with them; add it to a configuration that names them.
- **The setup code is in the console output.** In Docker that is `docker logs` / `docker compose logs server`: anyone
  who can read the container's logs can redeem it until the server has an admin. Redeem it promptly; a new one
  (`orchester-server setup-code`) ends the one before.
- **The generated key on Windows** inherits the access list of its folder (`<data_dir>/tls/`): unlike Unix, where it
  gets mode 0600, it gets no restricted access list of its own. Keep the data directory readable only by the server's
  account.

## Configuration

For a standalone binary, run `cargo build --locked --release -p orchester-server`, then
`orchester-server --config server.toml serve`. Copy `deploy/server/server.example.toml` as a starting point.
Without `--config`, the binary reads `server.toml` in the current directory if present, otherwise defaults to
`127.0.0.1:8080`, `./data`, name `Orchester` and `tls = "self_signed"`. Host commands accept the same `--config` and
environment settings and can run while the server is running, against the same data directory:
`serve`, `backup <path>`, `tls show`, `tls regenerate`, `setup-code`, `account-code <username>`,
`cloud show|on|off|unlink`, `cloud pin <root public key>`. Codes are printed only to the command's own output.
Commands that write need the database's write lock, including during the schema check. If another writer holds it for
the five-second busy timeout, the command exits unsuccessfully, prints no code, and reports
`server database is busy or locked; retry after the writer releases the lock`.

Environment overrides TOML: `ORCHESTER_SERVER_LISTEN`, `ORCHESTER_SERVER_DATA_DIR`, `ORCHESTER_SERVER_NAME`,
`ORCHESTER_SERVER_ADDRESS`, `ORCHESTER_SERVER_TLS` (`self_signed`, `pem`, `off`), `ORCHESTER_SERVER_TLS_NAMES`
(comma-separated), `ORCHESTER_SERVER_TLS_CERT`, `ORCHESTER_SERVER_TLS_KEY` (both, and only, with `tls = "pem"`),
`ORCHESTER_SERVER_TRUSTED_PROXIES` (comma-separated exact IPs or CIDR ranges, for example `172.29.0.2,10.20.0.0/16`).
Only list proxy connection addresses or networks you control: forwarded addresses are ignored from other peers. The
server combines all forwarding-header lines and walks the chain from the connection backwards; malformed lists use the
peer. Mapped IPv4 addresses are normalized; failed-join limits group IPv6 clients by /48, connection caps by /64.
Never include a client address or a CIDR range containing clients in `trusted_proxies`: clients inside a trusted range
can forge forwarding hops and evade the address limit. Trust the proxy's connection address as narrowly as possible.

Connections must send their first byte within 2 seconds; after TLS, the same deadline applies to the first HTTP byte.
Header reads, keep-alive idle time and body reads each have a 10-second deadline. The server accepts at most 256
HTTP connections (64 per untrusted peer), with at most 64 concurrent 10-second TLS handshakes (32 per untrusted peer).
The handshake cap is fixed at 32 so several desktops behind a NAT can connect together.
Peers matching `trusted_proxies` are exempt from both per-peer caps so all Caddy clients do not share a single
64-connection budget. They still count toward both global caps and obey the same deadlines. Per-client fairness for
joins comes from the redemption limits using the validated forwarded client address. Failures and attempts in flight
count against the same budgets, so concurrent guesses cannot overrun them. A server restart clears the in-memory limits.

## Backups

The data directory contains `orchester-server.db` (SQLite WAL) with forward-only versioning and append-only audit
records, and `tls/` with the generated certificate and its key. Create an online snapshot of the database, including
committed WAL contents, while the server is running:

```sh
docker compose exec server mkdir -p /data/backups
docker compose exec server orchester-server backup /data/backups/orchester-server-2026-10-03.db
docker compose cp server:/data/backups/orchester-server-2026-10-03.db ./orchester-server-2026-10-03.db
```

Use a fresh filename for each snapshot: the command refuses to overwrite any existing file, including the live
database. It exits successfully only after SQLite's online backup completes; no `sqlite3` installation is required.
The command opens the existing source database read-only: it never creates a source database, changes its journal
mode or runs migrations. This allows backup while a WAL writer holds an IMMEDIATE transaction. It is a physical
snapshot, so even a database with a newer schema can be copied without interpreting or changing its version;
restoring that schema still requires a compatible server binary.
The database is copied in one step under a read snapshot, so ongoing commits cannot keep restarting the backup.
This holds the read lock for the copy and may briefly delay writers on non-WAL databases. If SQLite cannot complete
the step because the database is busy or locked, the command reports failure, removes its incomplete file, and can
be retried with the same filename.
With the server stopped, a read-only backup can leave SQLite's `orchester-server.db-wal` and `-shm` files beside the
database. Run host commands as the server's user (uid 10001 in the image, e.g. `docker compose exec server …`), not as
root against a bind-mounted directory, or the server may later fail to open root-owned files.
For a standalone binary: `orchester-server --config server.toml backup /secure/backups/snapshot.db`. The destination's
parent directory must exist. Unix snapshots are created with mode 0600; on Windows they inherit the directory ACLs.
Protect the copied snapshot as carefully as the live data: it holds the server's keys, code HMACs and device-token
digests. Back up `tls/` too, or desktops must confirm a new certificate after a restore.

To restore, stop the server and host commands, preserve the old database together with its `-wal` and `-shm` files,
then put the snapshot at `<data_dir>/orchester-server.db` in a clean data directory and start the server. Copying only
the live database during writes can miss its WAL; use the backup command instead. Data is not encrypted at rest.
The container runs as uid/gid 10001; bind-mounted data must be writable by that user. `docker compose down` preserves
the named volumes. Linux SIGTERM and Ctrl+C request graceful shutdown.
