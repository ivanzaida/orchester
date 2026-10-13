//! The token in use and the stored one never disagree: a read that started before a rotation does not put the old
//! token back, and two first uses make one token. Reads are held in the store to stage each race.

use std::{thread, time::Duration};

use super::*;

/// How long the second party is given to reach the store or the lock before the held read goes on.
const GIVEN: Duration = Duration::from_millis(100);

/// An installation whose server is off from its first start: no token read or made yet.
fn off_from_the_start(vault: &Arc<MemoryVault>) -> Installed {
  let data = Data::new();
  let launch = data.launch();
  launch
    .workspace()
    .expect("a session")
    .set_mcp_server_enabled(false)
    .expect("kept");
  launch.end();
  Installed::start_with(data, vault.clone(), Some(free_port()))
}

fn wait_for_held_reads(vault: &MemoryVault, count: usize) {
  let until = std::time::Instant::now() + Duration::from_secs(5);
  while vault.waiting_reads() < count {
    assert!(std::time::Instant::now() < until, "a read reaches the store");
    thread::sleep(Duration::from_millis(5));
  }
}

#[test]
fn app_mcp_a_read_before_a_rotation_does_not_bring_the_old_token_back() {
  let vault = Arc::new(MemoryVault::default());
  let installed = off_from_the_start(&vault);
  installed.server.read_token().expect("a first token is made");
  let installed = installed.restart();
  assert!(installed.server.token().is_none(), "not read since the restart");
  vault.hold_reads();
  let reader = {
    let server = installed.server.clone();
    thread::spawn(move || server.read_token().map(|token| token.expose().to_owned()))
  };
  wait_for_held_reads(&vault, 1);
  let rotator = {
    let server = installed.server.clone();
    thread::spawn(move || server.rotate_token())
  };
  thread::sleep(GIVEN);
  vault.release_reads();
  reader.join().expect("the reader ends").expect("read");
  rotator.join().expect("the rotation ends").expect("rotated");
  let stored = vault.values().into_values().next().expect("one token stored");
  assert_eq!(installed.token(), stored, "the token in use is the stored one");
}

#[test]
fn app_mcp_two_first_uses_make_one_token() {
  let vault = Arc::new(MemoryVault::default());
  let installed = off_from_the_start(&vault);
  vault.hold_reads();
  let readers: Vec<_> = (0..2)
    .map(|_| {
      let server = installed.server.clone();
      thread::spawn(move || server.read_token().map(|token| token.expose().to_owned()))
    })
    .collect();
  wait_for_held_reads(&vault, 1);
  thread::sleep(GIVEN);
  vault.release_reads();
  let tokens: Vec<String> = readers
    .into_iter()
    .map(|reader| reader.join().expect("the reader ends").expect("read"))
    .collect();
  assert_eq!(tokens[0], tokens[1], "both got the same token");
  assert_eq!(vault.stores(), 1, "one token made");
}
