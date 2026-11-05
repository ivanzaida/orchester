//! *Export conversation…* of a run (ORC-327, 09c `O3jhaZ`, 09d `A2YMy`): on the Conversation tab *More actions* gains
//! *Copy conversation as Markdown* and *Export conversation…* after *Copy link*; the dialog covers the whole run with
//! every step, and *Save…* writes the chosen Markdown, steps as headings, where the save dialog said. The Summary's
//! menu has no export.

use orchester_app::routes::{ProjectPage, Route, TaskView};
use orchester_core::time::Timestamp;

use super::{
  Window, key_of,
  kontur::{self, SeedRequest, scenarios},
  open, seeded,
  support::{Data, clipboard::own_clipboard},
};

const TITLE: &str = "Add retry to webhook sender";

fn press(window: &mut Window, id: &str) {
  super::click(window, id);
}

/// The menu open, its export offered.
fn open_menu(window: &mut Window) {
  press(window, "task-more");
  window.until("the export in the menu", |window| {
    window.tree.get_element_by_id("task-export-conversation").is_some()
  });
}

#[test]
fn conversation_export_saves_the_whole_run_with_its_steps() {
  let data = Data::new();
  let request = SeedRequest {
    data_dir: data.dir.path().to_path_buf(),
    repo: data.dir.path().join("repo-conversation"),
    scenario: scenarios::named("conversation").expect("a known scenario"),
    fake_codex: None,
    now: Timestamp::now(),
  };
  let seeded = kontur::seed(&request).expect("the scenario seeds");
  let launch = data.launch();
  let (mut window, stand_ins) = Window::launched_with_stand_ins(data, launch);
  let key = key_of(&seeded, TITLE);
  open(&mut window, &seeded, &key, "conversation-chat");
  open_menu(&mut window);
  press(&mut window, "task-export-conversation");
  window.until("the dialog", |window| {
    window.tree.get_element_by_id("task-export-dialog").is_some()
  });
  let what = window.text("task-export-dialog-what");
  assert!(what.starts_with(&format!("{key} · {TITLE} · run ")), "{what}");
  assert!(what.contains(" steps · ") && what.ends_with(" messages"), "{what}");
  let name = window.text("task-export-dialog-file-name");
  assert!(
    name.starts_with(&format!("{}-run-", key.to_string().to_lowercase())),
    "{name}"
  );
  assert!(name.ends_with("-conversation.md"), "{name}");
  press(&mut window, "task-export-dialog-with-tools");
  let path = window.data.dir.path().join("run.md");
  stand_ins.save_at(Some(path.clone()));
  press(&mut window, "task-export-dialog-save");
  window.until("the file", |window| {
    path.exists() && window.tree.get_element_by_id("task-export-dialog").is_none()
  });
  let markdown = std::fs::read_to_string(&path).expect("the file");
  assert!(markdown.starts_with(&format!("# {key} · {TITLE}\n")), "{markdown}");
  assert!(markdown.contains("\n## "), "a heading for each step: {markdown}");
  assert!(markdown.contains("\n- Command"), "with its commands: {markdown}");
  assert_eq!(stand_ins.asked_to_save.lock()[0].file_name, name);
}

#[test]
fn conversation_export_copies_the_messages_from_the_menu() {
  let (mut window, seeded) = seeded("conversation");
  let key = key_of(&seeded, TITLE);
  let copied = own_clipboard(&mut window, true);
  open(&mut window, &seeded, &key, "conversation-chat");
  open_menu(&mut window);
  press(&mut window, "task-copy-conversation");
  window.until("the copy", |_| !copied.lock().is_empty());
  let markdown = copied.lock()[0].clone();
  assert!(markdown.starts_with(&format!("# {key} · {TITLE}\n")), "{markdown}");
  assert!(markdown.contains("\n## "), "{markdown}");
  assert!(!markdown.contains("\n- Command"), "messages only: {markdown}");
  assert!(window.tree.get_element_by_id("task-export-dialog").is_none());
}

#[test]
fn conversation_export_is_not_offered_on_the_summary() {
  let (mut window, seeded) = seeded("conversation");
  let key = key_of(&seeded, TITLE);
  open(&mut window, &seeded, &key, "conversation-chat");
  window.navigation().go(Route::Project(
    seeded.project,
    ProjectPage::Task(key.clone(), TaskView::Summary),
  ));
  window.until("the Summary", |window| {
    window.tree.get_element_by_id("task-more").is_some()
  });
  press(&mut window, "task-more");
  window.until("the menu", |window| {
    window.tree.get_element_by_id("task-copy-link").is_some()
  });
  assert!(window.tree.get_element_by_id("task-export-conversation").is_none());
}
