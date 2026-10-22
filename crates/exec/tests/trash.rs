//! The system's Recycle Bin / Trash. Only the ignored tests move anything into the person's own bin, by hand; the
//! others check the refusals that come before any move, and the workspace's tests and QA copies use a trash folder of
//! their own (`orchester_workspace::accounts::trash`).

use orchester_exec::trash::{Bin, TrashRefusal, ensure_movable};

#[test]
#[ignore = "moves a folder into the person's real Recycle Bin / Trash; run by hand: cargo test -p orchester-exec --test trash -- --ignored"]
fn trash_moves_a_folder_to_the_systems_bin() {
  let folder = tempfile::tempdir().expect("a temporary folder").keep();
  std::fs::write(folder.join("note.txt"), "Orchester's trash check").expect("a file");
  orchester_exec::trash::move_to_trash(&folder).expect("moved");
  assert!(!folder.exists(), "the folder left its place for the bin");
}

#[test]
#[ignore = "moves a file into the person's real Recycle Bin / Trash; run by hand: cargo test -p orchester-exec --test trash -- --ignored"]
fn trash_moves_a_file_to_the_systems_bin() {
  let folder = tempfile::tempdir().expect("a temporary folder");
  let file = folder.path().join("orchester-trash-check.txt");
  std::fs::write(&file, "Orchester's trash check").expect("a file");
  orchester_exec::trash::move_to_trash(&file).expect("moved");
  assert!(!file.exists(), "the file left its place for the bin");
}

#[test]
fn trash_refuses_a_volume_without_a_bin_before_moving() {
  let refused = Bin::Missing.ensure_room(1).expect_err("no bin keeps it");
  assert_eq!(TrashRefusal::of(&refused), Some(TrashRefusal::NoBin));
}

#[test]
fn trash_refuses_an_item_larger_than_the_bin_keeps() {
  let bin = Bin::Keeps { capacity: Some(10) };
  let refused = bin.ensure_room(11).expect_err("too large");
  assert_eq!(TrashRefusal::of(&refused), Some(TrashRefusal::TooLarge));
  bin.ensure_room(10).expect("fits");
  Bin::Keeps { capacity: None }
    .ensure_room(u64::MAX)
    .expect("no size said");
}

#[cfg(windows)]
#[test]
fn trash_finds_no_bin_on_a_network_path() {
  let path = std::path::Path::new(r"\\?\UNC\localhost\c$\orchester-trash-check.txt");
  assert_eq!(orchester_exec::trash::bin_of(path).expect("an answer"), Bin::Missing);
}

/// Another program holds the file, reading and writing shared but not moving: refused with the system's reason.
#[cfg(windows)]
#[test]
fn trash_refuses_a_file_another_program_holds() {
  use std::os::windows::fs::OpenOptionsExt;

  let folder = tempfile::tempdir().expect("a temporary folder");
  let file = folder.path().join("held.log");
  std::fs::write(&file, "held").expect("a file");
  let held = std::fs::OpenOptions::new()
    .read(true)
    .share_mode(0x1 | 0x2)
    .open(&file)
    .expect("held");
  let refused = ensure_movable(&file).expect_err("in use");
  assert_eq!(refused.raw_os_error(), Some(32), "a sharing violation: {refused}");
  drop(held);
  ensure_movable(&file).expect("free once let go");
  assert!(file.exists());
}

/// Reading where a local folder's bin stands moves nothing; whichever answer the person's settings give is one.
#[test]
fn trash_reads_the_bin_of_a_local_folder() {
  let folder = tempfile::tempdir().expect("a temporary folder");
  orchester_exec::trash::bin_of(folder.path()).expect("an answer");
}
