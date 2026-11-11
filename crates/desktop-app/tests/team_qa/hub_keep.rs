//! The Agents hub of a team project keeps what it shows while a change is sent to the team server and while the
//! team's configuration is read again after it: its list, the chosen resource, the open form with its fields and the
//! filter. Nothing flashes the empty tab, the bones of a first read or a panel of another resource; the control that
//! sent the change shows it pending.
//!
//! `cargo test -p orchester-app --test team_qa team_qa_hub_keep`

use std::time::Duration;

use orchester_api::test_support::team_server::TeamServer;
use orchester_app::{
  project::hold::ReadHold,
  routes::{HubFilter, HubView, ProjectPage, Route},
  test_support::{Window, project_id},
  ui::icon::Icon,
};
use orchester_core::{id::ResourceId, resource::ResourceKind, store::ResourceStore};
use serde_json::json;

use super::{
  hub_edit::{db, opened, says_unreachable},
  prepared,
  team::State,
};

/// How long the stand-in takes to answer a write: long enough for many passes of the window in between.
const SLOW: Duration = Duration::from_millis(400);

/// What the hub must never show once it has shown a list: the empty tab, the bones of a first read, or a page that
/// is not the hub's list and panel.
fn kept(window: &mut Window, what: &str) {
  assert!(
    window.texts("team-hub-empty").is_none(),
    "{what}: the empty tab flashed"
  );
  assert!(
    window.texts("team-hub-loading").is_none(),
    "{what}: the first read's bones flashed"
  );
  assert!(window.texts("team-hub-list").is_some(), "{what}: the list is gone");
}

/// Passes until `done` holds, checking at every pass that the hub keeps its list and shows no empty tab or bones.
fn keeping_until(window: &mut Window, what: &str, mut done: impl FnMut(&mut Window) -> bool) {
  window.until_within(Duration::from_secs(20), what, |window| {
    kept(window, what);
    done(window)
  });
}

/// Passes a while with nothing to wait for, checking `still` at every pass besides what [`kept`] checks.
fn holding(window: &mut Window, what: &str, mut still: impl FnMut(&mut Window)) {
  let mut passes = 0;
  window.until(what, |window| {
    kept(window, what);
    still(window);
    passes += 1;
    passes == 40
  });
}

/// The route's view of the hub.
fn view(window: &Window) -> HubView {
  match window.navigation().route_now() {
    Route::Project(_, ProjectPage::AgentsHub(view)) => view,
    other => panic!("the hub, not {other:?}"),
  }
}

/// The window's hold of the hub's reads: held, the hub's read that follows a change waits.
fn reads(window: &Window) -> ReadHold {
  window.launch.services().expect("the session started").hold.clone()
}

/// Whether the button `id` shows its loader: what it sent is under way.
fn loading(window: &mut Window, id: &str) -> bool {
  let loader = Icon::LoaderCircle.glyph().to_string();
  window.tree.get_element_by_id(id).is_some_and(|button| {
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

/// What the text input `id` holds.
fn value(window: &mut Window, id: &str) -> String {
  window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.as_text_input())
    .unwrap_or_else(|| panic!("{id} is a text input"))
    .value()
}

/// The versions the stand-in holds of `id`.
fn versions(server: &TeamServer, id: ResourceId) -> usize {
  server
    .resources("kontur")
    .into_iter()
    .find(|(resource, _)| resource["id"] == id.to_string())
    .map_or(0, |(_, versions)| versions.len())
}

/// The tool form of the admin's hub, filled, with the team server answering writes only after [`SLOW`].
fn tool_form() -> (Window, TeamServer) {
  let (mut window, server) = prepared(State::Admin);
  window.navigation().go(Route::Project(
    project_id(),
    ProjectPage::hub(HubView::adding(HubFilter::Tools)),
  ));
  window.until("the form", |window| window.texts("team-hub-new-create").is_some());
  let program = std::env::current_exe()
    .expect("a program")
    .to_string_lossy()
    .into_owned();
  window.set("team-new-tool-name-input", "cargo-check");
  window.set("team-new-tool-program-input", &program);
  server.slow_resource_writes(SLOW);
  (window, server)
}

/// Whether the stand-in holds the tool the form made.
fn tool_made(server: &TeamServer) -> bool {
  server
    .resources("kontur")
    .iter()
    .any(|(resource, versions)| resource["name"] == "cargo-check" && versions.len() == 1)
}

/// The form as it was sent: its title, its name and Create pending.
fn form_as_sent(window: &mut Window) {
  assert_eq!(window.text("team-hub-panel-title"), "New tool", "the form stays");
  assert_eq!(value(window, "team-new-tool-name-input"), "cargo-check");
  assert!(loading(window, "team-hub-new-create"), "Create shows it is pending");
}

#[test]
fn team_qa_hub_keep_creating_a_tool_keeps_the_form_until_the_list_holds_it() {
  let (mut window, server) = tool_form();
  window.press("team-hub-new-create");
  // While the server answers and the hub is read again, the form stays with its fields, Create pending.
  keeping_until(&mut window, "the server to take it", |_| tool_made(&server));
  keeping_until(&mut window, "the new tool's panel", |window| {
    let shown = window
      .texts("team-hub-panel-title")
      .is_some_and(|title| title == ["cargo-check"]);
    if !shown {
      // The form stays until the list holds the new tool: the panel never shows another one, or none.
      assert_eq!(
        window.texts("team-hub-panel-title"),
        Some(vec!["New tool".to_owned()]),
        "the form gave way before the list held the new tool"
      );
    }
    shown
  });
  let view = view(&window);
  assert_eq!(view.filter, HubFilter::Tools);
  assert!(view.item.is_some() && !view.adding, "{view:?}");
  assert!(window.texts("team-hub-row-new").is_none());
}

/// The server has taken the new tool and the hub's read again has not answered yet: the form is what shows, as sent.
#[test]
fn team_qa_hub_keep_a_created_tool_waits_for_the_hubs_read_with_create_pending() {
  let (mut window, server) = tool_form();
  let hold = reads(&window);
  hold.set(true);
  window.press("team-hub-new-create");
  keeping_until(&mut window, "the server to take it", |window| {
    form_as_sent(window);
    tool_made(&server)
  });
  holding(&mut window, "the hub's read held", form_as_sent);
  assert!(view(&window).adding, "the route stays on the form");
  hold.set(false);
  keeping_until(&mut window, "the new tool's panel", |window| {
    window
      .texts("team-hub-panel-title")
      .is_some_and(|title| title == ["cargo-check"])
  });
  let made = view(&window).item.expect("the new tool is chosen");
  assert_eq!(window.text(&format!("team-hub-row-{made}-name")), "cargo-check");
}

/// The window on `name` of `kind` under `filter` in the `team-edit` state, once its panel's `shown` is there.
fn editing(filter: HubFilter, kind: ResourceKind, name: &'static str, shown: &str) -> (Window, TeamServer, ResourceId) {
  let (window, server, (_, picked)) = opened(("team-edit", State::TeamEdit), shown, move |window| {
    let id = db(window)
      .resources(kind)
      .expect("resources")
      .into_iter()
      .find(|resource| resource.name().as_str() == name)
      .expect("the seed's resource")
      .id();
    (HubView::new(filter, Some(id)), Some(id))
  });
  (window, server, picked.expect("the resource"))
}

/// Saves what `field` holds as `typed` with the server slow and the hub's read held, checking at every pass that the
/// panel stays on `name` with what was typed; then lets the read go and waits for the panel to read v2.
fn saves_in_place(
  window: &mut Window,
  (server, id): (&TeamServer, ResourceId),
  (name, field, typed): (&str, &str, &str),
) {
  window.set(field, typed);
  server.slow_resource_writes(SLOW);
  let hold = reads(window);
  hold.set(true);
  window.press("team-hub-edit-save");
  let still = |window: &mut Window| {
    assert_eq!(
      window.text("team-hub-panel-title"),
      name,
      "the panel stays on what was saved"
    );
    assert_eq!(value(window, field), typed, "what was typed stays");
    assert_eq!(view(window).item, Some(id), "the route stays on it");
  };
  assert!(loading(window, "team-hub-edit-save"), "Save shows it is pending");
  keeping_until(window, "the server to number it", |window| {
    still(window);
    versions(server, id) == 2
  });
  holding(window, "the hub's read held", still);
  hold.set(false);
  keeping_until(window, "the team's v2", |window| {
    still(window);
    window.text("team-hub-chip-team") == "Team · v2"
  });
}

#[test]
fn team_qa_hub_keep_saving_a_tool_keeps_its_panel_and_the_list() {
  let (mut window, server, id) = editing(
    HubFilter::Tools,
    ResourceKind::Tool,
    "cargo-check",
    "team-hub-from-team-arguments-input",
  );
  saves_in_place(
    &mut window,
    (&server, id),
    (
      "cargo-check",
      "team-hub-from-team-arguments-input",
      "check\n--workspace",
    ),
  );
  assert_eq!(view(&window).filter, HubFilter::Tools);
}

#[test]
fn team_qa_hub_keep_saving_a_profile_keeps_its_panel_and_the_list() {
  let (mut window, server, id) = editing(
    HubFilter::Profiles,
    ResourceKind::AgentProfile,
    "Reviewer",
    "team-hub-role-instructions-input",
  );
  saves_in_place(
    &mut window,
    (&server, id),
    (
      "Reviewer",
      "team-hub-role-instructions-input",
      "You are the Reviewer. Read the task, then the diff.",
    ),
  );
  assert_eq!(view(&window).filter, HubFilter::Profiles);
}

/// Another admin publishes a version of the chosen tool while this admin types in it, and the stream reads the team's
/// configuration again (the stand-in sends no events of its own: its stream drops and opens again, which reads it
/// whole): the hub reads again, and the panel stays on the tool, with what was typed, and the list says v2.
#[test]
fn team_qa_hub_keep_another_admins_change_keeps_the_chosen_resource_and_what_was_typed() {
  const TYPED: &str = "check\n--all-targets";
  let (mut window, server, id) = editing(
    HubFilter::Tools,
    ResourceKind::Tool,
    "cargo-check",
    "team-hub-from-team-arguments-input",
  );
  let field = "team-hub-from-team-arguments-input";
  window.set(field, TYPED);
  let mut newer = server
    .resources("kontur")
    .into_iter()
    .find(|(resource, _)| resource["id"] == id.to_string())
    .map(|(_, versions)| versions[0]["body"].clone())
    .expect("the stand-in holds the tool");
  newer["action"]["command"]["args"] = json!(["check", "--release"]);
  server.publish_as("kontur", id, newer, "olha");
  let still = |window: &mut Window| {
    assert_eq!(window.text("team-hub-panel-title"), "cargo-check");
    assert_eq!(value(window, field), TYPED);
    assert_eq!(view(window), HubView::new(HubFilter::Tools, Some(id)));
  };
  server.set_down(true);
  keeping_until(&mut window, "the stream to drop", |window| {
    still(window);
    says_unreachable(window)
  });
  server.set_down(false);
  let row = format!("team-hub-row-{id}-version");
  keeping_until(&mut window, "the other admin's version in the list", |window| {
    still(window);
    window
      .texts(&row)
      .is_some_and(|version| version.join(" ").contains('2'))
  });
}
