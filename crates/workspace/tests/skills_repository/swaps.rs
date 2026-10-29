//! Only disposable fixtures are substituted, deterministically at the actual filesystem access boundary.
use std::{cell::Cell, fs};

use orchester_workspace::skills::repository::{self, AccessPoint, ScanError};

use super::{Fixture, directory_link, write};

#[test]
fn skills_repository_swap_before_open_rejects_replaced_directory_and_ancestor() {
  for relative in [".claude", ".claude/skills", ".claude/skills/checks/reference"] {
    let f = Fixture::new();
    let skill = f.skill();
    write(&skill.join("reference/notes.md"), "Owned reference");
    write(
      &f.outside.join("PRIVATE.md"),
      "External directory names must not be enumerated",
    );
    let victim = f.root.join(relative);
    let point = if relative == ".claude" {
      f.root.join(".claude/skills")
    } else {
      victim.clone()
    };
    let did_swap = Cell::new(false);
    let substituted = Cell::new(false);
    let enumerated = Cell::new(false);
    let result = repository::scan_with_access_observer(&f.root, &|stage, path| {
      if stage == AccessPoint::Enumerate && path == point {
        enumerated.set(true);
      }
      if stage == AccessPoint::Open && path == point && !did_swap.replace(true) {
        let moved = fs::rename(&victim, f.root.join("held-original"));
        match moved {
          Ok(()) => {
            directory_link(&f.outside, &victim);
            substituted.set(true);
          }
          #[cfg(windows)]
          Err(error) => assert_eq!(
            error.raw_os_error(),
            Some(32),
            "held chain refuses substitution: {relative}"
          ),
          #[cfg(unix)]
          Err(error) => panic!("replace private directory {relative}: {error}"),
        }
      }
    });
    assert!(did_swap.get());
    if substituted.get() {
      assert!(!enumerated.get(), "never enumerate through the substituted parent");
      assert!(result.is_err(), "substitution must be refused before enumeration");
    }
    match result {
      Err(error) => assert!(matches!(
        error,
        ScanError::NotRegular(_) | ScanError::Read { .. } | ScanError::Outside(_)
      )),
      Ok(scan) => {
        use orchester_core::agent_files::import::Candidate;
        assert!(
          scan
            .items
            .iter()
            .all(|item| matches!(&item.outcome, Ok(Candidate::Skill { files, .. })
          if files.iter().all(|file| file.content.as_ref() != b"External directory names must not be enumerated")))
        );
      }
    }
    assert_eq!(
      fs::read_to_string(f.outside.join("PRIVATE.md")).expect("outside"),
      "External directory names must not be enumerated"
    );
  }
}
#[test]
fn skills_repository_swap_after_open_reads_the_held_regular_object() {
  let f = Fixture::new();
  let note = f.skill().join("notes.md");
  write(&note, "Admitted owned bytes");
  let reached = Cell::new(false);
  let scan = repository::scan_with_access_observer(&f.root, &|stage, path| {
    if stage == AccessPoint::Read && path == note && !reached.replace(true) {
      let replaced = fs::rename(&note, note.with_extension("held"));
      #[cfg(windows)]
      assert!(
        replaced.is_err(),
        "held object refuses replacement until byte access ends"
      );
      #[cfg(unix)]
      {
        replaced.expect("Unix entry may move; open object remains the admitted one");
        fs::create_dir(&note).expect("substitute nonregular entry");
      }
    }
  })
  .expect("read admitted object");
  assert!(reached.get());
  use orchester_core::agent_files::import::Candidate;
  assert!(scan.items.iter().any(|item| matches!(&item.outcome,
    Ok(Candidate::Skill { files, .. }) if files.iter().any(|file| file.content.as_ref() == b"Admitted owned bytes"))));
}
#[test]
fn skills_repository_swap_before_enumeration_stays_on_the_held_owned_parent_chain() {
  let f = Fixture::new();
  let skill = f.skill();
  let folder = skill.join("reference");
  write(&folder.join("notes.md"), "Held reference");
  write(&f.outside.join("PRIVATE.md"), "Unowned bytes");
  let reached = Cell::new(false);
  let external_name_seen = Cell::new(false);
  let scan = repository::scan_with_access_observer(&f.root, &|stage, path| {
    if stage == AccessPoint::ListedEntry && path.file_name().is_some_and(|name| name == "PRIVATE.md") {
      external_name_seen.set(true);
    }
    if stage == AccessPoint::Enumerate && path == folder && !reached.replace(true) {
      let moved = fs::rename(&folder, skill.join("admitted-reference"));
      #[cfg(windows)]
      assert!(moved.is_err(), "the held parent chain cannot be swapped");
      #[cfg(unix)]
      {
        moved.expect("Unix descriptor keeps original directory");
        directory_link(&f.outside, &folder);
      }
    }
  });
  assert!(reached.get());
  assert!(
    !external_name_seen.get(),
    "rejecting after external enumeration would already be too late"
  );
  // Unix may reject a later canonical child lookup; no external enumeration or byte access is accepted.
  match scan {
    Ok(scan) => {
      use orchester_core::agent_files::import::Candidate;
      assert!(
        scan
          .items
          .iter()
          .all(|item| matches!(&item.outcome, Ok(Candidate::Skill { files, .. })
        if files.iter().all(|file| file.content.as_ref() != b"Unowned bytes")))
      );
    }
    Err(error) => assert!(matches!(
      error,
      ScanError::Outside(_) | ScanError::Read { .. } | ScanError::NotRegular(_)
    )),
  }
}
#[test]
fn skills_repository_swap_before_file_open_rejects_nonregular_replacement() {
  let f = Fixture::new();
  let note = f.skill().join("SKILL.md");
  write(&note, "Owned bytes");
  let swapped = Cell::new(false);
  let error = repository::scan_with_access_observer(&f.root, &|stage, path| {
    if stage == AccessPoint::Open && path == note && !swapped.replace(true) {
      fs::remove_file(&note).expect("replace private file");
      fs::create_dir(&note).expect("nonregular substitution");
    }
  })
  .expect_err("no bytes read from a nonregular object");
  assert!(swapped.get());
  assert!(matches!(error, ScanError::NotRegular(_)));
}

#[cfg(unix)]
#[test]
fn skills_repository_swap_to_fifo_before_open_does_not_block_or_read() {
  use std::{os::unix::fs::OpenOptionsExt, sync::mpsc, time::Duration};
  let f = Fixture::new();
  let note = f.skill().join("notes.md");
  write(&note, "Owned bytes");
  let root = f.root;
  let path = note.clone();
  let (send, receive) = mpsc::channel();
  let reader = std::thread::spawn(move || {
    let swapped = Cell::new(false);
    let result = repository::scan_with_access_observer(&root, &|stage, entry| {
      if stage == AccessPoint::Open && entry == path && !swapped.replace(true) {
        fs::remove_file(&path).expect("replace private file");
        assert!(
          std::process::Command::new("mkfifo")
            .arg(&path)
            .status()
            .expect("fifo")
            .success()
        );
      }
    });
    send.send(result).expect("report actual result");
  });
  let result = receive.recv_timeout(Duration::from_secs(3));
  if result.is_err() {
    // Release an incorrectly blocking reader, without making the regression hang or reading private bytes.
    let release = fs::OpenOptions::new()
      .write(true)
      .custom_flags(rustix::fs::OFlags::NONBLOCK.bits() as i32)
      .open(&note);
    drop(release);
  }
  reader.join().expect("reader");
  assert!(matches!(
    result.expect("special substitution must return without a writer"),
    Err(ScanError::NotRegular(_))
  ));
}
