//! The Agents hub of a local project keeps what it shows while the read that follows a change is under way: a made
//! resource's form stays as it was sent, Create pending, until the list holds it; a restored entry's *Restore* stays
//! pending until it leaves *Archived*; a new memory record's editor stays, Save pending, until the memory is read
//! again. Nothing flashes the empty tab, the bones of a first read or another entry's panel. `cargo test -p
//! orchester-app --test agents local_hub_keep`.

use orchester_app::{
  project::hold::ReadHold,
  routes::{HubFilter, HubView, MemoryTab, MemoryView, ProjectPage, Route},
  ui::icon::Icon,
};
use orchester_core::{resource::ResourceKind, time::Timestamp};
use orchester_store::ProjectDb;
use orchester_workspace::memory::read::ProjectMemory;

use super::{
  harness::Agents,
  local_hub::open,
  local_hub_create::{held, show_form},
  support::project_id,
};

/// The window's hold of the hub's reads: held, the hub's read that follows a change waits.
fn reads(agents: &Agents) -> ReadHold {
  agents.window.launch.services().expect("services").hold.clone()
}

/// Whether the button `id` shows its loader: what it started is under way.
fn loading(agents: &mut Agents, id: &str) -> bool {
  let loader = Icon::LoaderCircle.glyph().to_string();
  agents.window.tree.get_element_by_id(id).is_some_and(|button| {
    let mut glyphs = Vec::new();
    collect(button, &mut glyphs);
    glyphs.contains(&loader)
  })
}

fn collect(element: lurq::node::element::ElementRef<'_>, into: &mut Vec<String>) {
  into.extend(element.text_content().map(str::to_owned));
  for child in element.children() {
    collect(child, into);
  }
}

/// What the hub must never show once it has shown a list: the empty tab or the bones of a first read.
fn kept(agents: &mut Agents, what: &str) {
  assert!(!agents.exists("local-hub-empty"), "{what}: the empty tab flashed");
  assert!(
    !agents.exists("local-hub-loading"),
    "{what}: the first read's bones flashed"
  );
  assert!(agents.exists("local-hub-list"), "{what}: the list is gone");
}

/// Passes a while with the hub's read held, checking `still` at every pass besides what [`kept`] checks.
fn holding(agents: &mut Agents, what: &str, mut still: impl FnMut(&mut Agents)) {
  let mut passes = 0;
  agents.until(what, |s| {
    kept(s, what);
    still(s);
    passes += 1;
    passes == 40
  });
}

fn view(agents: &Agents) -> HubView {
  match agents.window.navigation().route_now() {
    Route::Project(_, ProjectPage::AgentsHub(view)) => view,
    other => panic!("the hub, not {other:?}"),
  }
}

/// A tool made in a project with no tool: until the hub reads it the form stays as it was sent with Create pending,
/// never the Tools tab's empty state; then the new tool is chosen.
#[test]
fn local_hub_keep_a_made_tool_keeps_the_form_until_the_list_holds_it() {
  let mut agents = Agents::new(true);
  show_form(&mut agents, HubFilter::Tools);
  agents.set("team-new-tool-name-input", "cargo-check");
  agents.set(
    "team-new-tool-program-input",
    &std::env::current_exe().expect("a program").to_string_lossy(),
  );
  let hold = reads(&agents);
  hold.set(true);
  agents.click("local-hub-new-create");
  agents.until("the tool is made", |s| {
    kept(s, "made");
    held(s, ResourceKind::Tool) == 1
  });
  holding(&mut agents, "the hub's read held", |s| {
    assert_eq!(s.text("team-hub-panel-title"), "New tool", "the form stays");
    assert_eq!(s.value("team-new-tool-name-input"), "cargo-check");
    assert!(loading(s, "local-hub-new-create"), "Create shows it is pending");
    assert!(view(s).adding, "the route stays on the form");
  });
  hold.set(false);
  agents.until("the new tool's panel", |s| {
    kept(s, "read");
    s.exists("team-hub-panel-title") && s.text("team-hub-panel-title") == "cargo-check"
  });
  let view = view(&agents);
  assert_eq!(view.filter, HubFilter::Tools);
  assert!(view.item.is_some() && !view.adding, "{view:?}");
}

/// A restored profile stays in *Archived*, *Restore* pending, until the hub reads it again; then it leaves.
#[test]
fn local_hub_keep_restore_stays_pending_until_the_entry_leaves_archived() {
  let mut agents = Agents::new(true);
  let builder = agents.profile(true);
  ProjectDb::open(&agents.root)
    .expect("database")
    .archive_local_resource(builder, Timestamp::now())
    .expect("archived");
  open(&mut agents, HubView::new(HubFilter::Archived, Some(builder)));
  agents.until("its panel", |s| s.exists("archive-restore"));
  let hold = reads(&agents);
  hold.set(true);
  agents.click("archive-restore");
  let restored = |s: &Agents| {
    ProjectDb::open(&s.root)
      .expect("database")
      .archived_local_resources(ResourceKind::AgentProfile)
      .expect("archived")
      .is_empty()
  };
  agents.until("restored", |s| restored(s));
  holding(&mut agents, "the hub's read held", |s| {
    assert!(loading(s, "archive-restore"), "Restore shows it is pending");
    assert_eq!(view(s), HubView::new(HubFilter::Archived, Some(builder)));
  });
  hold.set(false);
  agents.until("it leaves Archived", |s| !s.exists(&format!("local-hub-row-{builder}")));
}

/// The project's memory as it is stored.
fn memory(agents: &Agents) -> ProjectMemory {
  ProjectMemory::load_in(&agents.root).expect("memory")
}

/// The first record of an empty *Active*: until the hub's memory is read again the editor stays as it was sent, Save
/// pending, never the status's empty state; then the record is listed.
#[test]
fn local_hub_keep_a_new_memory_record_keeps_the_editor_until_the_list_holds_it() {
  let mut agents = Agents::new(true);
  agents.window.navigation().go(Route::Project(
    project_id(),
    ProjectPage::Memory(MemoryView::on(MemoryTab::Active)),
  ));
  agents.until("the empty Active", |s| s.exists("memory-empty"));
  agents.click("memory-new");
  agents.until("the editor", |s| s.exists("memory-editor-new-text"));
  agents.set("memory-editor-new-text", "Release notes live in CHANGELOG.md.");
  let hold = reads(&agents);
  hold.set(true);
  agents.click("memory-editor-new-save");
  agents.until("the record is stored", |s| !memory(s).records.is_empty());
  let mut passes = 0;
  agents.until("the memory's read held", |s| {
    assert!(!s.exists("memory-empty"), "the empty state flashed");
    assert!(!s.exists("memory-loading"), "the first read's bones flashed");
    assert_eq!(s.value("memory-editor-new-text"), "Release notes live in CHANGELOG.md.");
    assert!(loading(s, "memory-editor-new-save"), "Save shows it is pending");
    passes += 1;
    passes == 40
  });
  hold.set(false);
  agents.until("the record listed", |s| {
    assert!(!s.exists("memory-empty"), "the empty state flashed");
    !s.exists("memory-editor-new") && s.exists("memory-list")
  });
}

fn left(agents: &mut Agents, id: &str) -> f32 {
  agents
    .window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"))
    .x
}

/// While the Memory filter reads the memory the first time, its statuses stand as they stay once it is read with
/// nothing waiting: *Active* first, where it stays.
#[test]
fn local_hub_keep_memory_statuses_do_not_move_when_the_memory_is_read() {
  let mut agents = Agents::new(true);
  open(&mut agents, HubView::default());
  let hold = reads(&agents);
  hold.set(true);
  agents
    .window
    .navigation()
    .go(Route::Project(project_id(), ProjectPage::Memory(MemoryView::default())));
  agents.until("the memory read", |s| s.exists("memory-loading"));
  let reading = left(&mut agents, "memory-tab-active");
  assert!(
    reading < left(&mut agents, "memory-tab-waiting"),
    "Active comes first while the memory is read"
  );
  hold.set(false);
  agents.until("the memory read", |s| !s.exists("memory-loading"));
  let read = left(&mut agents, "memory-tab-active");
  assert!(
    (read - reading).abs() < 0.5,
    "Active stays where it stood: {read} vs {reading}"
  );
}
