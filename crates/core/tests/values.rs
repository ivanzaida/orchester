use std::num::NonZeroU32;

use orchester_core::{
  id::TaskId,
  name::{Name, NameError},
  project::{ProjectKey, TaskKey},
};

#[test]
fn ids_are_time_ordered_and_round_trip_through_text() {
  let first = TaskId::generate();
  let second = TaskId::generate();
  assert!(first < second);
  assert_eq!(first.to_string().parse::<TaskId>().unwrap(), first);
}

#[test]
fn name_is_trimmed_and_bounded() {
  assert_eq!(Name::new("  Add search ").unwrap().as_str(), "Add search");
  assert_eq!(Name::new("   "), Err(NameError::Blank));
  let long = "x".repeat(Name::MAX_CHARS + 1);
  assert_eq!(
    Name::new(&long),
    Err(NameError::TooLong {
      length: Name::MAX_CHARS + 1
    })
  );
}

#[test]
fn project_key_formats_task_key() {
  let key = TaskKey::new(ProjectKey::new("ORC").unwrap(), NonZeroU32::new(129).unwrap());
  assert_eq!(key.to_string(), "ORC-129");
}

#[test]
fn project_key_rejects_invalid_values() {
  for invalid in ["", "O", "orc", "1RC", "OR-C", "ABCDEFGHIJK"] {
    assert!(ProjectKey::new(invalid).is_err(), "{invalid} should be rejected");
  }
  assert!(ProjectKey::new("K2").is_ok());
}

#[test]
fn project_key_is_suggested_from_the_name() {
  let suggest = |name: &str| {
    ProjectKey::suggest(&Name::new(name).expect("valid name"))
      .as_str()
      .to_owned()
  };
  assert_eq!(suggest("Kontur"), "KON");
  assert_eq!(suggest("orchester"), "ORC");
  assert_eq!(suggest("Personal website"), "PW");
  assert_eq!(suggest("my-cool api v2"), "MCAV");
  assert_eq!(suggest("2048 game"), "GAM");
  assert_eq!(suggest("Контур"), "PRJ");
  assert_eq!(suggest("A"), "PRJ");
}
