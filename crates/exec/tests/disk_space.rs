//! The disk guard's reading of free space, and folder sizes (ORC-114).

use std::fs;

use orchester_core::workspaces::DiskSpace;
use orchester_exec::disk_space::{LocalDisks, folder_size};

#[test]
fn disk_space_reads_the_disk_of_a_folder_and_of_one_still_to_be_made() {
  let folder = tempfile::tempdir().expect("a temporary folder");
  let usage = LocalDisks.usage(folder.path()).expect("the free space");
  assert!(usage.total > 0 && usage.free <= usage.total);
  let later = LocalDisks
    .usage(&folder.path().join("workspaces/1"))
    .expect("the free space of its disk");
  assert_eq!(later.total, usage.total);
}

#[test]
fn disk_space_folder_size_adds_every_file_below() {
  let folder = tempfile::tempdir().expect("a temporary folder");
  fs::create_dir_all(folder.path().join("target/debug/deps")).expect("folders");
  fs::write(folder.path().join("target/debug/deps/a"), [0_u8; 1000]).expect("a file");
  fs::write(folder.path().join("target/b"), [0_u8; 24]).expect("a file");
  assert_eq!(folder_size(&folder.path().join("target")).expect("its size"), 1024);
  assert_eq!(folder_size(&folder.path().join("gone")).expect("nothing"), 0);
}
