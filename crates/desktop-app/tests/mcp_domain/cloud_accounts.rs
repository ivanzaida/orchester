//! Projects per Orchester Cloud account (the person's decision, 2026-10-16) through the domain tools: another account
//! signed in on the same computer finds none of the first one's projects, by id or by name (they are never hinted at:
//! `project_not_found`, not `sign_in_required`), and the first one finds them again, with their runs, when it signs
//! back in.

use orchester_workspace::workspace::MoveTo;
use serde_json::{Value, json};

use super::{
  Served,
  signed_in::account_cloud::{EMAIL, OTHER_EMAIL, PASSWORD},
};

/// Signs the session out, then in as `email`, as a person does in Settings › Orchester Cloud.
fn switch_to(served: &Served, email: &str) {
  let cloud = served.services().cloud.clone();
  served._runtime.block_on(async {
    cloud.sign_out().await.expect("signed out");
    cloud
      .sign_in(email.into(), orchester_api::Credential::new(PASSWORD.into()))
      .await
      .expect("signed in");
  });
}

fn runs(value: &Value) -> usize {
  value["runs"].as_array().map_or(0, Vec::len)
}

#[test]
fn domain_another_cloud_account_finds_none_of_the_projects() {
  let (mut served, seeded) = Served::seeded("cards");
  let id = seeded.project.to_string();
  // The seed adds it with nobody signed in, which makes it this computer's: the account takes it as a person would.
  let services = served.services();
  let kontur = services
    .workspace
    .projects()
    .expect("the application database")
    .into_iter()
    .find(|project| project.id == seeded.project)
    .expect("the seeded project");
  services
    .servers
    .move_project(&kontur, MoveTo::Account)
    .expect("moved into the account");
  let before = runs(&served.ok("list_runs", &json!({ "project": "Kontur" })));
  assert!(before > 0, "the scenario has runs");

  switch_to(&served, OTHER_EMAIL);
  let listed = served.ok("list_projects", &json!({}));
  assert!(!listed.to_string().contains(&id), "not listed: {listed}");
  for named in ["Kontur", id.as_str()] {
    for tool in ["get_project", "list_tasks", "list_runs", "list_pipelines"] {
      let refused = served.refused(tool, &json!({ "project": named }));
      assert_eq!(refused["code"], "project_not_found", "{tool} of {named}: {refused}");
    }
  }
  let everywhere = served.ok("list_runs", &json!({}));
  assert!(!everywhere.to_string().contains(&id), "no run of it: {everywhere}");

  switch_to(&served, EMAIL);
  let listed = served.ok("list_projects", &json!({}));
  assert!(listed.to_string().contains(&id), "listed again: {listed}");
  assert_eq!(
    runs(&served.ok("list_runs", &json!({ "project": id }))),
    before,
    "with its runs"
  );
}
