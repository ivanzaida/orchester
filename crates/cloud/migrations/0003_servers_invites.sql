-- Team servers linked to the cloud, which accounts use them, and invites in flight (docs/architecture/server.md,
-- "Orchester Cloud", "Invites"). A server is registered from the link statement it signed with its own key; an invite
-- is kept only until it expires or the server revokes it.

CREATE TABLE servers (
  id uuid PRIMARY KEY,
  owner uuid NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
  name text NOT NULL,
  address text NOT NULL,
  fingerprint text CHECK (fingerprint ~ '^[0-9a-f]{64}$'),
  public_key text NOT NULL,
  linked_at timestamptz NOT NULL,
  unlinked_at timestamptz
);
CREATE INDEX servers_owner ON servers (owner);

CREATE TABLE server_accounts (
  server uuid NOT NULL REFERENCES servers (id) ON DELETE CASCADE,
  account uuid NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
  first_joined timestamptz NOT NULL,
  PRIMARY KEY (server, account)
);
CREATE INDEX server_accounts_account ON server_accounts (account);

CREATE TABLE invites (
  id uuid PRIMARY KEY,
  server uuid NOT NULL REFERENCES servers (id) ON DELETE CASCADE,
  kind text NOT NULL CHECK (kind IN ('server', 'project', 'account')),
  email text NOT NULL CHECK (email = lower(email)),
  project_address text,
  project_name text,
  role text CHECK (role IN ('owner', 'member')),
  inviter_display_name text NOT NULL,
  expires_at timestamptz NOT NULL,
  received_at timestamptz NOT NULL
);
CREATE INDEX invites_email ON invites (email);
CREATE INDEX invites_expiry ON invites (expires_at);
CREATE INDEX invites_server ON invites (server, received_at);
