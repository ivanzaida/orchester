# Orchester Cloud API

The JSON API of `orchester-cloud` (`crates/cloud`): accounts, devices, identity tokens, the team server registry and
invites in flight (C1a, with SA's linking and admin sign-in).
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
| `invalid_code` | 401 | A two-factor code that is wrong or was used, or a recovery code that is unknown or used ([Two-factor authentication](#two-factor-authentication)) |
| `challenge_expired` | 401 | A sign-in's two-factor challenge that is unknown, used, past its 5 minutes or 5 tries, or whose password changed since: sign in again |
| `email_unconfirmed` | 403 | The password was right, but the email is not confirmed yet |
| `mfa_required` | 403 | Accepting an invite (or stepping up) needs two-factor authentication, and the account has it off |
| `session_without_mfa` | 403 | The account has two-factor authentication, but this device signed in without a code: step up with one, or sign in again |
| `not_found` | 404 | No such path, or no such device of the account |
| `method_not_allowed` | 405 | |
| `request_timeout` | 408 | The body did not arrive within 10 seconds |
| `payload_too_large` | 413 | The body is over 1 MiB |
| `unsupported_media_type` | 415 | A body without `Content-Type: application/json` |
| `invalid` | 422 | An input breaks a rule: an email that cannot be one, a password outside 12–256 characters, an empty or over-long name |
| `invalid_link` | 422 | A confirmation or reset link that is unknown, used or expired. One answer for all three |
| `conflict` | 409 | The team server is linked by a statement as new as this one, or this statement was used; or two-factor authentication is on already (`POST /me/two-factor`) |
| `too_many_servers` | 409 | The person owns 5 linked team servers, ownerless ones they linked included |
| `rate_limited` | 429 | Too many attempts; see Rate limits below |
| `keys_unavailable` | 503 | No published key set lets the cloud sign now. The cloud renews its key set itself at every start and every minute while it serves (`docs/cloud-hosting.md`, "Keys"), so this means it could not, for example its database failed for weeks |
| `busy` | 503 | No password hashing slot freed within 10 seconds |
| `internal` | 500 | |

## Shapes

```text
DeviceInfo { name: string (1–256), platform: "windows"|"macos"|"linux"|"web" }
Account    { id, email, display_name, created_at }
Device     { id, name, platform, created_at, last_used_at, current: bool }
TwoFactor  { enabled: bool, enabled_at: ms | null, recovery_codes_left: 0–10, this_session: bool }
```

`this_session` says whether the asking device is a *two-factor session*: it signed in with a code, stepped up with one,
or turned two-factor authentication on itself.

## Endpoints

| Method and path | Auth | Request | Response | Errors |
| --- | --- | --- | --- | --- |
| `POST /auth/signup` | none | `{ email, password, display_name }` | `202`, empty | `422 invalid`, `429` |
| `POST /auth/confirm` | none | `{ link_token, password }` | `204` | `422 invalid_link`, `429` |
| `POST /auth/confirm/resend` | none | `{ email }` | `202`, empty | `422 invalid`, `429` |
| `POST /auth/links/inspect` | none | `{ link_token, purpose: "confirm"\|"reset" }` | `200 { email }` | `422 invalid_link`, `429` |
| `POST /auth/sessions` | none | `{ email, password, device: DeviceInfo }` | `201 { token, device: Device, account: Account }`; with two-factor authentication on, `202 { challenge, expires_at }` | `401 invalid_credentials`, `403 email_unconfirmed`, `422 invalid` (device), `429` |
| `POST /auth/sessions/two-factor` | none | `{ challenge, code }`: a code from the app or a recovery code | `201 { token, device, account, recovery_codes_left }` | `401 invalid_code`, `401 challenge_expired`, `429` |
| `POST /auth/sessions/current/two-factor` | device | `{ code }` | `200 { recovery_codes_left }`: this device is a two-factor session now, with the same token | `401 invalid_code`, `403 mfa_required`, `429` |
| `POST /auth/reset` | none | `{ email }` | `202`, empty | `422 invalid`, `429` |
| `POST /auth/reset/finish` | none | `{ link_token, new_password }` | `204` | `422 invalid`, `422 invalid_link`, `429` |
| `POST /auth/password` | device | `{ current_password, new_password }` | `204` | `401 invalid_credentials`, `422 invalid`, `429` |
| `DELETE /auth/sessions/current` | device | | `204` | |
| `GET /me` | device | | `{ account: Account, two_factor: TwoFactor }` | |
| `PATCH /me` | device | `{ display_name? }` | `{ account: Account, two_factor: TwoFactor }` | `422 invalid` |
| `POST /me/two-factor` | device | `{ password }` | `201 { secret, uri }`: a new setup, replacing one not confirmed | `401 invalid_credentials`, `409 conflict` (on already), `429` |
| `POST /me/two-factor/confirm` | device | `{ code }`: a code from the app | `200 { recovery_codes: [10] }`, shown once | `401 invalid_code`, `404` (no setup in its 15 minutes), `429` |
| `DELETE /me/two-factor` | device | `{ password, code }`: a code from the app or a recovery code | `204` | `401 invalid_credentials`, `401 invalid_code`, `404` (off), `429` |
| `POST /me/two-factor/recovery-codes` | device | `{ code }`: either kind | `200 { recovery_codes: [10] }`, shown once; every earlier one ends | `401 invalid_code`, `404` (off), `429` |
| `DELETE /me` | device | `{ password }` | `204` | `401 invalid_credentials`, `429` |
| `GET /devices` | device | | `{ devices: [Device] }`, oldest first | |
| `DELETE /devices/{id}` | device | | `204` | `404` |
| `GET /keys` | none | | `{ key_set }`: the newest key set, a compact JWS the cloud's root key signed. The cloud makes its root at its first start and publishes a key set before it listens, so a running cloud always has one | `503 keys_unavailable` |
| `POST /identity-tokens` | device | `{ audience, purpose?, invite? }`: the team server's id (a UUID); `purpose: "admin"` for a sign-in to that server's admin; `invite`: an invite from that server to the account's email, open or accepted by this account, which the token accepts | `201 { identity_token, expires_at }` (seconds) | `403 mfa_required`, `403 session_without_mfa` (an `invite` from a device that is not a two-factor session; the invite stays open), `404 not_found` (purpose `admin` for a server not linked here; an `invite` that is expired, declined, accepted by another account, not to the account's email or not from `audience`), `422 invalid` (also purpose `link`, or `invite` with a purpose), `429`, `503 keys_unavailable` |
| `POST /servers` | device | `{ link }`: the link statement the team server signed | `201 { server, root_key, key_set, identity_token, expires_at }` | `409 conflict` (a replayed or older statement), `409 too_many_servers`, `422 invalid` (also a `server` that is not its `public_key`'s id), `429`, `503 keys_unavailable` |
| `GET /servers` | device | | `{ servers: [{ id, name, address, fingerprint, owner, joined_at }] }` | |
| `GET /servers/{id}` | device | | `{ server: { id, name, address, fingerprint, check_code } }` | `404` (unknown or not linked) |
| `POST /invites` | the server's signature | `{ invite }`: an invite the team server signed | `202 { server: { id, name, address } }`: the inviting server as the cloud registered it | `422 invalid` (also an invite that was revoked, until it would have expired), `429` |
| `GET /invites` | device | | `{ invites: [{ id, kind, role, server: { id, name, address }, project: { address, name } \| null, inviter_display_name, expires_at, needs_two_factor }] }`; `needs_two_factor`: this device could not accept it now (it is not a two-factor session) | |
| `DELETE /invites/{id}` | the server's signature | `{ revocation }`: the team server's signed revocation | `204`; the id stays revoked until the invite would have expired | `422 invalid` |
| `POST /invites/{id}/decline` | device | | `204`; the invite stays declined until it expires: no token accepts it, and delivering it again changes nothing | `404` (also one already answered) |

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
unknown, used or expired link, or one for the other purpose, answers `422 invalid_link` after the same one query, and
counts as a failed link finish against the client's address.

**Sign-in.** A wrong password and an unknown email answer alike (`401 invalid_credentials`). `403 email_unconfirmed`
comes only after the right password. The token in `201` is shown once; the cloud keeps only its SHA-256. `device`
names the computer (the desktop) or the browser (the web front, platform `web`). An account with two-factor
authentication answers the right password with `202 { challenge, expires_at }` instead, and a code finishes the sign-in
([Two-factor authentication](#two-factor-authentication)).

**Forgot password.** `POST /auth/reset { email }` answers `202` for any email, before it touches the database. The
lookup, the link (valid for 1 hour) and the email happen in the background, and only an account gets one.

**Reset page** (`<public URL>/reset/<token>`). It asks for the new password and sends
`POST /auth/reset/finish { link_token, new_password }`. A password outside 12–256 characters is refused
(`invalid`) before the link is used, so the person can try again with the same link. On `204`: the password is
set, the email counts as confirmed (the link proves it), every device of the account is signed out, and every
other reset link ends.

**Password change** (signed in). It needs the current password. It signs out the account's other devices and ends
open reset links; the device that asked stays signed in.

**Identity tokens.** `POST /identity-tokens { audience, purpose? }` returns a token for one team server, valid for 5
minutes.
The desktop hands it to that team server, which checks it with `orchester_api::identity::VerifiedKeySet` against
the key set from `GET /keys`, itself checked with the root key the team server pinned, and accepts each `jti` once.
Claims: `iss` (the public URL), `aud`, `sub` (the account id), `email`, `name`, `device { name, platform }`,
`purpose` (absent to join with a device; `admin` for a sign-in to the server admin; `link` for linking the server),
`invite` (when the token accepts an invite), `mfa` (`true` when the device it was issued to is a two-factor session;
absent otherwise, and in every token from a cloud before two-factor authentication), `jti`, `iat`, `exp`. A team server
accepts a token only for the use it was issued for.

**Accepting an invite** (the desktop, N1). The app reads the invite's server from the registry (`GET /servers/{id}`:
the address and the certificate fingerprint it pins), then asks `POST /identity-tokens { audience, invite }`. The
cloud first checks that the device is a two-factor session (`403 mfa_required` while the account has it off,
`403 session_without_mfa` for a device that signed in without a code; either way the invite stays open). It then
checks that the invite is unexpired, to the account's email and from that server, and either open or accepted
by this same account. It marks the invite accepted by the account, issues a join token whose `invite` claim names it,
and records the join. The team server admits the verified email to that invite with the token; without the claim it
admits only an account it already knows.

The rule for an answered invite (N1 security review 1, M2 and minor 1): the cloud keeps the row with its answer
(`accepted` and by whom, or `declined`) until the invite expires. An answered invite is not in `GET /invites`. An
accepted one still issues tokens that accept it, to the account that accepted it only, until it expires or the server
revokes it: a join that failed after its token (the server unreachable, a token lost) is simply asked again, and the
team server, which accepts each token once and ends its own invite at the first join, admits it once. A declined one
issues none, and cannot then be accepted; an accepted one cannot then be declined. Because the row stays, delivering
the same signed invite again changes nothing. The cloud has no word from the server that the join happened; the row
goes with the invite's expiry (the minute's sweep) or a revocation.

**Linking a team server** (`POST /servers { link }`, from the cloud's `/authorize` page, purpose `link`). The link
statement is a compact JWS of type `orchester-link+jws` that the server signed with its own Ed25519 key:
`{ server, nonce, name, address, public_key, fingerprint, issued_at }` (`orchester_api::statements::LinkStatement`).

- **A server's id is its key's** (security review 1, B1): `server` must be
  `orchester_api::identity::PublicKey::server_id(public_key)`, a version-8 UUID of the first 16 bytes of
  SHA-256(`"orchester-team-server-id\0"` ‖ the 32 key bytes). A team server makes its id that way and, at every
  start, replaces a stored id that is not its key's with its key's, so nobody can register another server's id with a
  key of their own, and every identity token the cloud issues for an id (the token's `aud`) is for the holder of that
  key. Migration 4 deletes the servers linked under migration 3 whose id is not their key's (with their accounts and
  invites, logged); such a server links again under its own id (security review 2).
- The cloud verifies the statement with the `public_key` inside, refuses one issued more than 10 minutes ago, and
  uses each `nonce` once: a replay is `409 conflict`.
- **The newest statement owns the server.** A statement newer than the one that linked it takes the server over for
  the signed-in account (its admins moved it); an older or equally old one is `409 conflict`.
- One person owns at most 5 linked servers (`409 too_many_servers`): the account's, and those the same person linked
  from an account they deleted (see *Deleting the account*). Linking one it owns already does not count.

The answer carries what the server pins: the cloud's root key (`root_key`, base64url; the cloud makes it at its first
start and keeps it, `docs/cloud-hosting.md`, "Keys") and the newest key set it signed, and an identity token with purpose `link` that links the admin's account on the server. A token with purpose
`admin` is issued only for a linked server.

**The web front's `/authorize`** takes a team server admin's request only in a form's body, `POST /authorize/start`
(`purpose`, `server` or `statement`, `state`, `return`), keeps it sealed in this browser's `__Host-orchester_authorize`
cookie for 10 minutes and shows `/authorize?return=<callback>`, whose address names only the callback: the statement
never travels in a URL, a history or a log. Confirming uses the request up and binds it to the account signed in then.
`/authorize/start` keeps a request only when the post's `Origin` is the callback's origin (the admin that asked);
anything else leaves this browser's request as it was. A browser sends `Origin: null` when an https page posts to plain
http; a cloud on plain http (loopback only, a local or test setup) therefore takes a null origin with an https
callback, and an https cloud never does. The confirm form carries a digest of the request the page
showed, and confirming refuses with *This page is out of date, open the link again* when the cookie holds another
one by then, so a request another site posts after the page loaded is never the one confirmed (security review 2).

**Invites in flight.** A team server signs each invite by email (`orchester-invite+jws`:
`{ id, server, kind, email, project?, role?, inviter, expires_at, issued_at }`, times in milliseconds) and its server
admin delivers it, from the host or from the inviter's browser through the web front's `/invites/deliver`. The cloud
checks the signature with the key the server linked with, keeps the invite once (a second delivery changes nothing),
and emails the invitee in its own words only (nothing the server chose: an account gets *open Orchester*, an address
with no account `/signup?invite=<id>`). The answer names the inviting server as registered (`202 { server }`).
`GET /invites` lists them to the account with that email; a revocation (`orchester-invite-revocation+jws`,
`{ id, server, issued_at }`) deletes one and keeps its id and server in `revoked_invites` until the invite would have expired
(at most 8 days), so the same signed invite cannot be delivered again (security review 1, minor 1); expired invites
and ended tombstones are deleted by the minute's sweep.

**Invite limits** (security review 1, M2): deliveries are counted per client address before the signature is
checked; the servers one person owns keep at most 100 new invites a day between them, those linked from an account the
person deleted included, and revoking gives none back;
an invite's email has buckets of its own, apart from confirmation and reset emails: one a day per server and address
and three a day per address. An invite over an email limit is still kept and listed in the invitee's app, without an
email. Numbers in [Rate limits](#rate-limits).

**The web front's `/invites/deliver`** takes the signed invite in a form's body only, delivers it, and keeps the
answer in a signed cookie for `/invites/delivered`, which leads back only to the admin at the address the cloud
registered for that server (`<address>/admin`): nothing in the post or the page's address names where to go
(security review 1, minor 2).

**Declining an invite.** `POST /invites/{id}/decline` answers an open invite to the account's email with no (`404`
for any other, also one already answered). It leaves the list and issues no token. The row stays, as declined, until
the invite expires, so delivering it again (from the server or the inviter's browser) changes nothing and sends no
email. `revoked_invites` holds only the team server's revocations. The team server keeps its own copy until it
expires or is revoked; nothing joins with it, since a join needs an identity token that names it.

**Deleting the account.** `DELETE /me { password }` asks for the password again (failures count as a password
change's do). It deletes the account, its devices, links and joins, and the invites to its email, answered or not. The team
servers it linked stay registered with no owner, with their members' joins and invites (migration 5), until an admin of
the server links it again: the newest statement the server's key signs owns it. Meanwhile the server keeps the
person who linked it as `last_owner` (migration 7, N1 security re-review 2), and counts toward that person's caps: the
5 linked servers and the 100 invites a day. A re-signup gets a new account id, so the person is keyed by the account's
email, which the cloud does not keep: `last_owner` is an HMAC-SHA256 of the normalized email under a key derived (HKDF,
info `orchester-cloud people v1`) from the key-encryption key, so the database alone can neither name the email nor
check one against it, and signing up again with the same email gives no caps back. A new link statement clears it with
the new owner. A server ownerless since before migration 7 has no `last_owner` and counts its invites on its own. So
after a deletion the cloud still holds, on each server the account linked, that keyed hash of its email, until the
server is linked again; the web's delete confirmation says so when the account linked servers ("A keyed hash of your
email stays with the team servers you linked, for abuse limits, until they are linked again."). The audit keeps the
account's id with the actions it took.

## Two-factor authentication

Added 2026-10-16 with migration 10 (the sign-in gate's migration 9 is applied apart; a migration is applied when its
number is not recorded yet, whatever newer ones the database has). An invite to a team server is sent and accepted only
by a person with two-factor authentication on.

**Method.** TOTP (RFC 6238): HMAC-SHA-1, 6 digits, a 30-second step, accepted for the step before, the current one and
the one after. A code works once: the cloud keeps the last step it accepted, and refuses that step and every earlier
one (`401 invalid_code`). The secret is 20 random bytes, shown in base32. It is sealed with
`ORCHESTER_CLOUD_KEY_ENCRYPTION_KEY` (AES-256-GCM, as the root and signing keys are) with the account's id as associated
data, so it opens only for its account. The secret and the recovery codes appear only in the answers that make them,
never in a log, an error, `Debug` output or the audit.

**Recovery codes.** Ten, each `xxxxx-xxxxx`: ten characters of Crockford's base32 in lower case (`0-9a-z` without `i`,
`l`, `o`, `u`; 50 random bits) with a dash in the middle. Each works once in place of a code from the app, anywhere a
code is asked. Case, spaces and dashes do not matter, and `o`, `i` and `l` read as `0`, `1` and `1`. The cloud keeps
only an HMAC-SHA256 of each under a key derived from the key-encryption key (HKDF, info
`orchester-cloud recovery codes v1`). Signing in or stepping up with one answers `recovery_codes_left`, so a client can
warn when few remain; `GET /me` says it too.

**Turning it on.** `POST /me/two-factor { password }` answers the secret and an
`otpauth://totp/<host>:<email>?secret=…&issuer=<host>&algorithm=SHA1&digits=6&period=30` link for a QR code, where
`<host>` is the host of `ORCHESTER_CLOUD_PUBLIC_URL`. A setup not confirmed within 15 minutes ends. A code from the app
confirms it: `POST /me/two-factor/confirm { code }` turns it on, answers the recovery codes once, and makes the device
that confirmed it a two-factor session. Other devices stay signed in as they were, without it.

**Turning it off** takes the password and a code from the app or a recovery code (`DELETE /me/two-factor`). It deletes
the authenticator with its recovery codes, and every device stops being a two-factor session. **New recovery codes**
take a code (`POST /me/two-factor/recovery-codes`) and end every earlier one. The account's email gets a notice when
two-factor authentication is turned on or off and when a recovery code is used (purposes `two_factor_on`,
`two_factor_off`, `recovery_code_used`, each with a link to `<public URL>/account#security`).

**Signing in.** With it on, the right password answers `202 { challenge, expires_at }`, for the desktop and the web
front alike. `challenge` is `orcm_` and 64 hex characters; the cloud keeps only its SHA-256. It works once, for 5 minutes
and 5 codes, and only while the password is the one checked (a change or a reset ends it). `POST /auth/sessions/two-factor
{ challenge, code }` with a code from the app or a recovery code answers what a password-only sign-in answers, with
`recovery_codes_left`. The device is a two-factor session. A wrong code answers `401 invalid_code` and the challenge may
be tried again; an ended one answers `401 challenge_expired`, and the client signs in again with the password.

**Stepping up.** A device that signed in without a code (before two-factor authentication was turned on, or turned on
on another device) becomes a two-factor session with `POST /auth/sessions/current/two-factor { code }`: no password,
no new token. The web front's Account page uses it.

**Code limits.** Every code check of an account counts against one limit: 5 wrong codes in 15 minutes per account,
whichever endpoint and challenge they came to (`429` with `Retry-After` until the oldest leaves the window). Each check
reserves its place before it runs, so parallel guesses cannot pass it, and a right code takes its place back. This
limit is apart from the password limits: wrong codes do not block passwords, and a right password with two-factor
authentication counts as a successful password check. A challenge also ends after 5 codes, right or wrong. Failed
codes are audited (`two_factor_failed`).

**What a two-factor session may do.** Only a two-factor session accepts an invite (`POST /identity-tokens` with
`invite`: `403 mfa_required` or `403 session_without_mfa` otherwise, and the invite stays open), and `GET /invites`
says for each whether this device would be refused (`needs_two_factor`). Every identity token says whether its device
is a two-factor session (claim `mfa`). Team servers take an invite's join token only with `mfa`, and send invites by email
only from a device token issued for an identity token with it ([architecture/server.md](architecture/server.md),
"Invites"). Invites made before and delivered later are not held to it: delivering (`POST /invites`) checks the
server's signature only.

**A lost phone.** The recovery codes are the only way back; two-factor authentication is not reset by email, and a
password reset leaves it on. On a cloud the person runs, an operator can turn it off for an account with one
statement, which deletes the authenticator, its recovery codes and its challenges and ends every two-factor session
with it (`docs/cloud-hosting.md`, "Two-factor authentication for an account that lost its phone").

## Rate limits

| What | Limit |
| --- | --- |
| Sign-in failures | 5 per email in 15 minutes block that email for 15 minutes; 20 per client address and 100 per IPv6 /48 block that address or network |
| Failed current passwords (`/auth/password`) | 5 per account in 15 minutes |
| Emails (sign-up, resend, reset) | 5 an hour per email, 20 an hour per client address, 100 an hour per IPv6 /48 |
| Failed `confirm`, `reset/finish` and `links/inspect` | 20 per client address and 100 per IPv6 /48 in 15 minutes; for `confirm` and `reset/finish`, 5 an hour per link, each try counted before its password check |
| Wrong passwords for `DELETE /me` | Counted with the failed current passwords of `/auth/password` |
| Identity tokens | 60 an hour per device |
| Two-factor codes (sign-in, step-up, setup, turning off, new recovery codes) | 5 wrong an account in 15 minutes, apart from the password limits; a sign-in's challenge takes 5 codes |
| Invite deliveries (`POST /invites`) | 60 an hour per client address and 300 per IPv6 /48, counted before the signature is checked |
| New invites | 100 a day for all the servers one person owns, ownerless ones they linked included |
| Invite emails | 1 a day per server and invitee address, 3 a day per invitee address; separate from the emails above |
| Linked servers | 5 per person (`409 too_many_servers`), ownerless ones they linked included; not a rate |

The counters are in memory. A restart clears them; the cloud runs as one instance in v1. A client address is its
IPv4 address or its IPv6 /64. Each counter tracks at most 100,000 keys. A new key beyond that evicts the oldest key that holds no count, block or
attempt in flight, rather than being refused, so a flood of new emails cannot lock everyone out. When every key
still counts, the counter grows past 100,000 until those counts expire; it never lifts a block or resets a count
early. The address and network limits bound how fast a flood can add counts.

## Not built yet

- Unlinking a server from the cloud's side.
- A new email in `PATCH /me` is refused as an unknown field (`400`); the Account page shows *Changing your email is
  not available yet*, as designed.
- Magic link, Google and GitHub sign-in come later.
- `audience` is not checked against registered team servers for joins; a server not linked here may still be joined
  with a token.
