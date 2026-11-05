//! The `docker` CLI for database containers: one missing from the `PATH` names the folders searched, one whose engine
//! does not answer is not available either, both at once and typed; and the port an image exposes is read from its
//! configuration. `docker` stand-in is `orchester-exit-with`, which says nothing, as a CLI without its engine does
//! not name a server version. A real Docker is used by the runner's `service_data` tests.
//!
//! `cargo test -p orchester-exec --features test-program --test docker docker_`

use std::{
  ffi::OsString,
  fs,
  path::PathBuf,
  time::{Duration, Instant},
};

use orchester_exec::docker::{Docker, DockerUnavailable, lowest_tcp_port};
use tempfile::TempDir;

const STAND_IN: &str = env!("CARGO_BIN_EXE_orchester-exit-with");

fn scratch() -> TempDir {
  let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/docker-tests");
  fs::create_dir_all(&root).expect("a scratch folder");
  tempfile::tempdir_in(root).expect("a folder of the test's own")
}

fn path_of(folder: &std::path::Path) -> OsString {
  std::env::join_paths([folder]).expect("a PATH")
}

#[test]
fn docker_missing_names_the_folders_searched() {
  let empty = scratch();
  let started = Instant::now();
  let error = Docker::ready(Some(&path_of(empty.path()))).expect_err("no docker there");
  assert!(started.elapsed() < Duration::from_secs(5), "it fails at once");
  assert_eq!(
    error,
    DockerUnavailable::NotFound {
      searched: vec![empty.path().to_path_buf()]
    }
  );
  let said = error.to_string();
  assert!(
    said.starts_with("Docker is not available: no `docker` program was found"),
    "{said}"
  );
  assert!(said.contains(&empty.path().display().to_string()), "{said}");
  assert_eq!(
    Docker::find(None),
    Err(DockerUnavailable::NotFound { searched: Vec::new() })
  );
}

#[test]
fn docker_whose_engine_does_not_answer_is_not_available() {
  let folder = scratch();
  let name = if cfg!(windows) { "docker.exe" } else { "docker" };
  let program = folder.path().join(name);
  fs::copy(STAND_IN, &program).expect("a stand-in docker");
  let path = path_of(folder.path());
  assert_eq!(Docker::find(Some(&path)).expect("found").program(), program);
  let started = Instant::now();
  let error = Docker::ready(Some(&path)).expect_err("its engine names no version");
  assert!(started.elapsed() < Duration::from_secs(10), "it fails at once");
  let DockerUnavailable::NotRunning { program: asked, answer } = &error else {
    panic!("not running: {error:?}");
  };
  assert_eq!(asked, &program);
  assert_eq!(answer, "it exited with 0 and said nothing");
  assert!(
    error
      .to_string()
      .starts_with("Docker is not available: the Docker engine did not answer `"),
    "{error}"
  );
}

#[test]
fn docker_image_port_is_its_lowest_exposed_tcp_port() {
  assert_eq!(lowest_tcp_port(r#"{"5432/tcp":{}}"#), Some(5432));
  assert_eq!(
    lowest_tcp_port("{\"8080/tcp\":{},\"443/tcp\":{},\"53/udp\":{}}\n"),
    Some(443)
  );
  assert_eq!(lowest_tcp_port(r#"{"6379":{}}"#), Some(6379));
  for none in ["null", "{}", r#"{"53/udp":{}}"#, "", "not json"] {
    assert_eq!(lowest_tcp_port(none), None, "{none}");
  }
}
