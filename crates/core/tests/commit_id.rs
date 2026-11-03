//! A Git commit id: the full object id of a SHA-1 or SHA-256 repository, in lowercase hex, read and sent as a string.

use orchester_core::run::{CommitId, CommitIdError};
use serde_json::json;

#[test]
fn commit_id_accepts_sha1_and_sha256_lowercase_hex() {
  for value in ["0123456789abcdef".repeat(4)[..40].to_owned(), "a".repeat(64)] {
    let commit = CommitId::new(&value).expect("a commit id");
    assert_eq!(commit.as_str(), value);
    assert_eq!(serde_json::to_value(&commit).expect("serializes"), json!(value));
    assert_eq!(serde_json::from_value::<CommitId>(json!(value)).expect("reads"), commit);
  }
}

#[test]
fn commit_id_refuses_other_lengths_uppercase_and_non_hex() {
  let invalid = [
    String::new(),
    "a".repeat(7),
    "a".repeat(39),
    "a".repeat(41),
    "a".repeat(63),
    "a".repeat(65),
    "A".repeat(40),
    format!("{}F", "a".repeat(63)),
    format!("{}g", "a".repeat(39)),
    format!("{} ", "a".repeat(39)),
    "main".to_owned(),
  ];
  for value in invalid {
    assert_eq!(CommitId::new(&value), Err(CommitIdError::Invalid(value.clone())));
    assert!(serde_json::from_value::<CommitId>(json!(value)).is_err(), "{value}");
  }
}
