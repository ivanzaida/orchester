# Hosting the shared server

S1a provides accounts and sessions. Project, task, claim and event routes arrive in S1b.
Supported production hosts are Linux x86_64 and aarch64, including Docker. The binary also runs on Windows and macOS
for development and tests.

From `deploy/server`, set `ORCHESTER_HOST` to a DNS name pointing to this host, then run `docker compose up -d --build`.
Caddy obtains and renews HTTPS certificates; ports 80 and 443 must be reachable. The server port is only on the compose
network. The example reserves subnet `172.29.0.0/24`; if it overlaps your network, change the subnet, both service
addresses and `ORCHESTER_SERVER_TRUSTED_PROXIES` together.

On a fresh data volume, `GET https://<host>/api/v1/server` returns `needs_bootstrap: true`. Create the first admin:

```sh
docker compose exec server orchester-server user create --admin alice
```

This command prints a random initial password once, to its own stdout. The server's logs never contain it. Sign in
with `POST /api/v1/auth/sessions` (`username`, `password`, `device`); this session can only change its password or sign
out. `POST /api/v1/auth/password` takes `current_password` and `new_password`, and makes the session usable. Passwords
are 12–256 characters, without composition rules. Usernames are 1–64 ASCII letters, digits, `.`, `_` or `-`, compared
case-insensitively. Initial passwords from admin creation and reset have the same restricted-session flow.

Recovery, on the host:

```sh
docker compose exec server orchester-server user reset-password alice
```

Reset revokes all that user's sessions. Tokens expire after 30 days without use; sign in again to get a new session.
Admin HTTP endpoints create users, update names/admin/disabled flags, reset passwords and revoke sessions.
There is no user deletion endpoint; the last enabled admin cannot be demoted or disabled.

For a standalone binary, run `cargo build --locked --release -p orchester-server`, then
`orchester-server --config server.toml serve`. Copy `deploy/server/server.example.toml` as a starting point.
Without `--config`, the binary reads `server.toml` in the current directory if present, otherwise defaults to
`127.0.0.1:8080`, `./data` and name `Orchester`. Host user commands accept the same `--config` and environment settings;
they can run while the server is running, against the same data directory.
User creation and password reset need write locks, including during the schema check. If another writer holds the
database for the five-second busy timeout, the command exits unsuccessfully, prints no credential, and reports
`server database is busy or locked; retry after the writer releases the lock`. Retry the command after the writer
finishes.

Environment overrides TOML: `ORCHESTER_SERVER_LISTEN`, `ORCHESTER_SERVER_DATA_DIR`, `ORCHESTER_SERVER_NAME`,
`ORCHESTER_SERVER_TLS_CERT`, `ORCHESTER_SERVER_TLS_KEY`, `ORCHESTER_SERVER_TRUSTED_PROXIES` (comma-separated exact IPs
or CIDR ranges, for example `172.29.0.2,10.20.0.0/16`). Both PEM paths are required for direct TLS. Only list proxy
connection addresses or networks you control: forwarded addresses are ignored from other peers. The server combines
all forwarding-header lines and walks the chain from the connection backwards; malformed lists use the peer.
Mapped IPv4 addresses are normalized, and IPv6 rate limits group clients by /64.
Never include a client address or a CIDR range containing clients in `trusted_proxies`: clients inside a trusted range
can forge forwarding hops and evade the address limit. Trust the proxy's connection address as narrowly as possible.

Connections must send their first byte within 2 seconds; after TLS, the same deadline applies to the first HTTP byte.
Header reads, keep-alive idle time and body reads each have a 10-second deadline. The server accepts at most 256
HTTP connections (64 per untrusted peer), with at most 64 concurrent 10-second TLS handshakes (32 per untrusted peer).
The handshake cap is fixed at 32 so several desktops behind a NAT can connect together.
Peers matching `trusted_proxies` are exempt from both per-peer caps so all Caddy clients do not share a single
64-connection budget. They still count toward both global caps and obey the same deadlines. Per-client fairness for
sign-in comes from request limits using the validated forwarded client address.
Peer caps use the connection address, normalize mapped IPv4, and group IPv6 by /64; excess connections are closed
immediately. Sign-in has username and address failure limits; password changes separately block a user after 5 failed
current-password checks in 15 minutes, for 15 minutes.
Both return `429` with `Retry-After`. A password-change block does not prevent that user's sign-in.
Five failed sign-ins lock a username for 15 minutes, even from different client addresses. There is no dedicated
admin unlock command or endpoint. A password reset changes the credential and revokes sessions, but does not clear
the running server's lockout counters; wait for `Retry-After` to expire. A server restart clears all in-memory rate
limits, including address and password-change counters.
Failures and in-flight checks share the same budget, reserved atomically before entering the hashing queue and
rechecked after acquiring a slot. Successful checks release their reservation. A full in-flight budget returns
`Retry-After: 1`; a failure block returns its remaining duration. Requests rejected by either check do no Argon2 work.

The data directory contains `orchester-server.db` (SQLite WAL) with forward-only versioning and append-only audit
records. Create an online snapshot, including committed WAL contents, while the server is running:

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
For a standalone binary: `orchester-server --config server.toml backup /secure/backups/snapshot.db`. The destination's
parent directory must exist. Unix snapshots are created with mode 0600; on Windows they inherit the directory ACLs.
Protect the copied snapshot as carefully as the live data: it contains password hashes and session-token digests.

To restore, stop the server and host commands, preserve the old database together with its `-wal` and `-shm` files,
then put the snapshot at `<data_dir>/orchester-server.db` in a clean data directory and start the server. Copying only
the live database during writes can miss its WAL; use the backup command instead. Data is not encrypted at rest.
The container runs as uid/gid 10001; bind-mounted data must be writable by that user. `docker compose down` preserves
the named volumes. Linux SIGTERM and Ctrl+C request graceful shutdown.
