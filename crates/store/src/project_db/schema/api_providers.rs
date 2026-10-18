//! An agent CLI connection may run on an API provider (`docs/design/api-providers.md`): `api_provider` holds its base
//! URL, the id of the project secret with its key and its models as JSON (`{"schema":1,"body":…}`), NULL while the
//! CLI runs on its own sign-in. A database with it is refused by older builds, so none of them runs such a connection
//! on its CLI's own sign-in by not knowing the column. `invocation_providers` records the base URL and key secret an
//! agent CLI invocation started on, so a later invocation knows whether its session was made on the same provider.

pub(super) const MIGRATION: &str = r"
ALTER TABLE connections ADD COLUMN api_provider TEXT;

CREATE TABLE invocation_providers (
  invocation_id TEXT PRIMARY KEY REFERENCES invocations (id),
  base_url TEXT NOT NULL,
  key_secret_id TEXT NOT NULL
) STRICT;
CREATE TRIGGER invocation_providers_immutable BEFORE UPDATE ON invocation_providers
BEGIN SELECT RAISE(ABORT, 'an invocation keeps the API provider it started on'); END;
";
