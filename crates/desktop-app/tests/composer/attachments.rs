//! Files in the composer (ORC-57): *Attach* hands the picker's files to the draft, a paste of an image or of long text
//! and a drop of files attach them, what the message cannot take is refused under the box with why, the draft keeps
//! its files when the task is left and opened again, and a message of files only is stored with them.

use std::{fs, path::Path};

use lurq::app::events::{ClipboardContent, ClipboardImage, DraggedPaths};
use orchester_core::{attachment::MediaType, time::Timestamp};
use orchester_store::{AttachmentOwner, ProjectDb};

use super::{
  click,
  kontur::{self, SeedRequest, Seeded, scenarios},
  messages, open,
  sending::ENTER,
  support::{Data, Window, stand_ins::StandIns},
  text,
};

/// A window on `task-states` with the stand-in pickers.
fn seeded() -> (Window, StandIns, Seeded) {
  let data = Data::new();
  let request = SeedRequest {
    data_dir: data.dir.path().to_path_buf(),
    repo: data.dir.path().join("repo-task-states"),
    scenario: scenarios::named("task-states").expect("a known scenario"),
    fake_codex: None,
    now: Timestamp::now(),
  };
  let seeded = kontur::seed(&request).expect("the scenario seeds");
  let (window, stand_ins) = Window::with_stand_ins(data);
  (window, stand_ins, seeded)
}

/// A 2 by 1 PNG, in a file named `name` in `folder`.
fn png_file(folder: &Path, name: &str) -> std::path::PathBuf {
  let image = ClipboardImage::from_rgba(2, 1, vec![255; 8]).expect("pixels");
  let path = folder.join(name);
  fs::write(&path, image.to_png().expect("a PNG")).expect("written");
  path
}

/// The names the composer's chips show, joined.
fn chips(window: &mut Window) -> String {
  text(window, "composer-attachments")
}

/// Waits until the chips name `name`.
fn until_chip(window: &mut Window, name: &str) {
  window.until(&format!("the chip of {name}"), |window| chips(window).contains(name));
}

#[test]
fn composer_attach_adds_the_picked_files_and_refuses_the_others_with_why() {
  let (mut window, stand_ins, seeded) = seeded();
  let folder = tempfile::tempdir().expect("a folder");
  let shot = png_file(folder.path(), "shot.png");
  let tool = folder.path().join("tool.exe");
  fs::write(&tool, [0x4D, 0x5A, 0x90, 0x00]).expect("written");
  open(&mut window, &seeded, "Localize the invoice emails");
  stand_ins.attach(vec![shot, tool]);
  click(&mut window, "composer-attach");
  until_chip(&mut window, "shot.png");
  window.until("the refusal", |window| {
    text(window, "composer-attachments-refused").contains("tool.exe")
  });
  assert!(!chips(&mut window).contains("tool.exe"));
}

#[test]
fn composer_a_pasted_image_and_long_text_become_attachments() {
  let (mut window, _stand_ins, seeded) = seeded();
  open(&mut window, &seeded, "Localize the invoice emails");
  click(&mut window, "composer-input");
  let image = ClipboardImage::from_rgba(1, 1, vec![0, 0, 0, 255]).expect("a pixel");
  assert!(window.tree.paste(ClipboardContent::image(image)));
  until_chip(&mut window, "Pasted image.png");
  let long = "word ".repeat(500);
  assert!(window.tree.paste(ClipboardContent::text(long)));
  until_chip(&mut window, "Pasted text.txt");
  assert!(
    !text(&mut window, "composer-input").contains("word word"),
    "the long text is not in the message"
  );
  // Short text is pasted as words.
  assert!(window.tree.paste(ClipboardContent::text("short")));
  window.pass();
  assert!(text(&mut window, "composer-input").contains("short"));
}

#[test]
fn composer_a_drop_attaches_files_and_refuses_a_folder() {
  let (mut window, _stand_ins, seeded) = seeded();
  let folder = tempfile::tempdir().expect("a folder");
  let notes = folder.path().join("notes.md");
  fs::write(&notes, "# Notes").expect("written");
  let inner = folder.path().join("docs");
  fs::create_dir(&inner).expect("made");
  open(&mut window, &seeded, "Localize the invoice emails");
  let bounds = window
    .tree
    .get_element_by_id_mut("composer-box")
    .and_then(|element| element.bounds())
    .expect("the box is laid out");
  let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
  assert!(window.tree.file_drop(DraggedPaths::from_paths([notes, inner]), x, y));
  window.pass();
  until_chip(&mut window, "notes.md");
  window.until("the folder refused", |window| {
    text(window, "composer-attachments-refused").contains("docs")
  });
}

#[test]
fn composer_the_draft_keeps_its_files_when_the_task_is_left() {
  let (mut window, stand_ins, seeded) = seeded();
  let folder = tempfile::tempdir().expect("a folder");
  stand_ins.attach(vec![png_file(folder.path(), "kept.png")]);
  open(&mut window, &seeded, "Show the signed-in account");
  click(&mut window, "composer-attach");
  until_chip(&mut window, "kept.png");
  open(&mut window, &seeded, "Paginate the events API");
  window.until("the other task's own composer", |window| {
    !chips(window).contains("kept.png")
  });
  open(&mut window, &seeded, "Show the signed-in account");
  until_chip(&mut window, "kept.png");
}

#[test]
fn composer_a_message_of_files_only_is_stored_with_them() {
  let (mut window, stand_ins, seeded) = seeded();
  let folder = tempfile::tempdir().expect("a folder");
  stand_ins.attach(vec![png_file(folder.path(), "only.png")]);
  let title = "Localize the invoice emails";
  let key = open(&mut window, &seeded, title);
  click(&mut window, "composer-attach");
  until_chip(&mut window, "only.png");
  click(&mut window, "composer-input");
  super::key(&mut window, ENTER, (false, false));
  window.until("the message stored", |_| messages(&seeded, &key).len() == 1);
  let stored = &messages(&seeded, &key)[0];
  let db = ProjectDb::open(&seeded.repo).expect("open");
  let attached = db.attachments_of(AttachmentOwner::Message(stored.id())).expect("read");
  assert_eq!(attached.len(), 1);
  assert_eq!(attached[0].media(), MediaType::Png);
  assert_eq!(attached[0].name().as_str(), "only.png");
  window.until("the chips emptied", |window| !chips(window).contains("only.png"));
  // The message shows by its file, not by the name it was kept with.
  window.until("the sent file", |window| {
    window
      .tree
      .get_element_by_id(&format!("conversation-message-{}-attachments", stored.id()))
      .is_some()
  });
}
