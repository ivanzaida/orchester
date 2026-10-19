//! `tools/qa-app.ps1` takes every scenario the QA seed registers (`examples/seed/kontur/scenarios`): its
//! `-Scenario` parameter lists them, so a QA copy of a scenario such as `api-providers` can be started.

use super::kontur::scenarios::ALL;

#[test]
fn connections_qa_app_offers_every_seed_scenario() {
  let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/qa-app.ps1");
  let script = std::fs::read_to_string(script).expect("tools/qa-app.ps1 reads");
  let offered = script
    .lines()
    .find(|line| line.trim_start().starts_with("[ValidateSet(") && line.contains("\"empty\""))
    .expect("the -Scenario parameter's ValidateSet");
  for scenario in ALL {
    assert!(
      offered.contains(&format!("\"{}\"", scenario.name())),
      "qa-app.ps1 does not offer {}",
      scenario.name()
    );
  }
}
