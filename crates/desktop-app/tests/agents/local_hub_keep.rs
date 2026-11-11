//! The Agents hub of a local project keeps what it shows while the read that follows a change is under way: a made
//! resource's form stays as it was sent, Create pending, until the list holds it; a restored entry's *Restore* stays
//! pending until it leaves *Archived*. Nothing flashes the empty tab, the bones of a first read or another entry's
//! panel. `cargo test -p orchester-app --test agents local_hub_keep`.

use orchester_app::{
  project::hold::ReadHold,
  routes::{HubFilter, HubView, ProjectPage, Route},
  ui::icon::Icon,
};
use orchester_core::{resource::ResourceKind, time::Timestamp};
use orchester_store::ProjectDb;

use super::{
  harness::Agents,
  local_hub::open,
  local_hub_create::{held, show_form},
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
