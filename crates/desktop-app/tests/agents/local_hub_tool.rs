//! A local project's command tool is edited in its panel with the form that makes one (A29 `FrraS` as a local project
//! has it): *Save* publishes the fields as the next version at once, a change that widens what agents may run asks
//! first and only a person's press allows it, and *Cancel* puts the published version back.
//! `cargo test -p orchester-app --test agents local_hub_tool`.

use orchester_app::routes::{HubFilter, HubView};
use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{Tool, VersionNumber},
  store::ResourceStore,
};
use orchester_store::ProjectDb;
use orchester_workspace::{
  connections::mcp::Allowed,
  tools::{
    editor::{self, Edit},
    form::CommandText,
  },
};

use super::{harness::Agents, local_hub::open};

/// A command tool made and published as v1, which runs the test binary itself.
fn command(agents: &Agents) -> ResourceId {
  let text = CommandText {
    program: std::env::current_exe()
      .expect("test binary")
      .to_string_lossy()
      .into_owned(),
    ..CommandText::new_tool()
  };
  editor::apply_in(
    &agents.root,
    Edit::Save {
      name: Name::new("checks").expect("name"),
      tool: Box::new(text.tool(None).expect("command")),
      before: None,
    },
    &Allowed::Anything,
  )
  .expect("created")
}

fn latest(agents: &Agents, id: ResourceId) -> (VersionNumber, CommandText) {
  let db = ProjectDb::open(&agents.root).expect("database");
  let version = db.latest_version::<Tool>(id).expect("read").expect("a version");
  (version.number(), CommandText::of(version.body()).expect("command"))
}

fn show(agents: &mut Agents, id: ResourceId) {
  open(agents, HubView::new(HubFilter::Tools, Some(id)));
  agents.until("the tool's form", |s| s.exists("local-hub-tool-program-input"));
}

#[test]
fn local_hub_tool_save_publishes_the_fields_as_the_next_version() {
  let mut agents = Agents::new(true);
  let id = command(&agents);
  show(&mut agents, id);
  assert!(agents.exists("local-hub-tool-used-by"), "Used by stays in the panel");
  assert_eq!(
    agents.value("local-hub-tool-timeout-input"),
    latest(&agents, id).1.timeout
  );

  agents.set("local-hub-tool-timeout-input", "60");
  agents.click("local-hub-tool-save");
  agents.until("published as v2", |s| latest(s, id).0.get() == 2);
  assert_eq!(latest(&agents, id).1.timeout, "60");
  let db = ProjectDb::open(&agents.root).expect("database");
  assert!(db.draft::<Tool>(id).expect("read").is_none(), "Save leaves no draft");
}

#[test]
fn local_hub_tool_a_change_that_widens_asks_first_and_cancel_puts_the_version_back() {
  let mut agents = Agents::new(true);
  let id = command(&agents);
  show(&mut agents, id);

  agents.set("local-hub-tool-arguments-input", "--other");
  agents.click("local-hub-tool-save");
  agents.until("the question", |s| s.exists("local-hub-tool-confirm-confirm"));
  assert_eq!(
    latest(&agents, id).0,
    VersionNumber::FIRST,
    "nothing saved before the answer"
  );

  agents.click("local-hub-tool-cancel");
  agents.until("the question goes", |s| !s.exists("local-hub-tool-confirm-confirm"));
  assert_eq!(agents.value("local-hub-tool-arguments-input"), "");

  agents.set("local-hub-tool-arguments-input", "--other");
  agents.click("local-hub-tool-save");
  agents.until("the question again", |s| s.exists("local-hub-tool-confirm-confirm"));
  // Its confirm takes no press before the question has been drawn once.
  agents.press_until("local-hub-tool-confirm-confirm", "published as v2", |s| {
    latest(s, id).0.get() == 2
  });
  assert_eq!(latest(&agents, id).1.arguments, "--other");
}
