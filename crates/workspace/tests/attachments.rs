//! Attachments as the composers take them (ORC-57): a file picked or dropped is read only when it is a plain file of a
//! kind Orchester takes, a folder or a shortcut is refused without being followed, and a message's limits hold; each
//! refusal says why in the person's words.

use std::fs;

use orchester_core::attachment::{
  AttachmentRefusal, MediaType,
  limits::{MAX_ATTACHMENT_BYTES, MAX_ATTACHMENTS},
};
use orchester_workspace::attachments::{Refused, add, check_message, pasted, read_file, read_files};

#[test]
fn a_plain_file_is_read_and_told_apart_by_its_content() {
  let folder = tempfile::tempdir().expect("a folder");
  let path = folder.path().join("notes.log");
  fs::write(&path, "line one\nline two\n").expect("written");
  let read = read_file(&path).expect("taken");
  assert_eq!(read.name().as_str(), "notes.log");
  assert_eq!(read.media(), MediaType::Text);
  assert_eq!(read.text(), Some("line one\nline two\n"));
}

#[test]
fn folders_shortcuts_binaries_and_large_files_are_refused_with_why() {
  let folder = tempfile::tempdir().expect("a folder");
  let inner = folder.path().join("docs");
  fs::create_dir(&inner).expect("made");
  let shortcut = folder.path().join("report.lnk");
  let mut lnk = vec![
    0x4C, 0x00, 0x00, 0x00, 0x01, 0x14, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x46,
  ];
  lnk.extend([0; 16]);
  fs::write(&shortcut, lnk).expect("written");
  let binary = folder.path().join("tool.exe");
  fs::write(&binary, [0x4D, 0x5A, 0x90, 0x00, 0x03]).expect("written");
  let empty = folder.path().join("empty.txt");
  fs::write(&empty, "").expect("written");
  let large = folder.path().join("huge.txt");
  fs::File::create(&large)
    .and_then(|file| file.set_len(MAX_ATTACHMENT_BYTES + 1))
    .expect("sized");
  let missing = folder.path().join("gone.txt");
  let paths = [inner, shortcut, binary, empty, large, missing];
  let (taken, refused) = read_files(&paths);
  assert!(taken.is_empty());
  let why: Vec<(String, AttachmentRefusal)> = refused
    .into_iter()
    .map(|refused| (refused.name, refused.refusal))
    .collect();
  assert_eq!(
    why,
    [
      ("docs".to_owned(), AttachmentRefusal::Folder),
      ("report.lnk".to_owned(), AttachmentRefusal::Shortcut),
      ("tool.exe".to_owned(), AttachmentRefusal::Unsupported),
      ("empty.txt".to_owned(), AttachmentRefusal::Empty),
      (
        "huge.txt".to_owned(),
        AttachmentRefusal::TooLarge {
          limit: MAX_ATTACHMENT_BYTES
        }
      ),
      ("gone.txt".to_owned(), AttachmentRefusal::Unreadable),
    ]
  );
}

#[test]
fn refusals_name_the_file_and_the_limit() {
  let too_large = Refused::new(
    "huge.txt",
    AttachmentRefusal::TooLarge {
      limit: MAX_ATTACHMENT_BYTES,
    },
  )
  .message();
  assert_eq!(too_large.key, "attachments.refused.tooLarge");
  assert_eq!(
    too_large.args,
    [("name", "huge.txt".to_owned()), ("limit", "20".to_owned())]
  );
  let folder = Refused::new("docs", AttachmentRefusal::Folder).message();
  assert_eq!(folder.key, "attachments.refused.folder");
}

#[test]
fn a_message_holds_the_files_its_limits_allow() {
  let mut held = Vec::new();
  let files: Vec<_> = (0..=MAX_ATTACHMENTS)
    .map(|n| pasted(&format!("{n}.txt"), b"x".to_vec()).expect("text"))
    .collect();
  let refused = add(&mut held, files);
  assert_eq!(held.len(), MAX_ATTACHMENTS);
  assert_eq!(refused.len(), 1);
  assert_eq!(
    refused[0].refusal,
    AttachmentRefusal::TooMany { limit: MAX_ATTACHMENTS }
  );
  assert_eq!(check_message(&[], &held), Ok(()));
  held.push(pasted("extra.txt", b"x".to_vec()).expect("text"));
  assert_eq!(
    check_message(&[], &held).map_err(|refused| refused.refusal),
    Err(AttachmentRefusal::TooMany { limit: MAX_ATTACHMENTS })
  );
}
