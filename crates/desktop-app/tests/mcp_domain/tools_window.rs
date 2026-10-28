//! Window automation may fill a command draft, but cannot approve creating or publishing it.
use orchester_core::{resource::Tool, store::ResourceStore};
use orchester_store::ProjectDb;
use orchester_workspace::tools::data::ProjectTools;
use serde_json::json;

use super::connections_window::controlled;
#[test]
fn tools_window_confirmation_is_person_only_for_save_and_publish() {
  let (mut s, project, repo) = controlled();
  s.ok(
    "navigate",
    &json!({"route":"tools","project":project.to_string(),"tool":"new"}),
  );
  s.until("command form", |s| s.shows("tool-name"));
  s.set_by_client("tool-name", "Window command");
  s.set_by_client(
    "tool-program",
    &std::env::current_exe().expect("test binary").to_string_lossy(),
  );
  // Create draft is pressable once the program resolved; a client's press only asks.
  s.until("protected save question", |s| {
    if !s.shows("tool-confirm-confirm") {
      let _ = s.click_by_client("tool-create");
    }
    s.shows("tool-confirm-confirm")
  });
  let refused = s
    .click_by_client("tool-confirm-confirm")
    .expect_err("client cannot confirm");
  assert_eq!(refused["code"], "person_only");
  assert!(refused["message"].as_str().expect("message").contains("Tools page"));
  let db = ProjectDb::open(&repo).expect("db");
  let listed = ProjectTools::load(&db).expect("tools");
  assert!(!listed.tools.iter().any(|e| e.name.as_ref() == "Window command"));
  s.click_by_person("tool-confirm-confirm");
  s.until("saved tool", |_| {
    ProjectTools::load(&db)
      .expect("tools")
      .tools
      .iter()
      .any(|e| e.name.as_ref() == "Window command")
  });
  s.until("saved editor", |s| {
    s.shows("tool-publish") && text(s, "tool-draft-bar-status").contains("Draft")
  });
  let id = ProjectTools::load(&db)
    .expect("tools")
    .tools
    .iter()
    .find(|e| e.name.as_ref() == "Window command")
    .expect("entry")
    .id;
  s.click_by_client("tool-publish").expect("client may ask");
  s.until("protected publish question", |s| s.shows("tool-confirm-confirm"));
  assert_eq!(
    s.click_by_client("tool-confirm-confirm")
      .expect_err("client cannot publish")["code"],
    "person_only"
  );
  assert!(db.latest_version::<Tool>(id).expect("version").is_none());
  s.click_by_person("tool-confirm-confirm");
  s.until("published version", |_| {
    db.latest_version::<Tool>(id).expect("version").is_some()
  });
}
pub(super) fn text(s: &mut super::Served, id: &str) -> String {
  fn collect(e: lurq::node::element::ElementRef<'_>, texts: &mut Vec<String>) {
    texts.extend(e.text_content().map(str::to_owned));
    for child in e.children() {
      collect(child, texts);
    }
  }
  let mut texts = Vec::new();
  if let Some(e) = s.tree.get_element_by_id(id) {
    collect(e, &mut texts);
  }
  texts.join(" ")
}
