//! The process rule uses executable paths as reported and with links resolved, not a working directory or a prefix
//! string.

use std::{
  fs,
  path::{Path, PathBuf},
};

use orchester_core::{
  orchester_tools::{OrchesterTool, ProcessCall, RunProcessScope, StepTools, ToolCall, ToolPolicy},
  resource::RepositoryAccess,
  time::Timestamp,
};
use serde_json::json;

use crate::permissions;

fn folders() -> tempfile::TempDir {
  let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/run-processes-core");
  fs::create_dir_all(&base).expect("scratch base");
  tempfile::tempdir_in(fs::canonicalize(base).expect("absolute scratch")).expect("scratch folder")
}

#[cfg(windows)]
fn link(from: &Path, to: &Path) {
  use std::os::windows::process::CommandExt;
  let result = std::process::Command::new("cmd")
    .args(["/C", "mklink", "/J"])
    .arg(from)
    .arg(to)
    .creation_flags(0x0800_0000)
    .output()
    .expect("junction");
  assert!(result.status.success(), "{result:?}");
}

#[cfg(not(windows))]
fn link(from: &Path, to: &Path) {
  std::os::unix::fs::symlink(to, from).expect("link");
}

#[test]
fn run_processes_path_inside_outside_and_prefix_sibling() {
  let folder = folders();
  let root = folder.path().join("run");
  let nested = root.join("target/debug");
  fs::create_dir_all(&nested).expect("root");
  let inside = nested.join("app");
  fs::write(&inside, "").expect("executable");
  let sibling = folder.path().join("run-other");
  fs::create_dir(&sibling).expect("sibling");
  let outside = sibling.join("app");
  fs::write(&outside, "").expect("executable");
  let scope = RunProcessScope::new(&root, Timestamp::now()).expect("scope");
  assert_eq!(
    scope.relative_executable(&inside).expect("path"),
    Some(PathBuf::from("target/debug/app"))
  );
  assert_eq!(scope.relative_executable(&outside).expect("path"), None);
  assert_eq!(scope.relative_executable(&nested).expect("folder"), None);
  assert!(scope.relative_executable(&root.join("missing")).is_err());
  assert!(RunProcessScope::new(&inside, Timestamp::now()).is_err());
}

#[test]
fn run_processes_path_refuses_links_in_both_directions() {
  let folder = folders();
  let root = folder.path().join("run");
  let outside = folder.path().join("other");
  for dir in [&root, &outside] {
    fs::create_dir(dir).expect("folder");
    fs::write(dir.join("app"), "").expect("executable");
  }
  link(&root.join("escape"), &outside);
  link(&outside.join("enter"), &root);
  link(&folder.path().join("run-alias"), &root);
  let scope = RunProcessScope::new(&folder.path().join("run-alias"), Timestamp::now()).expect("linked root");
  assert_eq!(scope.relative_executable(&root.join("escape/app")).expect("path"), None);
  // A program reported at an outside path stays outside even when a link there now leads into the worktree.
  assert_eq!(
    scope.relative_executable(&outside.join("enter/app")).expect("path"),
    None
  );
  for spelling in [root.join("app"), folder.path().join("run-alias/app")] {
    assert_eq!(
      scope.relative_executable(&spelling).expect("path"),
      Some(PathBuf::from("app")),
      "{}",
      spelling.display()
    );
  }
  // Joining `..` to a verbatim (`\\?\`) path pops instead; the usual spelling keeps the component.
  let plain = PathBuf::from(root.to_string_lossy().trim_start_matches(r"\\?\"));
  let parent = plain.join("escape").join("..").join("app");
  assert_eq!(scope.relative_executable(&parent).expect("path"), None, "{parent:?}");
}

#[cfg(windows)]
#[test]
fn run_processes_path_windows_case_and_verbatim_spelling() {
  let folder = folders();
  let root = folder.path().join("WorkTree");
  fs::create_dir(&root).expect("folder");
  fs::write(root.join("App.EXE"), "").expect("executable");
  let scope =
    RunProcessScope::new(&PathBuf::from(root.to_string_lossy().to_lowercase()), Timestamp::now()).expect("case");
  assert!(
    scope
      .relative_executable(&root.join("app.exe"))
      .expect("case")
      .is_some()
  );
  assert!(
    scope
      .relative_executable(&fs::canonicalize(root.join("App.EXE")).expect("canonical"))
      .expect("verbatim")
      .is_some()
  );
}

#[test]
fn run_processes_strict_input_and_unasked_policy_for_both_clis() {
  let mut permissions = permissions(RepositoryAccess::ReadOnly);
  permissions.set_interactive_tool_approvals(true);
  let step = StepTools::for_step(&permissions, &[]);
  for tool in [OrchesterTool::ListRunProcesses, OrchesterTool::StopRunProcess] {
    assert_eq!(step.policy(tool), Some(ToolPolicy::Runs));
    assert!(step.claude_allow_rules().contains(&tool.called(true)));
    assert!(step.codex_approved().contains(&tool.as_str()));
  }
  assert_eq!(
    ToolCall::parse("list_run_processes", &json!({}), &step).expect("input"),
    ToolCall::Processes(ProcessCall::List)
  );
  let stop = ToolCall::parse("stop_run_process", &json!({"pid":123}), &step).expect("input");
  assert!(!step.call_asks(&stop));
  for input in [
    json!({"pid":0}),
    json!({"pid":-1}),
    json!({"pid":4294967296u64}),
    json!({"pid":"123"}),
    json!({"pid":1,"force":true}),
    json!({}),
  ] {
    assert!(ToolCall::parse("stop_run_process", &input, &step).is_err(), "{input}");
  }
  assert!(ToolCall::parse("list_run_processes", &json!({"run":"another"}), &step).is_err());
  let listed = step.mcp_tools();
  assert_eq!(
    listed
      .iter()
      .find(|tool| tool["name"] == "stop_run_process")
      .expect("listed")["annotations"]["readOnlyHint"],
    false
  );
  for claude in [true, false] {
    let prompt = step.prompt_paragraph(claude);
    assert!(prompt.contains(&OrchesterTool::StopRunProcess.called(claude)));
    assert!(prompt.contains("instead of Stop-Process or kill"));
  }
}

#[cfg(windows)]
#[test]
fn run_processes_path_windows_distinct_unicode_names_stay_outside() {
  let folder = folders();
  let root = folder.path().join("straße");
  let outside = folder.path().join("strasse");
  fs::create_dir(&root).expect("root");
  fs::create_dir(&outside).expect("distinct Windows name");
  fs::write(root.join("app.exe"), "").expect("inside");
  fs::write(outside.join("app.exe"), "").expect("outside");
  let scope = RunProcessScope::new(&root, Timestamp::now()).expect("scope");
  assert!(
    scope
      .relative_executable(&root.join("app.exe"))
      .expect("inside")
      .is_some()
  );
  assert_eq!(
    scope.relative_executable(&outside.join("app.exe")).expect("outside"),
    None
  );
}

#[test]
fn run_processes_scope_refuses_processes_started_before_the_run() {
  let folder = folders();
  let run_created = Timestamp::from_unix_millis(1_700_000_000_000);
  let scope = RunProcessScope::new(folder.path(), run_created).expect("scope");
  assert!(!scope.started_in_run(1_699_999_999_999_999));
  assert!(scope.started_in_run(1_700_000_000_000_000));
  assert!(scope.started_in_run(1_700_000_000_000_001));
}
