use orchester_core::repository::{RepositoryIdentity, RepositoryIdentityError};

#[test]
fn s1b_remote_identity_matches_protocols_case_ports_and_suffixes() {
  let expected = RepositoryIdentity::from_remote_url("https://host.example/Owner/Name.git/").expect("identity");
  for value in [
    "https://HOST.EXAMPLE:443/owner/name",
    "ssh://git@host.example:22/OWNER/NAME.git",
    "git@host.example:owner/name.git",
    "host.example:owner/name",
    "https://user:secret@host.example/Owner/Name.git",
  ] {
    assert_eq!(
      RepositoryIdentity::from_remote_url(value).expect("network URL"),
      expected
    );
  }
  assert_eq!(expected.as_str(), "host.example/owner/name");
  assert_eq!(
    RepositoryIdentity::parse(expected.as_str()).expect("canonical"),
    expected
  );
  assert_eq!(
    RepositoryIdentity::from_remote_url("ssh://git@[::1]:22/owner/name.git")
      .expect("IPv6")
      .as_str(),
    "[::1]/owner/name"
  );
  assert_eq!(
    RepositoryIdentity::from_remote_url("git@[::1]:owner/name.git")
      .expect("SCP IPv6")
      .as_str(),
    "[::1]/owner/name"
  );
}

#[test]
fn s1b_remote_identity_refuses_local_missing_and_secret_output() {
  assert_eq!(
    RepositoryIdentity::from_remote_url(""),
    Err(RepositoryIdentityError::MissingOrigin)
  );
  for local in [
    "/repo",
    "../repo",
    "./a:b",
    "C:/repo",
    "C:\\repo",
    "file:///repo",
    "owner/repo",
  ] {
    assert_eq!(
      RepositoryIdentity::from_remote_url(local),
      Err(RepositoryIdentityError::LocalOrigin)
    );
  }
  for invalid in [
    "https://user:secret@host/",
    "https://user:secret@host/../repo",
    "https://user:secret@host/owner/repo?token=secret",
  ] {
    let error = RepositoryIdentity::from_remote_url(invalid).expect_err("invalid URL");
    assert!(!format!("{error:?} {error}").contains("secret"));
  }
  let identity = RepositoryIdentity::from_remote_url("https://user:secret@host/owner/repo").expect("URL");
  assert!(!format!("{identity:?}").contains("secret"));
}
