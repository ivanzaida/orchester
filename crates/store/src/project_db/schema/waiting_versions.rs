//! A teammate's version that widens what this computer's agents may do (`docs/architecture/server.md`, row 35, "A
//! teammate's version that widens"): cached, but runs keep the version before it until the person's *Use v<N> here*.

/// A cached version with a row here waits: the latest version reads leave it out. *Use v<N> here* removes the rows of
/// v<N> and the versions before it, as does a later version that applies at once.
pub(super) const MIGRATION: &str = r"
CREATE TABLE waiting_versions (
  version_id TEXT PRIMARY KEY REFERENCES shared_version_bodies (version_id)
) STRICT;
";
