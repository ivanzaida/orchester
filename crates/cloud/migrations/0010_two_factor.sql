-- Two-factor authentication (2026-10-16; docs/cloud-api.md, "Two-factor authentication"). Migration 9 is the sign-in
-- gate's; the migrations do not depend on each other, and each is applied when it is not recorded yet.
--
-- An account's authenticator: its TOTP secret sealed with the key-encryption key and bound to the account, the last
-- time step a code was accepted for (a code works once), and when it was confirmed; a row without `confirmed_at` is a
-- setup in progress. Recovery codes are kept as an HMAC under a key derived from the key-encryption key, each usable
-- once. Deleting the row turns two-factor authentication off: the recovery codes go with it, and the devices that
-- signed in with it are no longer two-factor sessions (`devices.two_factor` names the authenticator they used).

CREATE TABLE two_factor (
  id uuid PRIMARY KEY,
  account uuid NOT NULL UNIQUE REFERENCES accounts (id) ON DELETE CASCADE,
  sealed_secret bytea NOT NULL,
  last_step bigint,
  created_at timestamptz NOT NULL,
  confirmed_at timestamptz
);

CREATE TABLE recovery_codes (
  two_factor uuid NOT NULL REFERENCES two_factor (id) ON DELETE CASCADE,
  code_hash bytea NOT NULL CHECK (length(code_hash) = 32),
  used_at timestamptz,
  PRIMARY KEY (two_factor, code_hash)
);

ALTER TABLE devices ADD COLUMN two_factor uuid REFERENCES two_factor (id) ON DELETE SET NULL;

-- The second step of a sign-in: only the SHA-256 of the challenge is kept. It works once, for five minutes and five
-- codes, and only while the password is the one that was checked: `password_check` is the SHA-256 of the account's
-- password hash then, which a new password (a new salt) changes.
CREATE TABLE sign_in_challenges (
  token_hash bytea PRIMARY KEY CHECK (length(token_hash) = 32),
  account uuid NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
  two_factor uuid NOT NULL REFERENCES two_factor (id) ON DELETE CASCADE,
  device_name text NOT NULL,
  platform text NOT NULL CHECK (platform IN ('windows', 'macos', 'linux', 'web')),
  password_check bytea NOT NULL CHECK (length(password_check) = 32),
  attempts integer NOT NULL DEFAULT 0,
  created_at timestamptz NOT NULL,
  expires_at timestamptz NOT NULL,
  used_at timestamptz
);
CREATE INDEX sign_in_challenges_expiry ON sign_in_challenges (expires_at);
