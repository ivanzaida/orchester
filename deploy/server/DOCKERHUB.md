# Orchester team server

A self-hosted team server for Orchester. It keeps a team's shared projects; its web admin, at `/admin`, sets up the
server, its projects, people and invites. People join from Orchester with a code, or with their Orchester Cloud
account once the server is linked to one. The server needs no internet and works air-gapped.

It runs as two containers, for linux/amd64 and linux/arm64, with this same guide on both image pages:

| Image | What |
| --- | --- |
| `ivanzaida/orchester-server` | The server (Rust), as uid 10001. Its data is in the volume at `/data` |
| `ivanzaida/orchester-server-admin` | The web admin (Next.js, Node 24). The server forwards every path outside `/api` to it; never publish it |

Tags: the release (`0.1.0`), its minor line (`0.1`), and `latest`. A pre-release (`0.2.0-rc.1`) has only its own tag.
Pin a release in production.

## Quick start

On a Linux host with Docker and the compose plugin, a public DNS name with an `A` (and `AAAA`) record pointing to it,
and ports 80 and 443 reachable from the internet:

1. Make a folder for it: `mkdir orchester-server && cd orchester-server`.
2. Save the [compose.yaml](#composeyaml) below as `compose.yaml`, and the [.env.example](#envexample) below as `.env`.
3. In `.env`, set `ORCHESTER_VERSION` to the release (for example `0.1.0`), `ORCHESTER_HOST` to the DNS name, and
   `ORCHESTER_ADMIN_SECRET` to the output of `openssl rand -base64 32`. Then `chmod 600 .env`.
4. `docker compose up -d`. Caddy gets the certificate; `docker compose ps` shows `server` and `admin` as *healthy*
   within a minute.
5. `docker compose logs server` shows `Setup code: ABCD-EFGH`. Open `https://<ORCHESTER_HOST>/admin`, enter the code,
   and choose the server's name and your username. Redeem it soon: whoever reads the container's logs can use it until
   the server has an admin. `docker compose exec server orchester-server setup-code` makes a new one while it has none.

Then create a project in the admin and invite people from its *Invites*: with a code they enter in Orchester, or by
email once the server is linked to Orchester Cloud. Their Orchester joins the server at `https://<ORCHESTER_HOST>`.

Caddy holds the certificate and sends every request to the server, which serves plain HTTP on the stack's private
network and forwards every path outside `/api` to the admin. The admin and the server's private API listener (port
8741, which believes the admin's forwarded headers) are never published. The network has the subnet
`172.29.0.0/24`; if another network on the host has it, set `ORCHESTER_SUBNET_PREFIX`.

### compose.yaml

<!-- inline: compose.yaml -->

### .env.example

Every variable of the three compose files on this page, with how to choose or generate each value:

<!-- inline: .env.example -->

## Without a public DNS name, or air-gapped

`compose.private.yaml` has no Caddy: the server presents its own certificate on port 8740. Save it beside `.env`,
and add `-f compose.private.yaml` to every `docker compose` command.

1. Set `ORCHESTER_SERVER_TLS_NAMES` to the names and addresses people reach the host by, before the first start: the
   server reads them only when it makes its certificate.
2. Air-gapped: on a computer with the internet, `docker pull` both images at the release (add `--platform
   linux/arm64` for an arm64 host), `docker save -o orchester-server.tar <both images>`, carry the archive and the two
   files to the host, and `docker load -i orchester-server.tar` there. Set `ORCHESTER_ADMIN_CLOUD_URL=` (empty).
   Nothing is fetched at run time.
3. `docker compose -f compose.private.yaml up -d`; the logs show the setup code and the certificate's check code.
   Open `https://<host>:8740/admin`. The browser warns about the certificate: compare the SHA-256 it shows with
   `docker compose -f compose.private.yaml exec server orchester-server tls show` before you trust it. People's
   Orchester pins the certificate when they join with a code; they compare its check code once.

<!-- inline: compose.private.yaml -->

## Behind Traefik

`compose.traefik.yaml` is for a host whose Traefik (v2 or v3, with its Docker provider) already holds ports 80 and
443. It publishes nothing: Traefik sends every path on `ORCHESTER_HOST` to the server, sends HSTS and redirects plain
HTTP to HTTPS. Add `-f compose.traefik.yaml` to every `docker compose` command.

1. Look up Traefik's network (default `traefik`), its HTTPS and HTTP entry points (`websecure`, `web`) and its
   certificate resolver (`letsencrypt`), and set the `TRAEFIK_*` variables if they differ.
2. Set `TRAEFIK_ADDRESS` to Traefik's address on that network, **without** the `/16`:
   `docker network inspect traefik --format '{{range .Containers}}{{.Name}} {{.IPv4Address}}{{println}}{{end}}'`.
   The server believes forwarded client addresses (for its limits on failed joins) only from there. If Traefik's
   container is recreated it may get another address: check it after each Traefik upgrade.
3. Set `ORCHESTER_HOST`, `ORCHESTER_VERSION` and `ORCHESTER_ADMIN_SECRET`, then `up -d` and `logs server`.

Only the server joins Traefik's network. The stack's own network is internal, so no other container reaches the
admin or the private API listener; the admin then has no route out, and invites by email reach Orchester Cloud
through the inviter's browser.

<!-- inline: compose.traefik.yaml -->

## Portainer

*Stacks* → *Add stack* → *Web editor*: paste one of the compose files, add the variables of `.env.example` under
*Environment variables*, and deploy. The `server` container's log has the setup code.

## Orchester Cloud (optional)

With `ORCHESTER_ADMIN_CLOUD_URL` set (the default), the admin offers *Link to Orchester Cloud* at the end of the setup
and in its *Settings*. Then people join with their cloud account and invites go out by email. The browser carries
everything between the admin and the cloud; the server never connects to the cloud and checks cloud identities
offline. On the host: `docker compose exec server orchester-server cloud show` (or `cloud off`, `cloud on`,
`cloud unlink`). Without the cloud, leave `ORCHESTER_ADMIN_CLOUD_URL=` empty: people join with codes, and admins sign
in with a pairing code from their Orchester or `orchester-server account-code <username>` on the host.

## Upgrades

1. Take a backup (below).
2. Set `ORCHESTER_VERSION` in `.env` to the new release, then `docker compose pull` and `docker compose up -d`.

The server migrates its database when it starts. Migrations are forward-only: to go back, restore the backup taken
before the upgrade and set the old version.

## Backups

The `server_data` volume holds the database, and with `compose.private.yaml` the server's certificate. Take an online
snapshot while the server runs, with a new file name each time:

```sh
docker compose exec server mkdir -p /data/backups
docker compose exec server orchester-server backup /data/backups/orchester-server-$(date +%F).db
docker compose cp server:/data/backups/orchester-server-$(date +%F).db .
# compose.private.yaml only: the certificate, or every computer confirms a new one after a restore
docker compose -f compose.private.yaml cp server:/data/tls ./tls
```

Keep the snapshots as carefully as the server (they hold its keys and the digests of its codes and device tokens),
and keep `.env` with them. `docker compose down` keeps the volumes; `docker compose down -v` deletes them.

To restore a snapshot, stop the server, copy it in (the database it replaces, with its `-wal` and `-shm` files, moves
to `/data/before-restore`), and start again. For `compose.private.yaml`, add `-f compose.private.yaml` and
`-v "$PWD/tls:/restore-tls:ro"` to bring the certificate back too:

```sh
docker compose stop server
docker compose run --rm --no-deps --user root -v "$PWD/orchester-server-2026-10-16.db:/restore.db:ro" \
  --entrypoint sh server -c 'mkdir -p /data/before-restore && for f in /data/orchester-server.db* /data/tls; do
    [ -e "$f" ] && mv "$f" /data/before-restore/; done; [ ! -d /restore-tls ] || cp -r /restore-tls /data/tls;
    cp /restore.db /data/orchester-server.db && chown -R 10001:10001 /data'
docker compose up -d
```
