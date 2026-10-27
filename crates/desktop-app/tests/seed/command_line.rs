//! The seed's command line (`examples/seed/cli.rs`): the scenario is checked against the registry, the only list of
//! scenarios, so `tools/qa-app.ps1` names none.

use std::collections::BTreeSet;

use crate::{
  cli::{self, CliError, Command},
  kontur::scenarios,
};

fn args(words: &[&str]) -> impl Iterator<Item = String> {
  words
    .iter()
    .map(|word| (*word).to_owned())
    .collect::<Vec<_>>()
    .into_iter()
}

fn registered() -> BTreeSet<&'static str> {
  scenarios::ALL.iter().map(|scenario| scenario.name()).collect()
}

#[test]
fn seed_command_line_refuses_an_unknown_scenario_listing_the_known_ones_and_writes_nothing() {
  let folder = tempfile::tempdir().expect("a temporary folder");
  let (bin, repo) = (folder.path().join("bin"), folder.path().join("repo-nope"));
  let mut out = Vec::new();
  let refused = cli::run(
    args(&[
      "--data-dir",
      bin.to_str().expect("a UTF-8 path"),
      "--repo",
      repo.to_str().expect("a UTF-8 path"),
      "--scenario",
      "nope",
    ]),
    &mut out,
  );
  let error = refused.expect_err("an unknown scenario is refused");
  assert!(
    matches!(&error, CliError::UnknownScenario(name) if name == "nope"),
    "{error}"
  );
  let message = error.to_string();
  let (head, list) = message.split_once('\n').expect("the known scenarios follow");
  assert_eq!(head, "unknown scenario nope; the known scenarios are:");
  let listed: Vec<&str> = list.lines().collect();
  let mut sorted = listed.clone();
  sorted.sort_unstable();
  assert_eq!(listed, sorted, "sorted");
  assert_eq!(listed.len(), scenarios::ALL.len());
  assert_eq!(listed.into_iter().collect::<BTreeSet<_>>(), registered());
  assert!(out.is_empty());
  let written: Vec<_> = std::fs::read_dir(folder.path()).expect("the folder reads").collect();
  assert!(written.is_empty(), "nothing was written: {written:?}");
}

#[test]
fn seed_command_line_accepts_every_registered_scenario() {
  for name in registered() {
    match cli::parse(args(&["--data-dir", "bin", "--repo", "repo", "--scenario", name])) {
      Ok(Command::Seed(request)) => assert_eq!(request.scenario.name(), name),
      Ok(Command::ListScenarios) => panic!("{name}: a list instead of a seed"),
      Err(error) => panic!("{name}: {error}"),
    }
  }
}

#[test]
fn seed_command_line_lists_the_registry_sorted_one_per_line() {
  let mut out = Vec::new();
  cli::run(args(&["--list-scenarios"]), &mut out).expect("the list is written");
  let printed = String::from_utf8(out).expect("UTF-8");
  let names: Vec<&str> = printed.lines().collect();
  assert_eq!(names, cli::scenario_names());
  assert_eq!(names.len(), scenarios::ALL.len(), "each scenario once");
  assert_eq!(names.iter().copied().collect::<BTreeSet<_>>(), registered());
  let mut sorted = names.clone();
  sorted.sort_unstable();
  assert_eq!(names, sorted);
  assert!(printed.ends_with('\n'));
  assert!(matches!(
    cli::parse(args(&["--list-scenarios", "--scenario", "empty"])),
    Err(CliError::ListWithOthers)
  ));
}

#[test]
fn seed_command_line_leaves_scenarios_out_of_the_qa_script() {
  let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/qa-app.ps1");
  let script = std::fs::read_to_string(script).expect("tools/qa-app.ps1 reads");
  assert!(!script.contains("ValidateSet"), "qa-app.ps1 lists no scenarios");
  assert!(
    script
      .lines()
      .any(|line| line.trim() == "[Parameter(Mandatory = $true)][string]$Scenario,"),
    "-Scenario is a plain mandatory string"
  );
}
