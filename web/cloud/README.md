# Orchester Cloud web front

The cloud's browser face (`docs/architecture/server.md`, "The cloud's web front"): sign-up, check your email,
the confirmation landing, sign-in, forgot password and reset password, in the Pencil design (`data/orchester2.pen`,
layer **Cloud**), and the Account page. The two-factor code step (`/signin/code`) and the Account page's *Security*
have no drawn design yet and follow the pages beside them. Next.js App Router, rendered on the server; the browser only posts forms to server actions, and
only the web server calls the cloud's API.

Besides Next.js and React it depends on `qrcode-generator` (MIT, no dependencies of its own), which draws the QR
code of the two-factor setup's `otpauth://` link on the server side as one SVG path: a QR encoder (Reed-Solomon error
correction, masks, versions) is not a few lines of code. The page needs no script for it.

## Configuration

From the environment only:

| Variable | Meaning |
| --- | --- |
| `ORCHESTER_CLOUD_PUBLIC_URL` | Where people reach the site, e.g. `https://orchester.cloud`. Forms must come from this origin or one of `ORCHESTER_CLOUD_EXTRA_ORIGINS`. Plain `http` only on a loopback address |
| `ORCHESTER_CLOUD_EXTRA_ORIGINS` | Optional: other origins the site also answers at and takes forms from, comma-separated, each `https://host[:port]` with no path (plain `http` only on a loopback address); unset or empty for none. For the time the site answers on two domains, e.g. `https://orchester-cloud.lurq.dev` beside `https://orchester.cloud` (`deploy/cloud/README.md`, "Moving to another domain"). The cookies stay host-only (`__Host-`), so each origin has its own sign-in, and the site's redirects are relative, so a visitor stays on the host they came to |
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

Open `http://localhost:4100` (not `127.0.0.1`: the address must match `ORCHESTER_CLOUD_PUBLIC_URL`, or add
`ORCHESTER_CLOUD_EXTRA_ORIGINS=http://127.0.0.1:4100` to take forms there too).

## The fake API's states

`tests/fake-api/server.ts` answers by what is typed:

| Input | Answer |
| --- | --- |
| Sign in `mira.koval@example.com` / `correct horse battery` | Signed in: the full Account page (2 invites, 2 servers, 3 devices; a third invite, of kind `account`, is left out as the frames have no row for it) |
| Sign in `new.account@example.com` / `correct horse battery` | A new account: no invites, no servers, one browser |
| Sign in `two.factor@example.com` / `correct horse battery` | Two-factor authentication on: the code step (`/signin/code`) |
| A two-factor code `123456` | Right, whenever one is asked (sign-in, *Turn on*, *Turn off*, new recovery codes, *Verify*) |
| The recovery code `abcde-fghjk` | Right once per account; `000000` and any other code `401 invalid_code`, `999999` `429` |
| *Turn on* with `correct horse battery` | The QR code and key; then `123456` turns it on and shows ten recovery codes |
| `POST /__fake/two-factor-on?email=…` | Turns it on for that account from "another device", so this browser's session is not a two-factor one |
| *Current password* other than `correct horse battery` | `401`: the current password is not right |
| *Delete account* with `correct horse battery` | The account is deleted and signs in no more; any other password `401` |
| Sign in with any other password or address | `401 invalid_credentials` |
| Sign in `unconfirmed@example.com` | `403 email_unconfirmed` |
| Any form with an address starting `limited` | `429`, 900 s for sign-in, 2280 s for emails |
| Any form with an address starting `down` | The cloud fails (`500`/`503`) |
| Any form with an address starting `slow` | Answers after 5 s (the busy buttons) |
| *Resend* for `resend-limit@example.com` | `429` (the resend limit) |
| `/confirm/confirm-ok`, `/reset/reset-ok` | A live link for `mira.koval@example.com` |
| *Confirm email* with `correct horse battery` | Confirmed; any other password `401 confirm_failed` (decision 65) |
| `/confirm/<anything else>`, `/reset/<anything else>` | Used or expired link |
| `/confirm/inspect-down` | The cloud fails while the page loads |

The fake keeps its state while it runs (a declined invite, a signed-out device or a deleted account stays gone);
restart it, or `POST /__fake/reset`, for the accounts as drawn. Each sign-in signs that account's earlier browser sessions out, so the page shows one browser.
