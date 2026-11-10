//! What a local profile runs with is one set of fields (`Agents · New agent profile` L3, `Agents · Profile` L1): the
//! create form and the profile's panel draw the same *Agent CLI*, *Connection* (with *Add*), *Model* and *Reasoning
//! effort* rows, offering the same choices. `cargo test -p orchester-app --test agents local_hub_runs_on`.

use orchester_app::routes::{HubFilter, HubView};

use super::{
  harness::Agents,
  local_hub::open,
  local_hub_create::{choose, show_form},
};

/// The rows of what a profile runs with, by the name their ids end in.
const ROWS: [&str; 4] = ["cli", "connection", "model", "effort"];

/// Each row's label and the options of its select, under the ids that start with `prefix`.
fn runs_on(agents: &mut Agents, prefix: &str) -> Vec<(String, Vec<String>)> {
  ROWS
    .iter()
    .map(|row| {
      let label = agents.text(&format!("{prefix}-{row}-row-label"));
      let options = agents
        .window
        .tree
        .get_element_by_id_mut(&format!("{prefix}-{row}"))
        .and_then(|element| element.as_select())
        .unwrap_or_else(|| panic!("{prefix}-{row} is a select"))
        .labels()
        .iter()
        .map(ToString::to_string)
        .collect();
      (label, options)
    })
    .collect()
}

#[test]
fn local_hub_runs_on_the_create_form_and_the_panel_draw_the_same_fields() {
  let mut agents = Agents::new(true);
  let profile = agents.profile(true);
  show_form(&mut agents, HubFilter::Profiles);
  choose(&mut agents, "team-new-profile-cli", "Codex");
  agents.until("its connection is chosen for it", |s| {
    s.exists("team-new-profile-add-connection")
  });
  choose(&mut agents, "team-new-profile-model", "test-model");
  let created = runs_on(&mut agents, "team-new-profile");

  open(&mut agents, HubView::new(HubFilter::Profiles, Some(profile)));
  agents.until("the panel", |s| s.exists("local-hub-profile-add-connection"));
  let edited = runs_on(&mut agents, "local-hub-profile");

  assert_eq!(created, edited, "the same rows, offering the same choices");
  let labels: Vec<&str> = created.iter().map(|(label, _)| label.as_str()).collect();
  assert_eq!(labels, ["Agent CLI", "Connection", "Model", "Reasoning effort"]);
  assert!(
    created[1].1.iter().all(|option| option.starts_with("Codex · ")),
    "a connection is named with its CLI: {:?}",
    created[1].1
  );
}
