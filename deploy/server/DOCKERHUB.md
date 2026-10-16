# Orchester team server

A self-hosted team server for Orchester: it keeps a team's shared projects,
and its web admin sets up the server, its projects, people and invites. People join from Orchester with a code, or
with an Orchester Cloud account once the server is linked to one. The server needs no internet and works air-gapped.

Two images, run together, for linux/amd64 and linux/arm64:

| Image | What |
| --- | --- |
| `ivanzaida/orchester-server` | The server (Rust). Its data (a SQLite database, and the certificate it makes when it presents its own) is in the volume at `/data`. It runs as uid 10001 |
| `ivanzaida/orchester-server-admin` | The web admin (Next.js on Node 24). The server forwards every path outside `/api` to it; never publish it |

Tags: the release (`0.1.0`), its minor line (`0.1`), and `latest`. Pre-releases (`0.2.0-rc.1`) get only their own
tag. Pin a release in production.

## Quick start

On a Linux host with Docker, a public DNS name pointing to it, and ports 80 and 443 open:

1. Save `compose.yaml` and `.env.example` (both below) in a new folder, and rename `.env.example` to `.env`.
2. In `.env`, set `ORCHESTER_VERSION` to the release, `ORCHESTER_HOST` to the DNS name and `ORCHESTER_ADMIN_SECRET`
   to the output of `openssl rand -base64 32`. Then `chmod 600 .env`.
3. `docker compose up -d`. Caddy gets the certificate.
4. `docker compose logs server` shows `Setup code: XXXX-XXXX`. Open `https://<DNS name>/admin` and enter it to make the
   first admin.

The release on GitHub also has `compose.private.yaml` (no public DNS name, or air-gapped: the server's own
certificate on port 8740), `compose.traefik.yaml` (behind a Traefik the host already runs) and a Portainer app
template.

## Upgrades and backups

Back up first, then set `ORCHESTER_VERSION` to the new release, `docker compose pull` and `docker compose up -d`. The
server migrates its database when it starts.

```sh
docker compose exec server mkdir -p /data/backups
docker compose exec server orchester-server backup /data/backups/orchester-server-$(date +%F).db
docker compose cp server:/data/backups/orchester-server-$(date +%F).db .
```

With `compose.private.yaml`, also keep the server's certificate (`docker compose -f compose.private.yaml cp
server:/data/tls ./tls`), or every computer confirms a new one after a restore. Keep `.env` with the backups.
