# Orchester server admin

The team server's web admin (`docs/architecture/server.md`, "The server admin"): sign-in with Orchester Cloud or a
code, the first setup, projects and their members, invites and settings, people, devices and the server's settings,
in the Pencil design (`data/orchester2.pen`, layer **Server admin**). Next.js App Router, rendered on the server; the
browser only posts forms to server actions and route handlers, and only the admin's server side calls the team
server's API.

The team server is the front door: it answers `/api/*` itself and reverse-proxies every other path here (no
`basePath`): the admin's pages are under `/admin`, a project's own address `/<slug>` says to open it in Orchester, and
Next's assets are at `/_next`. The admin trusts the proxy's `X-Forwarded-Host` and `X-Forwarded-Proto` for its own
origin (CSRF, links, the HTTPS rule) and passes them on to the API; over plain HTTP anywhere but loopback every admin
path answers *This page needs HTTPS*.

## Configuration

From the environment only:

| Variable | Meaning |
| --- | --- |
| `ORCHESTER_ADMIN_SERVER_URL` | The team server's private API listener, e.g. `http://127.0.0.1:8741` (`/api/v1` is added) |
| `ORCHESTER_ADMIN_SECRET` | 32 or more random bytes in standard base64 with padding: signs the CSRF tokens, seals the session, setup and trip cookies |
| `ORCHESTER_ADMIN_CLOUD_URL` | Optional: Orchester Cloud's public address. Unset, cloud sign-in, linking and email invites say they are unavailable |

Cookies: `__Host-orchester_admin` (the admin web token, sealed, 12 hours), `__Host-orchester_admin_presession` (CSRF
before sign-in), `__Host-orchester_admin_state` (a trip to the cloud's `/authorize`: sealed, 10 minutes,
`SameSite=None` because the cloud comes back with a cross-site form post; a link trip carries the session in it),
`__Host-orchester_admin_setup` (the setup code between setup steps 1 and 2, sealed, 15 minutes) and
`__Host-orchester_admin_nothing` (the name the cloud gave, for *Nothing to manage here*). Codes shown once live only
in the answer to the action that made them.

## Work on it

From `web/` (the npm workspace, see `web/README.md`):

```sh
npm ci --ignore-scripts
npm run check               # every package: ESLint, TypeScript, unit tests (Vitest)
npm run test:pages:server   # next build, then the page tests in the installed Edge against the fake API
```

Run it by hand against the fake team server and cloud (two shells, from `web/server`):

```sh
npm run fake-api            # team server API on http://127.0.0.1:4210/api/v1, cloud on http://127.0.0.1:4220
npm run build
ORCHESTER_ADMIN_SERVER_URL=http://127.0.0.1:4210 ORCHESTER_ADMIN_CLOUD_URL=http://127.0.0.1:4220 \
  ORCHESTER_ADMIN_SECRET=$(node -e "console.log(require('crypto').randomBytes(32).toString('base64'))") \
  npx next start -H 127.0.0.1 -p 4200
```

Open `http://localhost:4200/admin`. Docker: `web/Dockerfile.server` (standalone output, `node server/server.js`, port
3100); `deploy/server` runs it beside the team server.

## The fake API's states

`tests/fake-api/server.ts` holds the design's illustrative Kontur team (fixture data, never shipped). It keeps its
state while it runs; `POST /__fake/reset {"scenario": "…"}` starts over, signed out.

| Input | Answer |
| --- | --- |
| Code `ADMN-0001` | Ivan Zaida, an admin |
| Code `OWNR-0001` | Maria Lind, who owns Mobile app and is not an admin |
| Code `NONE-0001` | *Nothing to manage here* |
| Codes starting `LIMT` | `429`, 15 minutes |
| Any other code | `401 invalid_code` |
| Setup code `Q4ZD-8M2K` (scenario `setup`) | Step 2; username `taken` answers `409 username_taken` |
| Cloud sign-in as ivan@ / maria@ / dana@ / stranger@ / stale@example.com | Ivan; Maria; nothing to manage (named Dana Ruiz); `not_invited`; `key_set_stale` |
| Email invites to addresses starting `limit` | `429` (the daily limit) |
| Email invites to addresses starting `no-mfa` | `403 mfa_required`: this admin did not sign in through Orchester Cloud with two-factor authentication |
| Scenario `no-mfa` | Every new invite, codes too, is `403 mfa_required` |
| … starting `offline` | The admin's host cannot reach the cloud: *Send through this browser* |
| … starting `refused` | The cloud refuses the invite (`422`) |
| Scenarios | `full` (default), `setup`, `empty`, `unlinked`, `cloud-off`, `stale`, `no-approval`, `plain` |
