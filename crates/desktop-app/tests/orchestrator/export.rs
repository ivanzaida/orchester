//! *Export conversation…* of an Orchestrator conversation (ORC-327, 07c `ovxpJ`, 07d `alIPD`): the header's … opens
//! a menu with *Copy conversation as Markdown* and *Export conversation…*; the dialog says what it covers, offers
//! *Messages only* and *Messages and tool calls*, names the file, and *Save…* writes the chosen Markdown where the
//! save dialog said, then closes; a write that failed says why and the dialog stays.

use orchester_app::routes::OrchestratorChat;

use super::{
  harness::{Orchestrator, Setup},
  support::clipboard::own_clipboard,
};

const ASKED: &str = "Write the brief for KON-31.";

/// The Orchestrator answered [`ASKED`].
fn answered(screen: &mut Orchestrator) {
  screen.go(OrchestratorChat::New);
  screen.send(ASKED);
  screen.until("the answer", |screen| {
    !screen.exists("orchestrator-thinking") && screen.text("orchestrator-conversation").contains("Noted: ")
  });
}

/// The dialog, opened from the header's menu.
fn open_dialog(screen: &mut Orchestrator) {
  screen.press("orchestrator-more");
  screen.until("the menu", |screen| screen.exists("orchestrator-more-panel"));
  assert!(
    screen
      .text("orchestrator-copy-all")
      .contains("Copy conversation as Markdown")
  );
  screen.press("orchestrator-export");
  screen.until("the dialog", |screen| screen.exists("orchestrator-export-dialog"));
}

#[test]
fn orchestrator_export_saves_the_chosen_markdown() {
  let (mut screen, stand_ins) = Orchestrator::with_stand_ins(Setup::Ready);
  answered(&mut screen);
  open_dialog(&mut screen);
  assert_eq!(screen.text("orchestrator-export-dialog-title"), "Export conversation");
  let what = screen.text("orchestrator-export-dialog-what");
  assert!(
    what.starts_with("Write the brief for KON-31. · 2 messages · "),
    "{what}"
  );
  assert_eq!(
    screen.text("orchestrator-export-dialog-file-name"),
    "write-the-brief-for-kon-31.md"
  );
  assert!(
    screen
      .text("orchestrator-export-dialog-file-format")
      .starts_with("Markdown · ≈ ")
  );
  for include in ["messages-only", "with-tools"] {
    if !screen.exists("orchestrator-export-dialog") {
      open_dialog(&mut screen);
    }
    screen.press(&format!("orchestrator-export-dialog-{include}"));
    let path = screen.root.join(format!("{include}.md"));
    stand_ins.save_at(Some(path.clone()));
    screen.press("orchestrator-export-dialog-save");
    screen.until("the file", |screen| {
      path.exists() && !screen.exists("orchestrator-export-dialog")
    });
    let markdown = std::fs::read_to_string(&path).expect("the file");
    assert!(markdown.starts_with("# Write the brief for KON-31.\n"), "{markdown}");
    assert!(markdown.contains("### You · "), "{markdown}");
    assert!(markdown.contains("### Orchestrator · "), "{markdown}");
    assert!(markdown.contains(&format!("Noted: {ASKED}")), "{markdown}");
  }
  let asked = stand_ins.asked_to_save.lock().clone();
  assert_eq!(asked.len(), 2);
  assert_eq!(asked[0].file_name, "write-the-brief-for-kon-31.md");
}

#[test]
fn orchestrator_export_says_why_a_write_failed() {
  let (mut screen, stand_ins) = Orchestrator::with_stand_ins(Setup::Ready);
  answered(&mut screen);
  open_dialog(&mut screen);
  let missing = screen.root.join("gone").join("brief.md");
  stand_ins.save_at(Some(missing.clone()));
  screen.press("orchestrator-export-dialog-save");
  screen.until("why", |screen| screen.exists("orchestrator-export-dialog-error"));
  let why = screen.text("orchestrator-export-dialog-error");
  assert!(why.contains("its folder no longer exists"), "{why}");
  assert!(screen.exists("orchestrator-export-dialog"), "the dialog stays");
  assert!(!missing.exists());
  stand_ins.save_at(None);
  screen.press("orchestrator-export-dialog-save");
  screen.until("the closed save dialog", |screen| {
    screen.exists("orchestrator-export-dialog-save")
  });
  assert!(
    screen.exists("orchestrator-export-dialog"),
    "a closed save dialog writes nothing"
  );
}

#[test]
fn orchestrator_export_copies_the_messages_from_the_menu() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let copied = own_clipboard(&mut screen.window, true);
  answered(&mut screen);
  screen.press("orchestrator-more");
  screen.until("the menu", |screen| screen.exists("orchestrator-copy-all"));
  screen.press("orchestrator-copy-all");
  screen.until("the copy", |_| !copied.lock().is_empty());
  let markdown = copied.lock()[0].clone();
  assert!(markdown.starts_with("# Write the brief for KON-31.\n"), "{markdown}");
  assert!(markdown.contains(&format!("Noted: {ASKED}")), "{markdown}");
  assert!(!screen.exists("orchestrator-export-dialog"), "the copy needs no dialog");
}
