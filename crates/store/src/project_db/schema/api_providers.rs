//! An agent CLI connection may run on an API provider (`docs/design/api-providers.md`): `api_provider` holds its base
//! URL, the id of the project secret with its key and its models as JSON (`{"schema":1,"body":…}`), NULL while the
//! CLI runs on its own sign-in. A database with it is refused by older builds, so none of them runs such a connection
//! on its CLI's own sign-in by not knowing the column.

pub(super) const MIGRATION: &str = r"
ALTER TABLE connections ADD COLUMN api_provider TEXT;
";
