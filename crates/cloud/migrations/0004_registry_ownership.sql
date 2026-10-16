-- Security review 1 (docs/architecture/server.md, "Linking a team server at setup", "Invites"): a server's id is its
-- own key's, and the newest statement that key signed decides who owns it; each statement works once; a revoked
-- invite stays revoked until it would have expired. Servers linked under migration 3 whose id is not their key's are
-- deleted when this migration is applied (`db::migrate`, security review 2).

ALTER TABLE servers ADD COLUMN statement_issued_at timestamptz;

CREATE TABLE used_link_statements (
  nonce text PRIMARY KEY,
  expires_at timestamptz NOT NULL
);
CREATE INDEX used_link_statements_expiry ON used_link_statements (expires_at);

CREATE TABLE revoked_invites (
  id uuid NOT NULL,
  server uuid NOT NULL,
  until timestamptz NOT NULL,
  PRIMARY KEY (id, server)
);
CREATE INDEX revoked_invites_until ON revoked_invites (until);
