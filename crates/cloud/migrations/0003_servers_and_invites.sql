-- Team servers and invites in flight (docs/architecture/server.md, "Orchester Cloud", "Invites"). A server is
-- registered from the link statement its owner carries; when the owner deletes their account, the server stays listed
-- for those who joined it, with no owner, and the next valid statement for it links it again.

CREATE TABLE servers (
  id uuid PRIMARY KEY,
  owner uuid REFERENCES accounts (id) ON DELETE SET NULL,
  name text NOT NULL,
  address text NOT NULL,
  -- The SHA-256 of the server's own certificate in hex, which apps pin; NULL for a CA's certificate.
  fingerprint text,
  public_key bytea NOT NULL CHECK (length(public_key) = 32),
  linked_at timestamptz NOT NULL
);
CREATE INDEX servers_owner ON servers (owner);

-- Which accounts joined which server, by accepting an invite, to list them on a new computer.
CREATE TABLE server_accounts (
  server uuid NOT NULL REFERENCES servers (id) ON DELETE CASCADE,
  account uuid NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
  joined_at timestamptz NOT NULL,
  PRIMARY KEY (server, account)
);
CREATE INDEX server_accounts_account ON server_accounts (account);

-- Invites a registered server signed, kept only while open: deleted when accepted, declined or expired.
CREATE TABLE invites (
  id uuid PRIMARY KEY,
  server uuid NOT NULL REFERENCES servers (id) ON DELETE CASCADE,
  email text NOT NULL CHECK (email = lower(email)),
  kind text NOT NULL CHECK (kind IN ('server', 'project', 'account')),
  role text CHECK (role IN ('owner', 'member')),
  admin boolean NOT NULL,
  project_name text,
  project_address text,
  inviter_display_name text NOT NULL,
  received_at timestamptz NOT NULL,
  expires_at timestamptz NOT NULL,
  CHECK ((project_name IS NULL) = (project_address IS NULL))
);
CREATE INDEX invites_email ON invites (email);
CREATE INDEX invites_expiry ON invites (expires_at);
