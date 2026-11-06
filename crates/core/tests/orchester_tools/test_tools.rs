//! The test tools: `run_tests` is offered with the project's published test suites to a step that reaches the
//! network, asks at every call (suites have no approval rules of their own yet) and auto-approval never answers it;
//! `test_output` comes with it and never asks. `run_tests` takes only a published suite's slug and a filter that is one
//! argument; `test_output` takes a test run as `run_tests` names it. The prompt tells the agent to run suites with
//! them.

use orchester_core::{
  orchester_tools::{OrchesterTool, StepTools, ToolCall, ToolCallError, ToolPolicy, auto_approvable},
  permission::NetworkAccess,
  resource::{RepositoryAccess, Slug, TestFilter},
};
use serde_json::json;

use crate::permissions;

fn slugs(values: &[&str]) -> Vec<Slug> {
  values.iter().map(|slug| Slug::new(slug).expect("a slug")).collect()
}

/// A step with `repository` access and network on, offered the project's `suites`.
fn step(repository: RepositoryAccess, suites: &[&str]) -> StepTools {
  let mut online = permissions(repository);
  online.set_network(NetworkAccess::On);
  StepTools::for_step(&online, &[]).with_test_suites(&online, &slugs(suites))
}

#[test]
fn test_tools_come_with_published_suites_and_the_network() {
  let offline = permissions(RepositoryAccess::Write);
  let none = StepTools::for_step(&offline, &[]).with_test_suites(&offline, &slugs(&["unit"]));
  for tool in [OrchesterTool::RunTests, OrchesterTool::TestOutput] {
    assert_eq!(none.policy(tool), None, "network off");
    assert_eq!(step(RepositoryAccess::Write, &[]).policy(tool), None, "no suite");
  }
  for repository in [RepositoryAccess::Write, RepositoryAccess::ReadOnly] {
    let offered = step(repository, &["unit", "e2e"]);
    assert_eq!(offered.policy(OrchesterTool::RunTests), Some(ToolPolicy::Asks));
    assert_eq!(offered.policy(OrchesterTool::TestOutput), Some(ToolPolicy::Runs));
  }
  assert!(!auto_approvable("Orchester", "run_tests"));
  assert!(auto_approvable("Orchester", "test_output"));
}

#[test]
fn test_tools_take_a_published_suite_and_a_filter_that_is_one_argument() {
  let offered = step(RepositoryAccess::Write, &["unit"]);
  assert_eq!(
    ToolCall::parse(
      "run_tests",
      &json!({"suite": "unit", "filter": "skills_migration"}),
      &offered
    ),
    Ok(ToolCall::RunTests {
      suite: Slug::new("unit").expect("a slug"),
      filter: Some(TestFilter::new("skills_migration").expect("a filter")),
    })
  );
  assert_eq!(
    ToolCall::parse("run_tests", &json!({"suite": "unit", "filter": null}), &offered),
    Ok(ToolCall::RunTests {
      suite: Slug::new("unit").expect("a slug"),
      filter: None,
    })
  );
  assert_eq!(
    ToolCall::parse("run_tests", &json!({"suite": "e2e"}), &offered),
    Err(ToolCallError::UnknownSuite("e2e".to_owned()))
  );
  for refused in [
    json!({"suite": "unit", "filter": "a\nb"}),
    json!({"suite": "unit", "shell": "cargo test"}),
    json!({"suite": "Unit Tests"}),
  ] {
    assert!(
      matches!(
        ToolCall::parse("run_tests", &refused, &offered),
        Err(ToolCallError::Invalid {
          tool: OrchesterTool::RunTests,
          ..
        })
      ),
      "{refused}"
    );
  }
}

#[test]
fn test_output_takes_a_test_run_as_run_tests_names_it() {
  let offered = step(RepositoryAccess::ReadOnly, &["unit"]);
  let Ok(ToolCall::TestOutput { run, test }) = ToolCall::parse(
    "test_output",
    &json!({"run": "t12", "test": "tests > subtracts"}),
    &offered,
  ) else {
    panic!("test_output reads its input")
  };
  assert_eq!(
    (run.to_string(), test.as_str()),
    ("t12".to_owned(), "tests > subtracts")
  );
  for refused in [
    json!({"run": "t0", "test": "adds"}),
    json!({"run": "12", "test": "adds"}),
    json!({"run": "../t1", "test": "adds"}),
    json!({"run": "t1", "test": "  "}),
    json!({"run": "t1", "test": "a\nb"}),
  ] {
    assert!(
      matches!(
        ToolCall::parse("test_output", &refused, &offered),
        Err(ToolCallError::Invalid {
          tool: OrchesterTool::TestOutput,
          ..
        })
      ),
      "{refused}"
    );
  }
}

#[test]
fn test_tools_are_listed_and_named_in_the_prompt() {
  let offered = step(RepositoryAccess::Write, &["unit", "e2e"]);
  let tools = offered.mcp_tools();
  let run_tests = tools
    .iter()
    .find(|tool| tool["name"] == "run_tests")
    .expect("run_tests is listed");
  assert_eq!(
    run_tests["inputSchema"]["properties"]["suite"]["enum"],
    json!(["unit", "e2e"])
  );
  assert_eq!(run_tests["annotations"]["readOnlyHint"], false);
  let test_output = tools
    .iter()
    .find(|tool| tool["name"] == "test_output")
    .expect("test_output is listed");
  assert_eq!(test_output["annotations"]["readOnlyHint"], true);
  let paragraph = offered.prompt_paragraph(false);
  assert!(
    paragraph.contains("Run the project's test suites (unit, e2e) with run_tests(suite, filter), never by shell"),
    "{paragraph}"
  );
  assert!(paragraph.contains("test_output(run, test)"), "{paragraph}");
}
