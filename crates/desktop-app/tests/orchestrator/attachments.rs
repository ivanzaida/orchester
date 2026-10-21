//! Files in the Orchestrator's composer (ORC-57): *Attach* hands the picker's files to the draft and what the message
//! cannot take is refused under the box with why, a drop and a paste attach too, the draft keeps its files through
//! another screen, and a message sent with a file is kept with it, shows it, and reaches the agent as a content block.

use std::{fs, path::Path};

use lurq::app::events::{ClipboardContent, ClipboardImage, DraggedPaths};
use orchester_app::routes::OrchestratorChat;
use orchester_core::attachment::MediaType;

use super::{
  drafts::leave,
  harness::{Orchestrator, Setup},
};

/// A 2 by 1 PNG, in a file named `name` in `folder`.
fn png_file(folder: &Path, name: &str) -> std::path::PathBuf {
  let image = ClipboardImage::from_rgba(2, 1, vec![255; 8]).expect("pixels");
  let path = folder.join(name);
  fs::write(&path, image.to_png().expect("a PNG")).expect("written");
  path
}

/// Waits until the composer's chips name `name`.
fn until_chip(screen: &mut Orchestrator, name: &str) {
  screen.until(&format!("the chip of {name}"), |screen| {
    screen.text("orchestrator-composer-attachments").contains(name)
  });
}

#[test]
fn orchestrator_attach_adds_the_picked_files_and_refuses_the_others_with_why() {
  let (mut screen, stand_ins) = Orchestrator::with_stand_ins(Setup::Ready);
  let folder = tempfile::tempdir().expect("a folder");
  let tool = folder.path().join("tool.exe");
  fs::write(&tool, [0x4D, 0x5A, 0x90, 0x00]).expect("written");
  stand_ins.attach(vec![png_file(folder.path(), "shot.png"), tool]);
  screen.go(OrchestratorChat::New);
  screen.click("orchestrator-attach");
  until_chip(&mut screen, "shot.png");
  screen.until("the refusal", |screen| {
    screen
      .text("orchestrator-composer-attachments-refused")
      .contains("tool.exe")
  });
  assert!(!screen.text("orchestrator-composer-attachments").contains("tool.exe"));
}

#[test]
fn orchestrator_a_drop_and_a_paste_attach_and_the_draft_keeps_them() {
  let (mut screen, _stand_ins) = Orchestrator::with_stand_ins(Setup::Ready);
  let folder = tempfile::tempdir().expect("a folder");
  let notes = folder.path().join("notes.md");
  fs::write(&notes, "# Notes").expect("written");
  let inner = folder.path().join("docs");
  fs::create_dir(&inner).expect("made");
  screen.go(OrchestratorChat::New);
  let bounds = screen.bounds("orchestrator-composer");
  let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
  assert!(
    screen
      .window
      .tree
      .file_drop(DraggedPaths::from_paths([notes, inner]), x, y)
  );
  until_chip(&mut screen, "notes.md");
  screen.until("the folder refused", |screen| {
    screen
      .text("orchestrator-composer-attachments-refused")
      .contains("docs")
  });
  screen.click("orchestrator-input");
  screen
    .window
    .tree
    .get_element_by_id_mut("orchestrator-input")
    .expect("the message field")
    .focus();
  let image = ClipboardImage::from_rgba(1, 1, vec![0, 0, 0, 255]).expect("a pixel");
  assert!(screen.window.tree.paste(ClipboardContent::image(image)));
  until_chip(&mut screen, "Pasted image.png");
  leave(&mut screen);
  screen.go(OrchestratorChat::New);
  until_chip(&mut screen, "notes.md");
  until_chip(&mut screen, "Pasted image.png");
}

#[test]
fn orchestrator_a_message_with_a_file_is_kept_shown_and_reaches_the_agent() {
  let (mut screen, stand_ins) = Orchestrator::with_stand_ins(Setup::Ready);
  let folder = tempfile::tempdir().expect("a folder");
  stand_ins.attach(vec![png_file(folder.path(), "shot.png")]);
  screen.go(OrchestratorChat::New);
  screen.click("orchestrator-attach");
  until_chip(&mut screen, "shot.png");
  screen.send("Look at this");
  screen.until("the answer", |screen| {
    screen.text("orchestrator-conversation").contains("Noted:") && !screen.exists("orchestrator-thinking")
  });
  assert!(
    !screen.text("orchestrator-composer-attachments").contains("shot.png"),
    "the chips were sent with the message"
  );
  let db = screen.db();
  let chat = db.chats().expect("chats").remove(0).chat.id();
  let attached: Vec<_> = db
    .chat_attachments(chat)
    .expect("read")
    .into_values()
    .flatten()
    .collect();
  assert_eq!(attached.len(), 1);
  assert_eq!(attached[0].name().as_str(), "shot.png");
  assert_eq!(attached[0].media(), MediaType::Png);
  assert!(db.attachment_path(&attached[0]).is_file(), "the copy is kept");
  assert!(
    screen.text("orchestrator-conversation").contains("shot.png"),
    "the sent message shows its file"
  );
  let agent = fs::read_to_string(screen.root.join(".orchester").join("fake-chat.log")).expect("the agent's log");
  assert!(
    agent.contains("image:image/png"),
    "the image reached the agent: {agent}"
  );
}
