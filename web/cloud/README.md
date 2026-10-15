# Orchester Cloud web front

The cloud's browser face (`docs/architecture/server.md`, "The cloud's web front"): sign-up, check your email,
the confirmation landing, sign-in, forgot password and reset password, in the Pencil design (`data/orchester2.pen`,
layer **Cloud**). Next.js App Router, rendered on the server; the browser only posts forms to server actions, and
only the web server calls the cloud's API.

## Configuration

From the environment only:

| Variable | Meaning |
| --- | --- |
| `ORCHESTER_CLOUD_PUBLIC_URL` | Where people reach the site, e.g. `https://orchester-cloud.lurq.dev`. Forms must come from this origin. Plain `http` only on a loopback address |
| `ORCHESTER_CLOUD_API_URL` | The API on the compose network, e.g. `http://cloud:8090` (`/api/v1` is added) |
| `ORCHESTER_CLOUD_WEB_SECRET` | 32 or more random bytes in base64: signs the CSRF tokens and seals the session cookie |

## Work on it

From `web/` (the npm workspace, see `web/README.md`):

```sh
npm ci --ignore-scripts
npm run check          # every package: ESLint, TypeScript, unit tests (Vitest)
npm run test:pages     # next build, then the page tests in the installed Edge against the fake API
```

Run it by hand against the fake API (two shells, from `web/cloud`):

```sh
npm run fake-api       # http://127.0.0.1:4110/api/v1
npm run build
ORCHESTER_CLOUD_PUBLIC_URL=http://localhost:4100 ORCHESTER_CLOUD_API_URL=http://127.0.0.1:4110 \
  ORCHESTER_CLOUD_WEB_SECRET=$(node -e "console.log(require('crypto').randomBytes(32).toString('base64'))") \
  npx next start -H 127.0.0.1 -p 4100
```

Open `http://localhost:4100` (not `127.0.0.1`: the address must match `ORCHESTER_CLOUD_PUBLIC_URL`).

## The fake API's states

`tests/fake-api/server.ts` answers by what is typed:

| Input | Answer |
| --- | --- |
| Sign in `mira.koval@example.com` / `correct horse battery` | Signed in (Mira Koval) |
| Sign in with any other password or address | `401 invalid_credentials` |
| Sign in `unconfirmed@example.com` | `403 email_unconfirmed` |
| Any form with an address starting `limited` | `429`, 900 s for sign-in, 2280 s for emails |
| Any form with an address starting `down` | The cloud fails (`500`/`503`) |
| Any form with an address starting `slow` | Answers after 5 s (the busy buttons) |
| *Resend* for `resend-limit@example.com` | `429` (the resend limit) |
| `/confirm/confirm-ok`, `/reset/reset-ok` | A live link for `mira.koval@example.com` |
| `/confirm/<anything else>`, `/reset/<anything else>` | Used or expired link |
| `/confirm/inspect-down` | The cloud fails while the page loads |
