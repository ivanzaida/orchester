use orchester_core::{
  connection::{AgentProvider, CatalogModel, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, SignIn},
  name::Name,
  resource::ModelId,
  time::Timestamp,
};
use orchester_desktop::connections::data::{ConnectionDraft, save_in};

use crate::harness::{Desktop, expected};

const ROLE: &str = "agent-role";
const RESPONSIBILITY: &str = "agent-responsibility";
const INSTRUCTIONS: &str = "agent-instructions";
const STATUS: &str = "agent-save-status";
const INPUTS: [&str; 3] = [ROLE, RESPONSIBILITY, INSTRUCTIONS];

/// A new profile in a project with a checked Codex connection, whose model has efforts.
fn new_profile() -> Desktop {
  let mut desktop = Desktop::with_project();
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
  // In the order they appear: Save draft and Publish (skipped, they cannot be used yet), New
  // agent in the list, Import (unavailable, skipped), then the fields; the connection, model
  // and effort selects are stops.
  assert_eq!(
    desktop.stops(8, false, &INPUTS),
    expected(&[
      None,
      Some(ROLE),
      Some(RESPONSIBILITY),
      None,
      None,
      None,
      Some(INSTRUCTIONS),
      None
    ])
  );
  assert_eq!(
    desktop.stops(3, true, &INPUTS),
    expected(&[Some(INSTRUCTIONS), None, None])
  );
}

#[test]
fn agent_editor_enter_saves_the_draft_once_it_is_valid() {
  let mut desktop = new_profile();
  let unsaved = status(&mut desktop);
  assert_eq!(
    desktop.stops(3, false, &INPUTS).last(),
    Some(&Some(RESPONSIBILITY.to_owned()))
  );
  desktop.type_text("Implement the plan");
  // The role is still missing: Enter saves nothing.
  desktop.enter();
  desktop.settle("the unsaved profile", |tree| tree.get_element_by_id(ROLE).is_some());
  assert_eq!(status(&mut desktop), unsaved);
  assert_eq!(desktop.stops(1, true, &INPUTS), expected(&[Some(ROLE)]));
  desktop.type_text("Builder");
  desktop.enter();
  desktop.settle("the saved draft", |tree| {
    tree
      .get_element_by_id(STATUS)
      .and_then(|status| status.text_content())
      .is_some_and(|text| text != unsaved)
  });
}
