-- Orchester Cloud C1: accounts, email links, devices, signing keys and key sets, audit
-- (docs/architecture/server.md, "Orchester Cloud"). Times come from the cloud's clock, never from now().

CREATE TABLE accounts (
  id uuid PRIMARY KEY,
  email text NOT NULL CHECK (email = lower(email)),
  display_name text NOT NULL,
  password_hash text NOT NULL,
  confirmed_at timestamptz,
  created_at timestamptz NOT NULL,
  password_changed_at timestamptz NOT NULL
);
CREATE UNIQUE INDEX accounts_email ON accounts (email);

-- One-time confirmation and reset links; only the SHA-256 of a link's token is kept.
CREATE TABLE email_links (
  token_hash bytea PRIMARY KEY CHECK (length(token_hash) = 32),
  account uuid NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
  purpose text NOT NULL CHECK (purpose IN ('confirm', 'reset')),
  created_at timestamptz NOT NULL,
  expires_at timestamptz NOT NULL,
  used_at timestamptz
);
CREATE INDEX email_links_account ON email_links (account);
CREATE INDEX email_links_expiry ON email_links (expires_at);

-- Signed-in desktops and browsers; only the SHA-256 of a device token is kept.
CREATE TABLE devices (
  id uuid PRIMARY KEY,
  account uuid NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
  name text NOT NULL,
  platform text NOT NULL CHECK (platform IN ('windows', 'macos', 'linux', 'web')),
  token_hash bytea NOT NULL UNIQUE CHECK (length(token_hash) = 32),
  created_at timestamptz NOT NULL,
  last_used_at timestamptz NOT NULL,
  revoked_at timestamptz
);
CREATE INDEX devices_account ON devices (account);

-- Identity token signing keys. The private half is sealed with AES-256-GCM under the key-encryption key from the
-- host's environment: a 12-byte nonce, then the ciphertext and tag, with the kid as associated data.
CREATE TABLE signing_keys (
  kid text PRIMARY KEY,
  public_key bytea NOT NULL CHECK (length(public_key) = 32),
  sealed_private_key bytea NOT NULL,
  not_before timestamptz NOT NULL,
  signs_until timestamptz NOT NULL,
  listed_until timestamptz NOT NULL,
  created_at timestamptz NOT NULL,
  CHECK (not_before < signs_until AND signs_until <= listed_until)
);

-- Key sets the offline root key signed, as published; GET /keys serves the newest.
CREATE TABLE key_sets (
  issued_at timestamptz PRIMARY KEY,
  expires_at timestamptz NOT NULL,
  compact text NOT NULL,
  published_at timestamptz NOT NULL
);

-- Who did what, by account and device ids; never an email, a password, a token or a link.
CREATE TABLE audit_events (
  sequence bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  actor uuid,
  action text NOT NULL,
  target text,
  address inet,
  at timestamptz NOT NULL
);
CREATE FUNCTION audit_events_append_only() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  RAISE EXCEPTION 'audit records are append-only';
END
$$;
CREATE TRIGGER audit_events_append_only BEFORE UPDATE OR DELETE OR TRUNCATE ON audit_events
  FOR EACH STATEMENT EXECUTE FUNCTION audit_events_append_only();
