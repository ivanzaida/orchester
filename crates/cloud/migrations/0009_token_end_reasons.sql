-- The sign-in gate (2026-10-16): a `401` for a device's or a web session's token that the cloud ended says why
-- (`docs/cloud-api.md`, "Why a token ended"). A revoked device keeps how it ended; the tokens of a deleted account,
-- whose devices go with it, are kept by their hash alone until they would have expired without use (90 days), with
-- nothing that names the account.

ALTER TABLE devices ADD COLUMN revoked_reason text CHECK (revoked_reason IN ('revoked', 'password_reset', 'signed_out'));
CREATE TABLE ended_tokens (
  token_hash bytea PRIMARY KEY CHECK (length(token_hash) = 32),
  reason text NOT NULL CHECK (reason IN ('account_deleted')),
  ended_at timestamptz NOT NULL,
  until timestamptz NOT NULL
);
