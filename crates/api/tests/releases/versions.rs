//! Release versions, channels and file names.

use orchester_api::releases::{Channel, ReleaseFileName, ReleaseFormatError, ReleaseVersion};

use super::version;

#[test]
fn releases_versions_order_as_semantic_versioning() {
  let ordered = [
    "0.4.0-beta.1",
    "0.4.0-beta.2",
    "0.4.0-beta.10",
    "0.4.0-rc.1",
    "0.4.0",
    "0.4.1",
    "0.10.0",
    "1.0.0-alpha",
    "1.0.0-alpha.1",
    "1.0.0-alpha.beta",
    "1.0.0-beta",
    "1.0.0-beta.2",
    "1.0.0-beta.11",
    "1.0.0-rc.1",
    "1.0.0",
  ];
  for pair in ordered.windows(2) {
    assert!(version(pair[0]) < version(pair[1]), "{} < {}", pair[0], pair[1]);
  }
}

#[test]
fn releases_versions_refuse_other_spellings() {
  let refused = [
    "",
    "1.0",
    "1.0.0.0",
    "01.0.0",
    "1.0.0+build.1",
    "1.0.0-",
    "1.0.0-01",
    "1.0.0-beta..1",
    "1.0.0-beta_1",
    "v1.0.0",
    "1.0.0 ",
    "4294967296.0.0",
  ];
  for text in refused {
    assert_eq!(
      ReleaseVersion::parse(text),
      Err(ReleaseFormatError::Version),
      "{text:?}"
    );
  }
  assert!(ReleaseVersion::parse(&format!("1.0.0-{}", "a".repeat(58))).is_ok());
  assert!(ReleaseVersion::parse(&format!("1.0.0-{}", "a".repeat(59))).is_err());
}

#[test]
fn releases_pre_release_is_beta_only() {
  let beta = version("0.5.0-beta.1");
  assert!(beta.is_pre_release());
  assert!(beta.allowed_on(Channel::Beta));
  assert!(!beta.allowed_on(Channel::Stable));
  let stable = version("0.5.0");
  assert!(stable.allowed_on(Channel::Stable) && stable.allowed_on(Channel::Beta));
}

#[test]
fn releases_channels_have_their_wire_names() {
  assert_eq!(Channel::parse("stable"), Some(Channel::Stable));
  assert_eq!(Channel::parse("beta"), Some(Channel::Beta));
  assert_eq!(Channel::parse("Beta"), None);
}

#[test]
fn releases_file_names_keep_to_url_safe_characters() {
  for text in ["Orchester-0.4.0-windows-x86_64.msi", "Orchester_0.4.0+mac.dmg", "a"] {
    assert_eq!(ReleaseFileName::parse(text).map(String::from).as_deref(), Ok(text));
  }
  let long = "a".repeat(129);
  for text in [
    "",
    ".hidden",
    "-flag",
    "a/b",
    "a\\b",
    "a b",
    "a\nb",
    "a%2f",
    long.as_str(),
  ] {
    assert_eq!(
      ReleaseFileName::parse(text),
      Err(ReleaseFormatError::FileName),
      "{text:?}"
    );
  }
}
