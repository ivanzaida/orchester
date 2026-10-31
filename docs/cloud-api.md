# Orchester Cloud API

The JSON API of `orchester-cloud` (`crates/cloud`): accounts, devices, identity tokens, the team server registry,
invites in flight (C1a, with SA's linking and admin sign-in) and app releases ([App releases](#app-releases)).
The design is in [architecture/server.md](architecture/server.md), "Orchester Cloud" and "Identity tokens and keys";
running it is in `docs/cloud-hosting.md` in the private repository `ivanzaida/orchester-cloud`. The wire types are in `orchester-api`'s `cloud` module, and
the token and key set checks team servers use are in its `identity` feature. The release types are in its `releases`
module, and the check of a release file's signature in its `release-signature` feature.

Two kinds of client call it:

- **The desktop app.** It signs in with the person's email and password and keeps the device token it gets back in
  the OS keychain (`orchester-cloud` / `device`).
- **The web front** (Next.js, `web/cloud`). It shows sign-up, sign-in, the confirmation landing and forgot/reset
  password pages. Its **server** calls this API; browsers never call it directly, so the API sends no CORS headers.
  It signs in with platform `web` and keeps the token in the `__Host-orchester_session` cookie (`HttpOnly`, `Secure`,
  `SameSite=Lax`), never in script-readable storage. The web front's server must forward the browser's address in `X-Forwarded-For` and be
  listed in `ORCHESTER_CLOUD_TRUSTED_PROXIES`. Otherwise every browser counts as the web front's one address in the
  rate limits.

## Base

The cloud has two addresses: its API address, which every API call goes to, and its web address (the public URL),
whose pages the app opens in the browser.

- **API:** `https://api.orchester.cloud/v1`, for example `https://api.orchester.cloud/v1/auth/sessions`. The paths
  below are under it.
- **Web pages** stay on `https://orchester.cloud`: `/signup`, `/signin`, `/account`, `/forgot`, `/invites/…` and the
  links in emails.
- **The old base**, `https://orchester.cloud/api/v1`, is the same service. It answers while app versions from before
  the API moved to its own host are in use, and is switched off after that. The desktop no longer calls it: what an
  earlier version stored with the public cloud (also its former address `https://orchester-cloud.lurq.dev`,
  `docs/cloud-hosting.md` in the private repository `ivanzaida/orchester-platform`, "Moving the cloud") moves to the
  two addresses at the start, without signing out.
- **Identity tokens and key sets** name the public URL, `https://orchester.cloud`, as their issuer, wherever the API
  answers, so team servers keep verifying them as before.
- **A test or self-run cloud** is set in the desktop's Settings › Orchester Cloud › Advanced, with its web address
  and, when its API has an address of its own, its API address (`/v1` there). Without an API address the desktop calls
  `/api/v1` on the web address, so a cloud with no separate API host keeps working. A build names another cloud with
  `ORCHESTER_CLOUD_URL` and, for its API address, `ORCHESTER_CLOUD_API_URL` at build time.
- Team servers keep `<server URL>/api/v1` ([architecture/server.md](architecture/server.md), "API").

## Conventions

- JSON bodies in UTF-8. Requests are limited to 1 MiB and must arrive within 10 seconds.
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
  `error`. A `429` and a `503 busy` carry `Retry-After` in seconds. A `401 unauthorized` for a token the cloud ended
  adds `reason` and `ended_at` ([Why a token ended](#why-a-token-ended)).

| Code | Status | Meaning |
| --- | --- | --- |
| `bad_request` | 400 | Malformed JSON, a wrong field type or an unknown field |
| `unauthorized` | 401 | No device token, or one that is unknown, revoked or expired; one the cloud ended says why ([Why a token ended](#why-a-token-ended)) |
| `invalid_credentials` | 401 | Wrong email or password. One answer for both, and for an email with no account |
| `invalid_code` | 401 | A two-factor code that is wrong or was used, or a recovery code that is unknown or used ([Two-factor authentication](#two-factor-authentication)) |
| `challenge_expired` | 401 | A sign-in's two-factor challenge that is unknown, used, past its 5 minutes or 5 tries, or whose password changed since: sign in again |
| `email_unconfirmed` | 403 | The password was right, but the email is not confirmed yet |
| `mfa_required` | 403 | Accepting an invite (or stepping up) needs two-factor authentication, and the account has it off |
| `session_without_mfa` | 403 | The account has two-factor authentication, but this device signed in without a code: step up with one, or sign in again |
| `forbidden` | 403 | The token is valid but may not do this: a release token without the `releases:write` scope, or a release admin call from an account that is not Orchester staff ([App releases](#app-releases)) |
| `not_found` | 404 | No such path, or no such device of the account |
| `method_not_allowed` | 405 | |
| `request_timeout` | 408 | The body did not arrive within 10 seconds |
| `payload_too_large` | 413 | The body is over 1 MiB (a release file's part: over its `part_size`) |
| `unsupported_media_type` | 415 | A body without `Content-Type: application/json` (a release file's part: without `application/octet-stream`) |
| `range_not_satisfiable` | 416 | A download's `Range` starts at or past the end of the file, or is not one `bytes=<start>-[<end>]` range |
| `invalid` | 422 | An input breaks a rule: an email that cannot be one, a password outside 12–256 characters, an empty or over-long name |
| `invalid_link` | 422 | A confirmation or reset link that is unknown, used or expired. One answer for all three |
| `conflict` | 409 | The team server is linked by a statement as new as this one, or this statement was used; or two-factor authentication is on already (`POST /me/two-factor`); or a release's stage does not allow the change ([App releases](#app-releases)) |
| `too_many_servers` | 409 | The person owns 5 linked team servers, ownerless ones they linked included |
| `rate_limited` | 429 | Too many attempts; see Rate limits below |
| `keys_unavailable` | 503 | No published key set lets the cloud sign now. The cloud renews its key set itself at every start and every minute while it serves (`docs/cloud-hosting.md` in `ivanzaida/orchester-cloud`, "Keys"), so this means it could not, for example its database failed for weeks |
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
| `GET /me/sign-ins` | device | | `{ sign_ins: [{ at, device_name, platform, succeeded, two_factor }] }`: the account's own sign-ins of the last 90 days, newest first, at most 50; `two_factor`: a code was asked for. A wrong password counts only for an account that exists | |
| `GET /keys` | none | | `{ key_set }`: the newest key set, a compact JWS the cloud's root key signed. The cloud makes its root at its first start and publishes a key set before it listens, so a running cloud always has one | `503 keys_unavailable` |
| `POST /identity-tokens` | device | `{ audience, purpose?, invite? }`: the team server's id (a UUID); `purpose: "admin"` for a sign-in to that server's admin, `"code"` to show with a code entered in the app; `invite`: an invite from that server to the account's email, open or accepted by this account, which the token accepts | `201 { identity_token, expires_at }` (seconds) | `403 mfa_required`, `403 session_without_mfa` (an `invite` from a device that is not a two-factor session; the invite stays open), `404 not_found` (purpose `admin` for a server not linked here; an `invite` that is expired, declined, accepted by another account, not to the account's email or not from `audience`), `422 invalid` (also purpose `link`, or `invite` with a purpose), `429`, `503 keys_unavailable` |
| `POST /servers` | device | `{ link }`: the link statement the team server signed | `201 { server, root_key, key_set, identity_token, expires_at }` | `409 conflict` (a replayed or older statement), `409 too_many_servers`, `422 invalid` (also a `server` that is not its `public_key`'s id), `429`, `503 keys_unavailable` |
| `GET /servers` | device | | `{ servers: [{ id, name, address, fingerprint, owner, joined_at }] }` | |
| `GET /servers/{id}` | device | | `{ server: { id, name, address, fingerprint, check_code } }` | `404` (unknown or not linked) |
| `DELETE /servers/{id}` | device | | `204`: the owner unlinks the server. The cloud forgets its link, its members' joins and its invites, so invites in flight are refused; nothing on the server changes, and it links again with a new statement | `403 session_without_mfa` (two-factor authentication on, and this device is not a two-factor session), `404` (not linked, or not this account's), `409 conflict` (an administrator of the cloud blocked it) |
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

**Why a token ended** (the desktop's sign-in gate, 2026-10-16). A `401 unauthorized` for a device's or a web
session's token that the cloud ended carries why and when (milliseconds):
`{ "error": "unauthorized", "message": "…", "reason": "<reason>", "ended_at": 1792147957042 }`.

| `reason` | The token ended because |
| --- | --- |
| `revoked` | another device of the account signed it out (`DELETE /devices/{id}`, also from the account page's Devices) |
| `password_reset` | the password was reset (`/auth/reset/finish`), or changed on another device (`/auth/password`) |
| `account_deleted` | the account was deleted (`DELETE /me`) |
| `signed_out` | the device signed itself out (`DELETE /auth/sessions/current`) |
| `account_disabled` | an administrator of the cloud disabled the account (since 2026-10-27). The cloud ends every token with it, refuses sign-in and issues no identity tokens for team servers until the account is enabled again; a sign-in with the right password answers `401 unauthorized` with this reason too, before any two-factor challenge |

A client that does not know `account_disabled` reads it as no reason, as below. Any other `401` has no `reason`: no token, one the cloud never issued, a desktop's token that ended after 90 days
without use, a browser's after its 12 hours, or one revoked before migration 9. A client reads a reason it does not
know as none and shows its general words. A revoked device keeps its reason in `devices.revoked_reason`. A deleted
account's devices go with it, so the hashes of their tokens are kept in `ended_tokens` (the hash, the reason, when,
and nothing that names the account) until they would have ended unused, 90 days later; the minute's sweep deletes
them then. Telling a token's holder why it ended says nothing they could not see in the app, and an unknown token
learns nothing.

**Forgot password.** `POST /auth/reset { email }` answers `202` for any email, before it touches the database. The
lookup, the link (valid for 1 hour) and the email happen in the background, and only an account gets one.

**Reset page** (`<public URL>/reset/<token>`). It asks for the new password and sends
`POST /auth/reset/finish { link_token, new_password }`. A password outside 12–256 characters is refused
(`invalid`) before the link is used, so the person can try again with the same link. On `204`: the password is
set, the email counts as confirmed (the link proves it), every device of the account is signed out, and every
other reset link ends.

**Password change** (signed in). It needs the current password. It signs out the account's other devices and ends
open reset links; the device that asked stays signed in. With two-factor authentication on, only a two-factor session
changes it, deletes the account (`DELETE /me`) or signs another device out (`DELETE /devices/{id}`); any other device
gets `403 session_without_mfa` before its password is looked at
([Two-factor authentication](#two-factor-authentication)).

**Identity tokens.** `POST /identity-tokens { audience, purpose? }` returns a token for one team server, valid for 5
minutes.
The desktop hands it to that team server, which checks it with `orchester_api::identity::VerifiedKeySet` against
the key set from `GET /keys`, itself checked with the root key the team server pinned, and accepts each `jti` once.
Claims: `iss` (the public URL), `aud`, `sub` (the account id), `email`, `name`, `device { name, platform }`,
`purpose` (absent to join with a device; `admin` for a sign-in to the server admin; `link` for linking the server;
`code` to show with a code), `invite` (when the token accepts an invite), `mfa` (`true` when the device it was issued
to is a two-factor session; absent otherwise, and in every token from a cloud before two-factor authentication),
`jti`, `iat`, `exp`. A team server
accepts a token only for the use it was issued for: a `code` token is taken with a code and nowhere else, so a server
that passes on a token an app got for a code there cannot join or sign in with it anywhere.

**Accepting an invite** (the desktop, N1). The app reads the invite's server from the registry (`GET /servers/{id}`:
the address and the certificate fingerprint it pins), then asks `POST /identity-tokens { audience, invite }`. The
cloud first checks that the device is a two-factor session (`403 mfa_required` while the account has it off,
`403 session_without_mfa` for a device that signed in without a code; either way the invite stays open). It then
checks that the invite is unexpired, to the account's email and from that server, and either open or accepted
by this same account. It marks the invite accepted by the account, issues a join token whose `invite` claim names it,
and records the join. The team server admits the verified email to that invite with the token; without the claim it
admits only an account it already knows.

**Accepting in the browser** (decided 2026-10-31, ORC-576; not built). The web front's *Accept* asks `POST
/identity-tokens { audience, invite, purpose: "accept" }` under the same checks, marks the invite accepted
(`accepted_at`) and records no join until the team server's signed receipt (`orchester-invite-accepted+jws`) comes
back, by `POST /invites/{id}/accepted { receipt }` from the server's web app or the browser's `/invites/joined` form;
until then `GET /invites` keeps listing it with `accepted_at`. The flow, the `orchester://` link and the ended and
unreachable cases are in [server.md](architecture/server.md), "Accepting outside Orchester" (row 77).

The rule for an answered invite (N1 security review 1, M2 and minor 1): the cloud keeps the row with its answer
(`accepted` and by whom, or `declined`) until the invite expires. An answered invite is not in `GET /invites` (once
browser acceptance is built, except one accepted in the browser whose receipt has not come back, above). An
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
start and keeps it, `docs/cloud-hosting.md` in `ivanzaida/orchester-cloud`, "Keys") and the newest key set it signed, and an identity token with purpose `link` that links the admin's account on the server. A token with purpose
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

Added 2026-10-16 with migration 10. An invite to a team server is sent and accepted only by a person with two-factor
authentication on.

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
the authenticator with its recovery codes, and every device stops being a two-factor session. No token ends with it,
so it records no `reason` ([Why a token ended](#why-a-token-ended)): the devices stay signed in, without two-factor. **New recovery codes**
take a code (`POST /me/two-factor/recovery-codes`) and end every earlier one. The account's email gets a notice when
two-factor authentication is turned on or off, when a recovery code is used, and at the tenth wrong code of a day
(purposes `two_factor_on`, `two_factor_off`, `recovery_code_used`, `two_factor_failures`, each with a link to
`<public URL>/account#security`).

**Signing in.** With it on, the right password answers `202 { challenge, expires_at }`, for the desktop and the web
front alike. `challenge` is `orcm_` and 64 hex characters; the cloud keeps only its SHA-256. It works once, for 5 minutes
and 5 codes, and only while the password is the one checked (a change or a reset ends it). `POST /auth/sessions/two-factor
{ challenge, code }` with a code from the app or a recovery code answers what a password-only sign-in answers, with
`recovery_codes_left`. The device is a two-factor session. A wrong code answers `401 invalid_code` and the challenge may
be tried again; an ended one answers `401 challenge_expired`, and the client signs in again with the password.

**Stepping up.** A device that signed in without a code (before two-factor authentication was turned on, or turned on
on another device) becomes a two-factor session with `POST /auth/sessions/current/two-factor { code }`: no password,
no new token. The web front's Account page uses it.

**Code limits.** Every code check of an account counts against two limits, whichever endpoint and challenge it came
to: 5 wrong codes in 15 minutes per account, in memory (`429` with `Retry-After` until the oldest leaves the window),
and 20 wrong codes a day, counted from the first of them and kept in the database, so a restart does not reset it
(`429` with `Retry-After` until that day ends). The day's count is the authenticator's for a sign-in's second step and
for a device that is a two-factor session; a device signed in without a code (before two-factor authentication was
on) counts its own, so its wrong step-ups, or any other code it enters, spend only its own day. Each check reserves
its place in the first before it runs, so parallel guesses cannot pass it, and a right code takes its place back. At
most 20 guesses a day at 3 codes in a million keep a guesser's chance under one in ten thousand a year. The tenth wrong
code of a day mails the account's email (purpose `two_factor_failures`): whoever entered it had the password or a
signed-in device.

**The cap locks out.** Whoever has the password can spend the account's day with wrong codes at the second step of a
sign-in, and then its owner cannot finish a sign-in with a code either, a recovery code included, until that day ends:
the cap trades a day of lockout for a bound on guessing. Devices already signed in stay signed in. A password reset
does not clear the count: a reset proves only the mailbox, and clearing on it would give whoever reads the mail twenty
guesses more each time; the owner changes the password (the intruder had it) and waits out the day, or the operator
clears the count (`docs/cloud-hosting.md` in `ivanzaida/orchester-cloud`, "Two-factor authentication for an account that lost its phone"). These limits are
apart from the password limits: wrong codes do not block passwords, and a right password with two-factor
authentication counts as a successful password check. A challenge also ends after 5 codes, right or wrong. Failed
codes are audited (`two_factor_failed`). A sign-in takes its challenge in the same transaction that spends the code,
before the code is looked at: a second request with the challenge waits for the first, then finds it used and spends
no code of its own.

**What a two-factor session may do.** Only a two-factor session accepts an invite (`POST /identity-tokens` with
`invite`: `403 mfa_required` or `403 session_without_mfa` otherwise, and the invite stays open), and `GET /invites`
says for each whether this device would be refused (`needs_two_factor`). Every identity token says whether its device
is a two-factor session (claim `mfa`). Team servers take an invite's join token only with `mfa`, make invites (by email
or with a code) only from a device token issued for an identity token with it, and take every code but the setup code
from an app only with a token for codes (purpose `code`, the server's id as `aud`) that says `mfa`
([architecture/server.md](architecture/server.md), "Invites", "Codes"). So the desktop asks for a token for the server it
enters a code for, from a two-factor session, and carries the key set with it. It asks only after the server proved
it holds the key whose id it gives itself, on a connection presenting the certificate the proof names (a team
server's id is its public key's; `POST /server/key-proof` on the team server), so a server cannot get a token for
another's id, nor pass on another's proof, unless the two present the same certificate. Invites made before and delivered later are not held to it: delivering
(`POST /invites`) checks the server's signature only. A team server keeps whether a device joined with `mfa` for as
long as that device token lives: turning two-factor authentication off at the cloud does not reach team servers, and
a device keeps sending invites there until an admin signs it out or the person joins it again.

**A lost phone.** The recovery codes are the only way back; two-factor authentication is not reset by email, and a
password reset leaves it on. On a cloud the person runs, an operator can turn it off for an account with one
statement, which deletes the authenticator, its recovery codes and its challenges and ends every two-factor session
with it (`docs/cloud-hosting.md` in `ivanzaida/orchester-cloud`, "Two-factor authentication for an account that lost its phone").

**Someone else turned it on.** Turning it on needs only the password from a signed-in device. Whoever has both while
it is off (a stolen password and an unlocked computer) can turn it on with their own phone and lock the person out of
new sign-ins. The person learns of it from the `two_factor_on` email. Their devices stay signed in, but none of them is
a two-factor session, so none can turn it off, make recovery codes, change the password, delete the account or sign
the intruder's device out. The way back is the operator's: on proof of the account (the mailbox), the statement above
turns it off, after which the person changes the password and signs the other devices out. Orchester Cloud adds no
self-service path for this, since any path that skips the code is one an intruder could take as well.

## App releases

Added 2026-10-30 (ORC-411): how the desktop app learns of and fetches a new version, how CI uploads one, and how
Orchester staff steer it. The types are in `orchester_api::releases`, the calls on `orchester_api::client::CloudClient`.
Three kinds of caller:

- **anonymous**: the update check and downloads need no sign-in, so an app used only locally updates too;
- **`releases:write`**: CI, with `Authorization: Bearer <release token>`, a cloud API token whose scopes include
  `releases:write`. The operator makes it (`docs/cloud-hosting.md` in `ivanzaida/orchester-platform`); it is not a
  device token and signs nobody in. A device token, or a token without the scope, is `403 forbidden`;
- **admin**: the device token of an Orchester staff account (the web front's session) that is a two-factor session.
  An account that is not staff is `403 forbidden`; staff without two-factor authentication `403 mfa_required`, and a
  device that signed in without a code `403 session_without_mfa`.

### Versions, channels and stages

- **Version**: `MAJOR.MINOR.PATCH` with an optional pre-release suffix (`0.5.0-beta.1`), ordered as Semantic Versioning
  2.0.0 orders them. Numbers have no leading zeros and fit in 32 bits, pre-release parts are `[0-9A-Za-z-]+` joined by
  `.`, build metadata (`+…`) is refused, at most 64 characters: every version has one spelling, which is also the one
  in paths and in the signed message.
- **Platforms** `windows`, `macos`; **architectures** `x86_64`, `aarch64` (Rust's `std::env::consts::ARCH`).
- **File names**: 1–128 characters of ASCII letters, digits, `.`, `_`, `+` and `-`, not starting with `.` or `-`, so
  they go into URL paths as they are.
- **Channels** `stable` and `beta`. A Beta install is offered every published release; a Stable install only releases
  staff promoted to Stable, at their rollout percentage. A version with a pre-release suffix is Beta only: it cannot be
  promoted (`422 invalid`).
- **Stages** (`stage` in the admin's shapes): `draft` while CI uploads; `beta` with `paused` once finalized; `stable`
  with `percent` (0–100) and `paused` once promoted (it stays on Beta for every Beta install); `withdrawn`, offered to
  nobody, its files no longer served (`404`). Withdrawing is final; a fixed build is a new version. `paused` is about
  the channel the stage names: a paused `beta` release is offered to no Beta install, a paused `stable` one to no
  Stable install (Beta installs still get it). A cloud before ORC-575 leaves `paused` out of `beta`; read it as
  `false`.

### The signed message

CI signs each file with an Ed25519 release key. The signature is over these exact bytes (`SignedFile::message`):
seven lines of ASCII, each ended by one `\n` (also the last), no other whitespace, the size in decimal without leading
zeros, the SHA-256 of the file's bytes in lowercase hex:

```text
orchester-release-file-v1
version=0.4.0
platform=windows
arch=x86_64
name=Orchester-0.4.0-windows-x86_64.msi
size=48213504
sha256=1055256c88ce265e44b7f378877ffe1c8e85c7670c4fa552f69f2e691a682490
```

So a signature vouches for one file of one version for one platform and architecture: a valid file moved to another
release, name or platform no longer verifies. Signatures travel as 128 lowercase hex characters, SHA-256 digests as
64. The app checks the downloaded file's size and SHA-256 and then the signature with the release keys built into it,
before it runs anything; the cloud checks the same at finalizing, with the same public keys
(`AcceptedReleaseKeys::verify`). Fixed vectors (RFC 8032's first test key) are in `crates/api/tests/releases/`.

**Key ids and rotation.** A release key's id is the first 8 bytes of SHA-256(`"orchester-release-key\0"` ‖ the public
key's 32 bytes), as 16 lowercase hex characters. Every declared and offered file names the `key_id` its signature was
made with. The app holds a short list of accepted public keys (the current one and, while it replaces another, the one
before); a signature naming a key not on the list is refused (`unknown_key`), whatever it says. To rotate: ship an app
version that accepts both keys, then sign with the new key; once installs have moved past the versions that know only
the old key, drop it from the list. The private key lives only in CI's secret store.

### Rollout

An install is in a Stable release's rollout when `bucket < percent`, where `bucket` is the first 8 bytes, big-endian,
of SHA-256(`"orchester-rollout-v1\0"` ‖ the install id as 36 lowercase hyphenated characters ‖ `"\0"` ‖ the version as
written), modulo 100 (`releases::rollout_bucket`). Raising the percentage only adds installs; each version spreads its
buckets anew, so the same installs are not always first. At 0 % nobody gets it, at 100 % everybody.

**The install id** is the app's `InstallationId` (`orchester_core::id`), the UUID of its application database
(`orchester.db`), which already names its secrets in the OS keychain. It names no person or computer. No other
anonymous id existed before this contract: the active-installs count of Launch readiness (ORC-133, in the platform
repository) counts the distinct install ids of update checks, so the two use this one id. A development build and a QA
copy have databases, and so ids, of their own.

### Endpoints

| Method and path | Auth | Request | Response | Errors |
| --- | --- | --- | --- | --- |
| `GET /updates/check?version&platform&arch&channel&install_id` | anonymous | the query, every field required: the app's version, platform, architecture, channel and install id | `200 { update: null }` when nothing newer is offered; `200 { update: { version, channel, notes, critical, published_at, files: [{ name, platform, arch, url, size, sha256, key_id, signature }] } }` | `400 bad_request` (a field missing, unknown or repeated), `422 invalid` (a version, platform, architecture, channel or id that is not one), `429` |
| `GET /releases/{version}/files/{name}` | anonymous | `Range: bytes=<start>-[<end>]` to resume, optional | `200` the whole file, or `206` with `Content-Range: bytes <start>-<end>/<size>`; both with `Content-Type: application/octet-stream`, `Content-Length`, `Accept-Ranges: bytes` and `ETag: "<sha256>"` | `404 not_found` (no such release or file, a draft, a withdrawn release, deleted files), `416 range_not_satisfiable`, `429` |
| `POST /releases` | `releases:write` | `{ version, notes, critical?, ci_run? }`: notes in Markdown, at most 64 KiB; `ci_run` the CI run that uploads it, `{ number, url }` | `201 { release: ReleaseSummary }`, a draft | `403 forbidden`, `409 conflict` (the version exists, in any stage), `422 invalid` (also a `url` that is not `https://…`, over 2048 characters or with spaces or control characters) |
| `POST /releases/{version}/files` | `releases:write` | `{ name, platform, arch, size, sha256, key_id, signature }` | `201 FileUpload` for a new file, `200 FileUpload` for the same declaration again (resume) | `403 forbidden`, `404` (no such draft), `409 conflict` (the release is not a draft, or another declaration under this name, or a second file for the same platform, architecture and name), `422 invalid` (size 0 or over 2 GiB) |
| `PUT /releases/{version}/files/{name}/parts/{part}` | `releases:write` | the part's bytes, `Content-Type: application/octet-stream`: `part_size` bytes, the last part the rest | `200 FileUpload`; a part sent again replaces it | `403 forbidden`, `404` (no such draft or declared file), `409 conflict` (not a draft), `413 payload_too_large`, `415`, `422 invalid` (a part number past `parts`, or a part of the wrong length) |
| `POST /releases/{version}/finalize` | `releases:write` | | `200 { version, files: [{ name, result: "accepted" }` or `{ name, result: "rejected", reason }] }` | `403 forbidden`, `404` (no such release), `409 conflict` (not a draft), `422 invalid` (no file declared) |
| `GET /admin/releases` | admin | | `{ releases: [ReleaseSummary], installs_by_channel: ChannelInstalls \| null }`, newest version first | `403 forbidden`, `403 mfa_required`, `403 session_without_mfa` |
| `GET /admin/releases/{version}` | admin | | `{ release: ReleaseDetails }` | as above, `404` |
| `POST /admin/releases/{version}/promote` | admin | `{ percent }` | `200 { release: ReleaseSummary }`, stage `stable`, not paused | as above, `404`, `409 conflict` (not `beta`, or a paused `beta`: resume it first), `422 invalid` (a pre-release version; a percent over 100) |
| `PUT /admin/releases/{version}/rollout` | admin | `{ percent }` | `200 { release }` | as above, `404`, `409 conflict` (not `stable`), `422 invalid` |
| `POST /admin/releases/{version}/pause` | admin | | `200 { release }`, `paused: true`: a `beta` release stops being offered on Beta, a `stable` one on Stable | as above, `404`, `409 conflict` (a draft or withdrawn release, or paused already) |
| `POST /admin/releases/{version}/resume` | admin | | `200 { release }`, `paused: false`: offered again on the channel it was paused on | as above, `404`, `409 conflict` (a draft or withdrawn release, or not paused) |
| `POST /admin/releases/{version}/withdraw` | admin | | `200 { release }`, stage `withdrawn` | as above, `404`, `409 conflict` (a draft, or withdrawn already) |
| `PATCH /admin/releases/{version}` | admin | `{ notes?, critical? }` | `200 { release }` | as above, `404`, `409 conflict` (withdrawn), `422 invalid` (notes over 64 KiB) |
| `DELETE /admin/releases/{version}/files` | admin | | `204`; the release stays in the list with `files_deleted: true` | as above, `404`, `409 conflict` (the release is still offered: see below) |

Shapes of the admin:

```text
ReleaseSummary  { version, stage: "draft"|"beta"|"stable"|"withdrawn", percent? (with stable),
                  paused? (with beta and stable), critical, created_at, published_at: ms | null, installs,
                  installs_by_channel: ChannelInstalls | null, files_deleted, ci_run: CiRun | null,
                  withdrawn: { at: ms, by: string } | null, uploads: [FileProgress] }
ReleaseDetails  { …ReleaseSummary, notes, files: [AdminFile],
                  adoption: [{ day: "YYYY-MM-DD", installs, installs_by_channel: ChannelInstalls | null }],
                  activity: [{ at, actor: string | null, action, percent? | critical? | channel? }] }
ChannelInstalls { stable, beta }
CiRun           { number, url: "https://…" }
FileProgress    { name, platform, arch, size, sha256, parts, received, state: "receiving"|"verifying"|"verified"
                  | "rejected", reason? (with rejected), received_sha256? (with rejected) }
AdminFile       { name, platform, arch, size, sha256, key_id, downloads }
FileUpload      { name, size, part_size, parts, received: [part numbers, ascending] }
```

`installs` counts the distinct install ids whose newest update check in the last 30 days named this version;
`adoption` counts, for each of the last 30 days (UTC, oldest first), the install ids whose checks that day named it;
`downloads` counts downloads that started at byte 0, so resumed ranges do not count twice. `activity` is newest first;
`action` is one of `created`, `rejected` (a finalize that rejected a file), `published`, `promoted` and `percent_set`
(with `percent`), `paused` and `resumed` (with `channel`, `stable` or `beta`), `withdrawn`, `critical_set` (with
`critical`), `notes_edited`, `files_deleted`; `actor` is the staff member's display name, `null` for CI.

The fields ORC-575 added, each optional for a reader, so a cloud or app on the shapes before it still reads:

- **`installs_by_channel`** splits a count by the channel each install follows, as the update check named it: on a
  release, its `installs`; on an adoption day, that day's `installs`; on the list, every active install (an install id
  with an update check in the last 30 days), whatever version it runs. `stable + beta` is the total. `null` (or absent)
  from a cloud that does not count them apart.
- **`withdrawn`** on a summary: when the release was withdrawn and the display name of the staff member who did it.
  Set with stage `withdrawn`, `null` otherwise.
- **`ci_run`**: the CI run that uploaded the release, as CI named it at `POST /releases`: its number as the CI shows it
  and an `https` link the cloud stores and shows as it is, without following it. `null` when CI named none. The body
  of `POST /releases` takes no unknown field, so a cloud before ORC-575 refuses one with `ci_run`: CI sends it only
  once the cloud reads this contract, and leaves the field out when it has no run.
- **`uploads`**: a draft's declared files and how far each is. `parts` is the file's part count, `received` how many
  of them the cloud has (`received` of `parts`), `sha256` the declared digest. `state` is `receiving` while parts
  arrive and until CI finalizes, `verifying` while finalizing checks it, `verified` once finalizing accepted it, and
  `rejected` with the `reason` finalizing gave (`incomplete`, `sha256_mismatch`, `unknown_key`, `signature_mismatch`)
  and `received_sha256`, the digest of the bytes that arrived (absent with `incomplete`). A rejected file stays in
  `uploads` until CI declares that name again, though its parts and declaration are dropped. Empty once the release
  is published, and from a cloud before ORC-575.
- **`paused` on `beta`**, and `channel` on `paused` and `resumed` activity: absent from a cloud before ORC-575, which
  paused only Stable; read them as `false` and `stable`.

**What the check offers.** Among releases newer than the asked version, for the asked platform and architecture
(a release without a file for them is skipped): on Beta, the newest in stage `beta` that is not paused or in stage
`stable`; on Stable, the newest in stage `stable` that is not paused and whose rollout includes the install.
`critical` is true when the offered release or any release between the asked version and it that this install could
have been offered is marked critical, so skipping a critical release still installs promptly. `files` are the offered
release's files for that platform and architecture; `url` is the absolute URL of its download on the cloud's API
address. An asked version the cloud does not know is still compared by order. Each check counts the install id toward
`installs`, `adoption` and their `installs_by_channel` under the channel it names.

**Uploading a release.** CI creates the draft, then declares each file with its size, SHA-256, key id and signature.
The answer gives the cloud's `part_size` (8 MiB) and the number of parts (`size / part_size`, rounded up). CI sends
each part with `PUT …/parts/{part}`, numbered from 0, part `i` holding bytes `i * part_size` on. These bodies are exempt
from the 1 MiB request limit up to `part_size`, and from the 10 seconds a body must arrive in (5 minutes). An
interrupted upload resumes: declaring the same file again answers `received`, and CI sends only the parts missing.
Then `POST …/finalize` checks every declared file: all its parts arrived (`incomplete` otherwise), the bytes' SHA-256 is
the declared one (`sha256_mismatch`), the key id is one the cloud accepts (`unknown_key`) and that key verifies the
signature over the signed message built from the version in the path and the declaration (`signature_mismatch`). When
every file is accepted the release is published: stage `beta`, `published_at` set. Otherwise it stays a draft, each
rejected file's parts and declaration are dropped, and CI declares and sends those again. A draft not finalized
within 7 days is deleted with its parts.

**Deleting an old release's files** frees their storage. It is allowed for a draft, a withdrawn release, and a release
older than the newest `stable` release that is at 100 % and not paused; any other answers `409 conflict`. The
release's row, counts and activity stay.

## Rate limits

| What | Limit |
| --- | --- |
| Sign-in failures | 5 per email in 15 minutes block that email for 15 minutes; 20 per client address and 100 per IPv6 /48 block that address or network |
| Failed current passwords (`/auth/password`) | 5 per account in 15 minutes |
| Emails (sign-up, resend, reset) | 5 an hour per email, 20 an hour per client address, 100 an hour per IPv6 /48 |
| Failed `confirm`, `reset/finish` and `links/inspect` | 20 per client address and 100 per IPv6 /48 in 15 minutes; for `confirm` and `reset/finish`, 5 an hour per link, each try counted before its password check |
| Wrong passwords for `DELETE /me` | Counted with the failed current passwords of `/auth/password` |
| Identity tokens | 60 an hour per device |
| Two-factor codes (sign-in, step-up, setup, turning off, new recovery codes) | 5 wrong an account in 15 minutes, and 20 wrong a day kept in the database, apart from the password limits; a sign-in's challenge takes 5 codes |
| Invite deliveries (`POST /invites`) | 60 an hour per client address and 300 per IPv6 /48, counted before the signature is checked |
| New invites | 100 a day for all the servers one person owns, ownerless ones they linked included |
| Invite emails | 1 a day per server and invitee address, 3 a day per invitee address; separate from the emails above |
| Linked servers | 5 per person (`409 too_many_servers`), ownerless ones they linked included; not a rate |
| Update checks (`GET /updates/check`) | 12 an hour per install id, 600 an hour per client address and 3,000 per IPv6 /48 |
| Downloads (`GET /releases/…/files/…`) | 60 an hour per client address and 300 per IPv6 /48, resumed ranges included |

The counters are in memory, but for the daily cap on wrong two-factor codes. A restart clears them; the cloud runs as
one instance in v1. A client address is its
IPv4 address or its IPv6 /64. Each counter tracks at most 100,000 keys. A new key beyond that evicts the oldest key that holds no count, block or
attempt in flight, rather than being refused, so a flood of new emails cannot lock everyone out. When every key
still counts, the counter grows past 100,000 until those counts expire; it never lifts a block or resets a count
early. The address and network limits bound how fast a flood can add counts.

## Not built yet

- A new email in `PATCH /me` is refused as an unknown field (`400`); the Account page shows *Changing your email is
  not available yet*, as designed.
- Magic link, Google and GitHub sign-in come later.
- `audience` is not checked against registered team servers for joins; a server not linked here may still be joined
  with a token.
