# Orchester Cloud API

The JSON API of `orchester-cloud` (`crates/cloud`): accounts, devices, identity tokens, the team server registry and
invites in flight (C1a).
The design is in [architecture/server.md](architecture/server.md), "Orchester Cloud" and "Identity tokens and keys";
running it is in [cloud-hosting.md](cloud-hosting.md). The wire types are in `orchester-api`'s `cloud` module, and
the token and key set checks team servers use are in its `identity` feature.

Two kinds of client call it:

- **The desktop app.** It signs in with the person's email and password and keeps the device token it gets back in
  the OS keychain (`orchester-cloud` / `device`).
- **The web front** (Next.js, `web/cloud`). It shows sign-up, sign-in, the confirmation landing and forgot/reset
  password pages. Its **server** calls this API; browsers never call it directly, so the API sends no CORS headers.
  It signs in with platform `web` and keeps the token in the `__Host-orchester_session` cookie (`HttpOnly`, `Secure`,
  `SameSite=Lax`), never in script-readable storage. The web front's server must forward the browser's address in `X-Forwarded-For` and be
  listed in `ORCHESTER_CLOUD_TRUSTED_PROXIES`. Otherwise every browser counts as the web front's one address in the
  rate limits.

## Conventions

- Base path `<public URL>/api/v1`, for example `https://orchester-cloud.lurq.dev/api/v1`. JSON bodies in UTF-8.
  Requests are limited to 1 MiB and must arrive within 10 seconds.
- Unknown request fields are refused (`400 bad_request`).
- Endpoints marked *device* need `Authorization: Bearer <device token>`. A device token is `orcc_` followed by 64
  hex characters. It ends when the device signs out or is revoked, and when the password is reset or changed on
  another device. A desktop's token also ends after 90 days without use; a browser's (platform `web`) ends 12
  hours after its sign-in.
- Times are milliseconds since the Unix epoch, UTC, except in identity tokens and key sets, which use JWT seconds.
- Ids are UUIDv7 strings.
- Every response carries `Cache-Control: no-store`.
- A request with a body must say `Content-Type: application/json` (`415 unsupported_media_type` otherwise).
- Errors have the form `{ "error": "<code>", "message": "<English, for logs>" }`. Clients show their own words by
  `error`. A `429` and a `503 busy` carry `Retry-After` in seconds.

| Code | Status | Meaning |
| --- | --- | --- |
| `bad_request` | 400 | Malformed JSON, a wrong field type or an unknown field |
| `unauthorized` | 401 | No device token, or one that is unknown, revoked or expired |
| `invalid_credentials` | 401 | Wrong email or password. One answer for both, and for an email with no account |
| `email_unconfirmed` | 403 | The password was right, but the email is not confirmed yet |
| `not_found` | 404 | No such path, or no such device of the account |
| `method_not_allowed` | 405 | |
| `request_timeout` | 408 | The body did not arrive within 10 seconds |
| `payload_too_large` | 413 | The body is over 1 MiB |
| `unsupported_media_type` | 415 | A body without `Content-Type: application/json` |
| `invalid` | 422 | An input breaks a rule: an email that cannot be one, a password outside 12–256 characters, an empty or over-long name |
| `invalid_link` | 422 | A confirmation or reset link that is unknown, used or expired. One answer for all three |
| `conflict` | 409 | `POST /servers`: another account, or another key, registered that team server id |
| `rate_limited` | 429 | Too many attempts; see Rate limits below |
| `keys_unavailable` | 503 | No published key set lets the cloud sign now: the operator has to publish one |
| `busy` | 503 | No password hashing slot freed within 10 seconds |
| `internal` | 500 | |

## Shapes

```text
DeviceInfo { name: string (1–256), platform: "windows"|"macos"|"linux"|"web" }
Account    { id, email, display_name, created_at }
Device     { id, name, platform, created_at, last_used_at, current: bool }
Server     { id, name, address, fingerprint: string|null }       the SHA-256 of the server's own certificate in hex
AccountServer { ...Server, owner: bool, joined_at }              linked (owner) or first joined
Invite     { id, kind: "server"|"project"|"account", role: "owner"|"member"|null, admin: bool, server: Server,
             project: { address, name }|null, inviter_display_name, expires_at }
```

## Endpoints

| Method and path | Auth | Request | Response | Errors |
| --- | --- | --- | --- | --- |
| `POST /auth/signup` | none | `{ email, password, display_name }` | `202`, empty | `422 invalid`, `429` |
| `POST /auth/confirm` | none | `{ link_token, password }` | `204` | `422 invalid_link`, `429` |
| `POST /auth/confirm/resend` | none | `{ email }` | `202`, empty | `422 invalid`, `429` |
| `POST /auth/links/inspect` | none | `{ link_token, purpose: "confirm"\|"reset" }` | `200 { email }` | `422 invalid_link`, `429` |
| `POST /auth/sessions` | none | `{ email, password, device: DeviceInfo }` | `201 { token, device: Device, account: Account }` | `401 invalid_credentials`, `403 email_unconfirmed`, `422 invalid` (device), `429` |
| `POST /auth/reset` | none | `{ email }` | `202`, empty | `422 invalid`, `429` |
| `POST /auth/reset/finish` | none | `{ link_token, new_password }` | `204` | `422 invalid`, `422 invalid_link`, `429` |
| `POST /auth/password` | device | `{ current_password, new_password }` | `204` | `401 invalid_credentials`, `422 invalid`, `429` |
| `DELETE /auth/sessions/current` | device | | `204` | |
| `GET /me` | device | | `{ account: Account }` | |
| `PATCH /me` | device | `{ display_name? }` | `{ account: Account }` | `422 invalid` |
| `GET /devices` | device | | `{ devices: [Device] }`, oldest first | |
| `DELETE /devices/{id}` | device | | `204` | `404` |
| `GET /keys` | none | | `{ key_set }`: the newest key set, a compact JWS the root key signed | `503 keys_unavailable` |
| `DELETE /me` | device | `{ password }` | `204` | `401 invalid_credentials`, `429` |
| `POST /identity-tokens` | device | `{ audience }`: a registered team server's id | `201 { identity_token, expires_at }` (seconds) | `422 invalid`, `429`, `503 keys_unavailable` |
| `POST /servers` | device | `{ link }`: the server's signed link statement | `201 { server: AccountServer }` | `409 conflict`, `422 invalid` |
| `GET /servers` | device | | `{ servers: [AccountServer] }`, oldest first | |
| `POST /invites` | none (the server's signature) | `{ invite }`: an invite the server signed | `202`, empty | `422 invalid`, `429` |
| `GET /invites` | device | | `{ invites: [Invite] }`: the open invites to the account's email, oldest first | |
| `POST /invites/{id}/decline` | device | | `204` | `404` |
| `POST /invites/{id}/accept` | device | | `201 { server: Server, project, identity_token, expires_at }` | `404`, `429`, `503 keys_unavailable` |

**Sign-up.** `202` means the request was taken. The answer is the same whether or not the email has an account, so
a sign-up form shows one message: *Check your email*. What goes out, in the background:

- a new email gets a confirmation link, valid for 24 hours;
- an email whose account is not confirmed yet gets one more link, and the earlier links stay valid. Each link
  carries the password and display name of the sign-up that sent it. A link is confirmed only with that sign-up's
  password, and confirming it sets that password and name on the account and ends the account's other links. So an
  account's password is always one chosen by a person who also reads the email. Someone who signed up with another
  person's email cannot use the owner's link, and the owner's link cannot be confirmed with their password;
- a confirmed email gets a *you already have an account* note.

An account that is never confirmed is deleted once its last link has expired.

**Confirmation page** (`<public URL>/confirm/<token>`). The page asks for the password chosen at sign-up, and
*Confirm my email* posts it. The confirmation happens only on that form's `POST`, from the web front's server
side, so mail scanners that fetch the link do not use it up. The server side sends
`POST /auth/confirm { link_token, password }`. An unknown, used or expired link and a wrong password give the same
`422 invalid_link`, after the same password check. On `204` the page says the email is confirmed and offers
sign-in. On `invalid_link` it offers to try the password again or to sign up again. After 5 wrong passwords a link
answers `429` for an hour; signing up again sends a new one. Confirming does not sign anyone in.

**Resend.** `POST /auth/confirm/resend { email }` answers `202` for any email, before it touches the database, and
counts against the email limits like a sign-up. In the background an unconfirmed account gets one more confirmation
link. It carries the password and display name of the account's newest open link, so it is confirmed with that
sign-up's password, like any link. A confirmed account gets the *you already have an account* note. An unknown email,
or an account whose links have all expired, gets nothing.

**Inspecting a link.** `POST /auth/links/inspect { link_token, purpose }` gives the email a live link was sent to,
without using the link, so the confirmation and reset pages can show the address before anything is pressed. An
unknown, used or expired link, or one for the other purpose, answers `422 invalid_link` and counts as a failed link
finish against the client's address.

**Sign-in.** A wrong password and an unknown email answer alike (`401 invalid_credentials`). `403 email_unconfirmed`
comes only after the right password. The token in `201` is shown once; the cloud keeps only its SHA-256. `device`
names the computer (the desktop) or the browser (the web front, platform `web`).

**Forgot password.** `POST /auth/reset { email }` answers `202` for any email, before it touches the database. The
lookup, the link (valid for 1 hour) and the email happen in the background, and only an account gets one.

**Reset page** (`<public URL>/reset/<token>`). It asks for the new password and sends
`POST /auth/reset/finish { link_token, new_password }`. A password outside 12–256 characters is refused
(`invalid`) before the link is used, so the person can try again with the same link. On `204`: the password is
set, the email counts as confirmed (the link proves it), every device of the account is signed out, and every
other reset link ends.

**Password change** (signed in). It needs the current password. It signs out the account's other devices and ends
open reset links; the device that asked stays signed in.

**Identity tokens.** `POST /identity-tokens { audience }` returns a token for one team server, valid for 5 minutes.
The desktop hands it to that team server, which checks it with `orchester_api::identity::VerifiedKeySet` against
the key set from `GET /keys`, itself checked with the root key the team server pinned, and accepts each `jti` once.
Claims: `iss` (the public URL), `aud`, `sub` (the account id), `email`, `name`, `device { name, platform }`,
`invite` (only in a token from accepting an invite), `jti`, `iat`, `exp`. `audience` must be a registered team
server (`POST /servers`); the desktop asks only for servers the cloud names.

**Team servers.** A team server signs a **link statement** with its own Ed25519 key (`orchester-link+jws`: its id,
name, address, public key, certificate fingerprint and `issued_at` in seconds; `orchester_api::cloud::LinkStatement`).
Its owner, signed in, sends it as `POST /servers { link }`. The statement must verify with the key it names and be
less than a day old. The cloud registers the server for that account as its owner and keeps the key. Linking again
updates the name, address and fingerprint. The same server id with another key, or by another account while the
owner's account exists, is `409 conflict`. `GET /servers` lists the servers the account owns or joined (by accepting
an invite), each with `owner` and `joined_at` (linked, for the owner; first joined otherwise).

**Invites.** The team server signs each invite by email with its key (`orchester-invite+jws`;
`orchester_api::cloud::InviteStatement`: id, server, kind, email, project `{ address, name }`, role, `admin`,
inviter's display name, `expires_at` and `issued_at` in milliseconds). The server or the inviter's browser posts it
as `POST /invites { invite }`; no token is needed, since the registered server's signature is the proof. The cloud
refuses one from an unregistered server, with another key's signature, already expired, open for more than 7 days,
or not in its kind's shape (`422 invalid`). It keeps the invite until it is accepted, declined or expired, and sends
one email: to the account page when the email has a confirmed account, otherwise to sign up first; the invite then
waits for that account. Posting the same invite again changes nothing and sends nothing. The email carries only the
inviter's display name and the server's and project's names.

`GET /invites` lists the open invites to the account's email. `POST /invites/{id}/decline` deletes one; the team
server keeps its own copy until it expires, and nothing joins with it, since only accepting gives a token that names
the invite. `POST /invites/{id}/accept` ends the invite and answers where to join (`server`, with its address and
fingerprint, and `project`) and a five-minute identity token for that server whose `invite` claim names the invite.
The account then counts as having joined the server (`GET /servers`). Without a published key set the answer is
`503 keys_unavailable` and the invite stays. A team server's revocation of an invite does not reach the cloud yet: the
invite stays listed until it expires, and the team server refuses a join with it.

**Deleting the account.** `DELETE /me { password }` asks for the password again (failures count as a password
change's do). It deletes the account, its devices, links and joins, and the open invites to its email. Servers it
linked stay listed for those who joined them, with no owner, until a new link statement for the server links them
again. The audit keeps the account's id with the actions it took.

## Rate limits

| What | Limit |
| --- | --- |
| Sign-in failures | 5 per email in 15 minutes block that email for 15 minutes; 20 per client address and 100 per IPv6 /48 block that address or network |
| Failed current passwords (`/auth/password`) | 5 per account in 15 minutes |
| Emails (sign-up, resend, reset) | 5 an hour per email, 20 an hour per client address, 100 an hour per IPv6 /48 |
| Failed `confirm`, `reset/finish` and `links/inspect` | 20 per client address and 100 per IPv6 /48 in 15 minutes; for `confirm` and `reset/finish`, 5 an hour per link, each try counted before its password check |
| Identity tokens (and accepted invites) | 60 an hour per device |
| Invites | 100 a day per team server, 30 a day per inviter on a server, 10 a day per invitee email |
| Wrong passwords for `DELETE /me` | Counted with the failed current passwords of `/auth/password` |

The counters are in memory. A restart clears them; the cloud runs as one instance in v1. A client address is its
IPv4 address or its IPv6 /64. Each counter tracks at most 100,000 keys. A new key beyond that evicts the oldest key that holds no count, block or
attempt in flight, rather than being refused, so a flood of new emails cannot lock everyone out. When every key
still counts, the counter grows past 100,000 until those counts expire; it never lifts a block or resets a count
early. The address and network limits bound how fast a flood can add counts.

## Not built yet

- A new email in `PATCH /me` is refused as an unknown field (`400`); the Account page shows *Changing your email is
  not available yet*, as designed.
- Unlinking or renaming a team server at the cloud (`PATCH`/`DELETE /servers/{id}`), and a team server's signed
  revocation of an invite (`DELETE /invites/{id}`).
- Magic link, Google and GitHub sign-in come later.
