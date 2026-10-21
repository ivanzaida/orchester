//! Files a first commit would add that look as if they hold secrets (`init_repository`'s card warns of them, and the
//! person decides): environment files, private keys and certificates' key stores, and the credential files of common
//! tools. Judged by a file's name only; its contents are never read.

/// The environment files' endings that hold examples, not values.
const EXAMPLE_ENDINGS: [&str; 5] = [".example", ".sample", ".template", ".dist", ".defaults"];
/// Endings of key and key store files.
const KEY_ENDINGS: [&str; 8] = [".pem", ".key", ".p12", ".pfx", ".jks", ".keystore", ".ppk", ".kdbx"];
/// Names of private keys and of tools' credential files.
const NAMES: [&str; 12] = [
  "id_rsa",
  "id_dsa",
  "id_ecdsa",
  "id_ed25519",
  ".npmrc",
  ".pypirc",
  ".netrc",
  "_netrc",
  ".git-credentials",
  ".htpasswd",
  "credentials.json",
  "secrets.json",
];

/// Those of `paths` (as git lists them, `/` between names) whose names look as if they hold secrets, in their order.
pub fn sensitive(paths: &[String]) -> Vec<String> {
  paths.iter().filter(|path| looks_sensitive(path)).cloned().collect()
}

fn looks_sensitive(path: &str) -> bool {
  let name = path.rsplit('/').next().unwrap_or(path).to_lowercase();
  let environment = name == ".env" || name.starts_with(".env.");
  if environment {
    return !EXAMPLE_ENDINGS.iter().any(|ending| name.ends_with(ending));
  }
  NAMES.contains(&name.as_str()) || KEY_ENDINGS.iter().any(|ending| name.ends_with(ending))
}
