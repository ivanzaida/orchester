//! Real file storage and checked writes, with the native chooser replaced by a private selection.
use std::{fs, path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  node::Element,
};
use orchester_app::{
  mcp::{McpKind, McpLink, McpSettings, McpStatus},
  root::{Root, RootProps},
  routes::{ProjectPage, Route},
  services::reference_picker::ReferencePicker,
};
use orchester_core::resource::{InstructionScope, Skill};
use orchester_workspace::skills::{data::ProjectSkills, editor};

use super::{Skills, support::project_id};

#[derive(Clone)]
struct HostProps {
  root: RootProps,
  picker: ReferencePicker,
}
impl PartialEq for HostProps {
  fn eq(&self, other: &Self) -> bool {
    self.root == other.root
  }
}
impl DevtoolsInspectable for HostProps {}
struct PickerHost;
impl Component for PickerHost {
  type Props = HostProps;
  fn create(_: &mut Ctx) -> Self {
    Self
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let p = ctx.props::<HostProps>().clone();
    ctx.provide(p.picker);
    ctx.mount_keyed::<Root>("app", p.root)
  }
}
fn with_picker(picker: ReferencePicker) -> Skills {
  let mut s = Skills::new();
  let props = HostProps {
    root: RootProps {
      link: s.window.link.clone(),
      mcp: McpLink::without_server(McpStatus::unavailable(McpKind::Installation, McpSettings::of_build())),
      start: s.window.launch.start(),
    },
    picker,
  };
  s.window.tree.mount_root::<PickerHost>(&mut s.window.app, props);
  s.window.pass();
  s
}
fn selected() -> (Skills, PathBuf) {
  let path = Arc::new(parking_lot::Mutex::new(PathBuf::new()));
  let chosen = path.clone();
  let picker = ReferencePicker::new(move |_| {
    let path = chosen.lock().clone();
    Box::pin(async move { Some(path) })
  });
  let s = with_picker(picker);
  let file = s.repo.parent().expect("fixture").join("private-reference.md");
  fs::write(&file, "Private selected reference").expect("file");
  *path.lock() = file.clone();
  (s, file)
}
fn add(s: &mut Skills) {
  s.click("skill-add-file");
  s.window.until("reference added", |w| {
    w.texts("skill-files")
      .is_some_and(|t| t.join(" ").contains("private-reference.md"))
  });
}
fn value(s: &mut Skills, id: &str) -> String {
  s.window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|e| e.as_text_input())
    .expect("editable input")
    .value()
}
fn leave(s: &mut Skills) {
  s.window
    .navigation()
    .go(Route::Project(project_id(), ProjectPage::Skills(Default::default())));
  s.window.pass();
}
#[test]
fn skills_reference_only_new_form_keeps_on_cancel_and_leaves_only_after_confirmation() {
  let (mut s, file) = selected();
  s.go(ProjectPage::NewSkill);
  add(&mut s);
  assert_eq!(value(&mut s, "skill-name"), "");
  leave(&mut s);
  assert!(s.text("skill-leave-question").contains("unsaved"));
  s.click("skill-leave-keep");
  assert!(s.text("skill-files").contains("private-reference.md"));
  assert_eq!(value(&mut s, "skill-name"), "");
  leave(&mut s);
  s.click("skill-leave-confirm");
  s.window
    .until("left", |w| w.tree.get_element_by_id("skills-empty").is_some());
  assert!(ProjectSkills::load_in(&s.repo).expect("skills").entries.is_empty());
  assert_eq!(fs::read_to_string(file).expect("source"), "Private selected reference");
  s.go(ProjectPage::NewSkill);
  assert!(!s.text("skill-files").contains("private-reference.md"));
}
#[test]
fn skills_create_withheld_real_result_keeps_reference_removal_inert() {
  let (mut s, _) = selected();
  s.go(ProjectPage::NewSkill);
  add(&mut s);
  s.set("skill-name", "checks");
  s.click("skill-create");
  // No tick_futures: the real write result cannot reach the route completion yet.
  s.click("skill-remove-file-0");
  assert!(matches!(
    s.window.navigation().route_now(),
    Route::Project(_, ProjectPage::NewSkill)
  ));
  assert!(s.text("skill-files").contains("private-reference.md"));
  s.window.until("created", |w| {
    matches!(w.navigation().route_now(), Route::Project(_, ProjectPage::Skill(_)))
  });
  s.window.until("created reference visible", |w| {
    w.texts("skill-files")
      .is_some_and(|t| t.join(" ").contains("private-reference.md"))
  });
  let listed = ProjectSkills::load_in(&s.repo).expect("skills");
  assert_eq!(listed.entries.len(), 1);
  assert_eq!(listed.entries[0].body.files().len(), 1);
  assert!(listed.entries[0].state.published().is_none());
}
fn conflict(s: &mut Skills) -> orchester_core::id::ResourceId {
  let id = s.seed("checks", false);
  s.go(ProjectPage::Skill(id));
  add(s);
  let root = s.repo.clone();
  s.window.until("reference saved", |_| {
    ProjectSkills::load_in(&root)
      .expect("skills")
      .entry(id)
      .is_some_and(|e| e.body.files().len() == 1)
  });
  s.settle(id, "Run checks before handing over.");
  let before = s.entry(id);
  let body = Skill::new(
    "External draft".into(),
    "# External body".into(),
    before.body.files().to_vec(),
    InstructionScope::Project,
  )
  .expect("body");
  editor::save_in(&s.repo, Some(&before), "checks", &body).expect("external edit");
  s.set("skill-body", "# My unsaved body");
  s.window
    .until("conflict", |w| w.tree.get_element_by_id("skill-save-error").is_some());
  s.set("skill-name", "");
  id
}
#[test]
fn skills_failed_reload_preserves_raw_fields_errors_and_leave_guard_until_confirmed_success() {
  let (mut s, _) = selected();
  let id = conflict(&mut s);
  let db = rusqlite::Connection::open(s.repo.join(".orchester/orchester.db")).expect("fixture db");
  let saved: String = db
    .query_row("SELECT body FROM resource_drafts", [], |row| row.get(0))
    .expect("draft");
  db.execute("UPDATE resource_drafts SET body = 'broken'", [])
    .expect("reject read");
  s.click("skill-reload");
  assert_eq!(value(&mut s, "skill-name"), "");
  assert_eq!(value(&mut s, "skill-body"), "# My unsaved body");
  s.click("skill-reload-confirm");
  s.window.until("reload rejected", |w| {
    w.tree.get_element_by_id("skills-refresh-error").is_some()
  });
  assert_eq!(value(&mut s, "skill-name"), "");
  assert_eq!(value(&mut s, "skill-body"), "# My unsaved body");
  assert!(s.text("skill-files").contains("private-reference.md"));
  assert!(s.window.tree.get_element_by_id("skill-name-note").is_some());
  assert!(s.window.tree.get_element_by_id("skill-save-error").is_some());
  assert_eq!(
    db.query_row("SELECT body FROM resource_drafts", [], |r| r.get::<_, String>(0))
      .expect("draft"),
    "broken"
  );
  leave(&mut s);
  assert!(s.text("skill-leave-question").contains("unsaved"));
  s.click("skill-leave-keep");
  db.execute("UPDATE resource_drafts SET body = ?1", [&saved])
    .expect("repair fixture");
  s.set("skill-body", "# Edited after rejected reload");
  seed_refresh_marker(&s);
  let feed = s.window.projects().feed(project_id());
  feed.outside.update(|counts| counts.pipelines += 1);
  s.click("skill-scope");
  s.window
    .until("successful passive read includes the new stored pipeline", |w| {
      w.texts("skill-scope-menu")
        .is_some_and(|t| t.join(" ").contains("Passive refresh marker"))
    });
  assert!(s.window.tree.get_element_by_id("skills-refresh-error").is_none());
  s.click("skill-scope");
  assert_eq!(value(&mut s, "skill-name"), "");
  assert_eq!(value(&mut s, "skill-body"), "# Edited after rejected reload");
  assert!(s.text("skill-files").contains("private-reference.md"));
  assert!(s.window.tree.get_element_by_id("skill-save-error").is_some());
  assert!(s.window.tree.get_element_by_id("skill-name-note").is_some());
  assert_eq!(s.entry(id).body.body(), "# External body");
  leave(&mut s);
  assert!(s.text("skill-leave-question").contains("unsaved"));
  s.click("skill-leave-keep");
  s.click("skill-reload");
  assert!(s.window.tree.get_element_by_id("skill-reload-question").is_some());
  s.click("skill-reload-keep");
  assert_eq!(value(&mut s, "skill-body"), "# Edited after rejected reload");
  s.click("skill-reload");
  s.click("skill-reload-confirm");
  s.window.until("accepted reload", |w| {
    w.tree
      .get_element_by_id_mut("skill-body")
      .and_then(|e| e.as_text_input())
      .is_some_and(|i| i.value() == "# External body")
  });
  assert_eq!(value(&mut s, "skill-name"), "checks");
  assert!(s.window.tree.get_element_by_id("skill-save-error").is_none());
  assert_eq!(s.entry(id).body.body(), "# External body");
  assert!(s.entry(id).state.published().is_none());
}
#[test]
fn skills_pending_reload_locks_open_scope_and_reference_controls_until_accepted_read() {
  let (mut s, _) = selected();
  let id = conflict(&mut s);
  s.click("skill-scope");
  assert!(s.window.tree.get_element_by_id("skill-scope-menu").is_some());
  let hold = s.window.launch.services().expect("services").hold.clone();
  hold.set(true);
  s.click("skill-reload");
  s.click("skill-reload-confirm");
  assert!(s.window.tree.get_element_by_id("skill-scope-menu").is_none());
  assert!(s.window.tree.get_element_by_id("skill-reloading").is_some());
  for (field, expected) in [("skill-name", ""), ("skill-body", "# My unsaved body")] {
    let b = s
      .window
      .tree
      .get_element_by_id_mut(field)
      .and_then(|e| e.bounds())
      .expect("field bounds");
    let (x, y) = (b.x + b.width / 2.0, b.y + b.height / 2.0);
    s.window.tree.mouse_move(x, y);
    s.window.tree.mouse_down(x, y, lurq::app::events::MouseButton::Left);
    s.window.tree.mouse_up(x, y, lurq::app::events::MouseButton::Left);
    s.window.tree.key_down("x".into(), "KeyX".into(), false, false, false);
    s.window.pass();
    assert_eq!(value(&mut s, field), expected, "pending reload refuses typing");
  }
  s.click("skill-scope");
  s.click("skill-remove-file-0");
  s.click("skill-add-file");
  s.click("skill-publish");
  assert!(s.window.tree.get_element_by_id("skill-scope-menu").is_none());
  assert!(s.text("skill-files").contains("private-reference.md"));
  assert!(s.window.tree.get_element_by_id("skill-save-error").is_some());
  assert_eq!(s.entry(id).body.body(), "# External body");
  hold.set(false);
  s.window.until("reload accepted", |w| {
    w.tree
      .get_element_by_id_mut("skill-body")
      .and_then(|e| e.as_text_input())
      .is_some_and(|i| i.value() == "# External body")
  });
  assert_eq!(s.entry(id).body.files().len(), 1);
}

#[test]
fn skills_pending_reference_picker_blocks_reload_until_its_real_result_arrives() {
  let (send, receive) = tokio::sync::oneshot::channel::<PathBuf>();
  let receive = Arc::new(parking_lot::Mutex::new(Some(receive)));
  let picker = ReferencePicker::new(move |_| {
    let receive = receive.lock().take().expect("one picker call");
    Box::pin(async move { Some(receive.await.expect("private selection")) })
  });
  let mut s = with_picker(picker);
  let id = s.seed("checks", false);
  let first = s.repo.parent().expect("fixture").join("first.md");
  fs::write(&first, "First reference").expect("file");
  let before = s.entry(id);
  let file = editor::add_file_in(&s.repo, &first, &[]).expect("reference");
  let body = Skill::new(
    before.body.description().into(),
    before.body.body().into(),
    vec![file],
    InstructionScope::Project,
  )
  .expect("body");
  let before = editor::save_in(&s.repo, Some(&before), "checks", &body).expect("first reference");
  s.go(ProjectPage::Skill(id));
  let changed = Skill::new(
    "External".into(),
    "# External body".into(),
    before.body.files().to_vec(),
    InstructionScope::Project,
  )
  .expect("body");
  editor::save_in(&s.repo, Some(&before), "checks", &changed).expect("competing write");
  s.set("skill-body", "# Local");
  s.window
    .until("conflict", |w| w.tree.get_element_by_id("skill-save-error").is_some());
  s.click("skill-add-file");
  s.click("skill-remove-file-0");
  s.click("skill-reload");
  assert!(s.window.tree.get_element_by_id("skill-reload-question").is_none());
  assert!(s.text("skill-files").contains("first.md"));
  let second = s.repo.parent().expect("fixture").join("second.md");
  fs::write(&second, "Second reference").expect("file");
  send.send(second).expect("deliver actual selected path");
  s.window.until("second reference added", |w| {
    w.texts("skill-files")
      .is_some_and(|t| t.join(" ").contains("second.md"))
  });
  assert!(s.text("skill-files").contains("first.md"));
  s.click("skill-reload");
  s.click("skill-reload-keep");
  assert!(s.text("skill-files").contains("second.md"));
  assert_eq!(
    s.entry(id).body,
    changed,
    "no resource write while the failed candidate remains local"
  );
}

#[test]
fn skills_failed_initial_create_retry_waits_for_actual_reference_selection() {
  let (send, receive) = tokio::sync::oneshot::channel::<PathBuf>();
  let receive = Arc::new(parking_lot::Mutex::new(Some(receive)));
  let picker = ReferencePicker::new(move |_| {
    let receive = receive.lock().take().expect("one picker call");
    Box::pin(async move { Some(receive.await.expect("private selection")) })
  });
  let mut s = with_picker(picker);
  s.go(ProjectPage::NewSkill);
  s.set("skill-name", "checks");
  s.set("skill-description", "Chosen reference");
  s.set("skill-body", "# Keep chosen reference");
  // The form validated before a real competing write took its name.
  let taken = s.seed("checks", false);
  s.click("skill-create");
  s.window.until("create failed", |w| {
    w.tree.get_element_by_id("skill-save-error").is_some()
  });
  assert!(matches!(
    s.window.navigation().route_now(),
    Route::Project(_, ProjectPage::NewSkill)
  ));
  s.set("skill-name", "selected-checks");
  s.click("skill-add-file");
  s.click("skill-retry");
  for _ in 0..8 {
    s.window.tree.tick_futures();
    s.window.pass();
  }
  assert!(matches!(
    s.window.navigation().route_now(),
    Route::Project(_, ProjectPage::NewSkill)
  ));
  assert_eq!(ProjectSkills::load_in(&s.repo).expect("skills").entries.len(), 1);
  let selected = s.repo.parent().expect("fixture").join("withheld.md");
  fs::write(&selected, "Exact withheld selection").expect("private file");
  send.send(selected.clone()).expect("deliver actual selection");
  s.window.until("selection arrives", |w| {
    w.texts("skill-files")
      .is_some_and(|t| t.join(" ").contains("withheld.md"))
  });
  s.click("skill-retry");
  s.window.until(
    "created by retry",
    |w| matches!(w.navigation().route_now(), Route::Project(_, ProjectPage::Skill(id)) if id != taken),
  );
  let listed = ProjectSkills::load_in(&s.repo).expect("skills");
  let created = listed.entries.iter().find(|e| e.id != taken).expect("created");
  assert_eq!(created.name.as_ref(), "selected-checks");
  assert_eq!(created.body.files().len(), 1);
  assert_eq!(created.body.files()[0].path.as_str(), "withheld.md");
  use orchester_core::store::BlobStore;
  let db = orchester_store::ProjectDb::open(&s.repo).expect("db");
  assert_eq!(
    db.blob(&created.body.files()[0].content).expect("blob"),
    b"Exact withheld selection"
  );
  assert_eq!(
    fs::read_to_string(selected).expect("source"),
    "Exact withheld selection"
  );
}

fn seed_refresh_marker(s: &Skills) {
  use orchester_core::{
    name::Name,
    resource::{Pipeline, Slug},
    store::ResourceStore,
    time::Timestamp,
  };
  let mut db = orchester_store::ProjectDb::open(&s.repo).expect("db");
  db.create_resource(
    Slug::new("passive-refresh").expect("slug"),
    Name::new("Passive refresh marker").expect("name"),
    &Pipeline::new(Vec::new()).expect("draft"),
    Timestamp::now(),
  )
  .expect("stored marker observable only after a successful read");
}
