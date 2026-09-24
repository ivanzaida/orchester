use orchester_core::{
  connection::{AgentProvider, CatalogModel, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, SignIn},
  name::Name,
  resource::ModelId,
  time::Timestamp,
};
use orchester_desktop::{
  agents::{
    data::{ProfileChange, ProjectAgents, apply_in},
    form::{ModelSelection, ProfileForm},
  },
  connections::data::{ConnectionDraft, save_in},
};

use crate::harness::{Desktop, expected};

const ROLE: &str = "agent-role";
const RESPONSIBILITY: &str = "agent-responsibility";
const INSTRUCTIONS: &str = "agent-instructions";
const STATUS: &str = "agent-save-status";
/// More than the window has stops on this screen.
const MAX_STOPS: usize = 30;

/// A project with a checked Codex connection, whose model has efforts.
fn with_connection() -> Desktop {
  let desktop = Desktop::with_project();
  let catalog = ModelCatalog::new(
    CatalogSource::Provider,
    vec![CatalogModel::new(
      ModelId::new("gpt-5.5").expect("valid model id"),
      vec!["low".into(), "high".into()],
    )],
  );
  let draft = ConnectionDraft {
    id: None,
    name: Name::new("Codex").expect("valid name"),
    provider: AgentProvider::CodexCli,
    executable: desktop.repo().join("codex.exe"),
    check: ConnectionCheck {
      at: Timestamp::now(),
      result: CheckResult::Available {
        version: "0.139.0".into(),
        sign_in: SignIn::SignedIn,
        catalog,
      },
    },
  };
  save_in(desktop.repo(), draft).expect("the connection is saved");
  desktop
}

/// A new profile in a project with a checked Codex connection.
fn new_profile() -> Desktop {
  let mut desktop = with_connection();
  desktop.wait_for("nav-agents");
  desktop.click("nav-agents");
  desktop.wait_for("empty-new-agent");
  desktop.click("empty-new-agent");
  desktop.wait_for(ROLE);
  desktop
}

fn status(desktop: &mut Desktop) -> String {
  let status = desktop.tree.get_element_by_id(STATUS).expect("save status");
  status.text_content().unwrap_or_default().to_owned()
}

#[test]
fn agent_editor_tab_follows_the_fields_and_skips_disabled_actions() {
  let mut desktop = new_profile();
  desktop.tab_to("new-agent", MAX_STOPS);
  // The new profile, selected in the list, then the form. Save draft and Publish (before New
  // agent) and Import cannot be used yet and are skipped; the connection, model and effort
  // selects are stops. After the form Tab moves on to the
  // status bar.
  assert_eq!(
    desktop.stops(8, false),
    expected(&[
      Some("agent-new"),
      Some(ROLE),
      Some(RESPONSIBILITY),
      None,
      None,
      None,
      Some(INSTRUCTIONS),
      Some("status-runner"),
    ])
  );
  assert_eq!(desktop.stops(3, true), expected(&[Some(INSTRUCTIONS), None, None]));
}

#[test]
fn agent_editor_enter_saves_the_draft_once_it_is_valid() {
  let mut desktop = new_profile();
  let unsaved = status(&mut desktop);
  desktop.tab_to(RESPONSIBILITY, MAX_STOPS);
  desktop.type_text("Implement the plan");
  // The role is still missing: Enter saves nothing.
  desktop.enter();
  desktop.settle("the unsaved profile", |tree| tree.get_element_by_id(ROLE).is_some());
  assert_eq!(status(&mut desktop), unsaved);
  assert_eq!(desktop.stops(1, true), expected(&[Some(ROLE)]));
  desktop.type_text("Builder");
  desktop.enter();
  desktop.settle("the saved draft", |tree| {
    tree
      .get_element_by_id(STATUS)
      .and_then(|status| status.text_content())
      .is_some_and(|text| text != unsaved)
  });
}

/// Stores a profile for `role` on the project's connection and returns its list item id.
fn stored_profile(desktop: &Desktop, role: &str) -> String {
  let agents = ProjectAgents::load_in(desktop.repo()).expect("the project's agents load");
  let model = ModelSelection::Listed(ModelId::new("gpt-5.5").expect("valid model id"));
  let profile = ProfileForm {
    role,
    responsibilities: "Keep the change small",
    instructions: "Work on {{task.title}}.",
    connection: agents.connections.first(),
    model: &model,
    custom_model: "",
    effort: None,
  }
  .profile()
  .expect("a valid profile");
  let id = apply_in(desktop.repo(), ProfileChange::Create(profile)).expect("the profile is stored");
  format!("agent-{id}")
}

#[test]
fn agent_list_keeps_the_focus_on_the_chosen_profile() {
  let mut desktop = with_connection();
  let planner = stored_profile(&desktop, "Planner");
  let builder = stored_profile(&desktop, "Builder");
  desktop.wait_for("nav-agents");
  desktop.click("nav-agents");
  desktop.wait_for(&builder);
  // Choosing a profile opens a new editor; the focus stays on the chosen item.
  desktop.tab_to(&builder, MAX_STOPS);
  desktop.enter();
  desktop.settle("the builder in the editor", |tree| {
    tree
      .get_element_by_id_mut(ROLE)
      .and_then(|element| element.as_text_input())
      .is_some_and(|input| input.value() == "Builder")
  });
  assert_eq!(desktop.focused().as_deref(), Some(builder.as_str()));
  // A press moves the focus with the selection too.
  desktop.press(&planner);
  desktop.settle("the planner in the editor", |tree| {
    tree
      .get_element_by_id_mut(ROLE)
      .and_then(|element| element.as_text_input())
      .is_some_and(|input| input.value() == "Planner")
  });
  assert_eq!(desktop.focused().as_deref(), Some(planner.as_str()));
}
