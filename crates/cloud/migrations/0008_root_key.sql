-- The cloud's root key, made at its first start: it signs the key sets team servers check identity tokens with, and
-- team servers pin its public half when they link. Its private half is sealed with the key-encryption key, as the
-- signing keys' are. The table holds one row at most, so replicas that start together cannot store two roots.

CREATE TABLE root_key (
  only_row boolean PRIMARY KEY DEFAULT true CHECK (only_row),
  public_key bytea NOT NULL CHECK (length(public_key) = 32),
  sealed_private_key bytea NOT NULL,
  created_at timestamptz NOT NULL
);
