//! Approve similar for Orchester's calls that reach something narrower than their tool: a folder outside the worktree
//! (never a root, a mount point, a user's home folder or a folder close below one) and a check of `run_check` (keyed to
//! the check, never one whose access rules ask before every call).

use std::path::{Path, PathBuf};

use orchester_core::{
  id::{InvocationId, RunId, StepExecutionId},
  orchester_tools::ProjectTool,
  permission::category::CommandScope,
  resource::{ApprovalMode, CommandAction, CommandProgram},
  run::{ApprovalRule, ApprovalRuleId, ApprovalSubject, PendingToolApproval, RuleScope, SimilarUnavailable, answering},
  time::Timestamp,
};

fn request(action: &str, target: &str) -> PendingToolApproval {
  PendingToolApproval {
    id: "invocation:1".to_owned(),
    run: RunId::generate(),
    execution: StepExecutionId::generate(),
    invocation: InvocationId::generate(),
    agent: "Builder".to_owned(),
    command: false,
    scope: None,
    server: "Orchester".to_owned(),
    action: action.to_owned(),
    target: Some(target.to_owned()),
    requested_at: Timestamp::from_unix_millis(0),
    native: None,
  }
}

fn rule(scope: RuleScope) -> ApprovalRule {
  ApprovalRule {
    id: ApprovalRuleId::new(1),
    run: RunId::generate(),
    scope,
    created_at: Timestamp::from_unix_millis(0),
    revoked_at: None,
    answered: 0,
  }
}

fn root() -> PathBuf {
  if cfg!(windows) {
    PathBuf::from(r"C:\")
  } else {
    PathBuf::from("/")
  }
}

fn text(path: &Path) -> String {
  path.to_string_lossy().into_owned()
}

#[test]
fn approval_reach_is_never_similar_for_a_broad_folder() {
  let temporary = tempfile::tempdir().expect("a folder");
  let resolve = |path: &Path| {
    std::fs::create_dir_all(path).expect("a folder");
    orchester_core::native::scope::resolve_local(&text(path)).expect("resolved")
  };
  let home = resolve(&temporary.path().join("home"));
  let profiles = home.parent().expect("the profiles folder").to_path_buf();
  let scope = CommandScope::new(&resolve(&home.join("work").join("repo")), &[], Some(&home), None);
  let similar = |folder: &Path| {
    RuleScope::similar(&ApprovalSubject::of(
      &request("read_external_file", &text(folder)),
      &scope,
    ))
  };
  for broad in [
    root(),
    root().join("deps"),
    home.clone(),
    home.join("projects"),
    home.parent().expect("a parent").to_path_buf(),
    // Another user's home next to this one, and a folder right below it.
    profiles.join("bob"),
    profiles.join("bob").join("projects"),
  ] {
    assert_eq!(
      similar(&broad),
      Err(SimilarUnavailable::BroadFolder),
      "{}",
      broad.display()
    );
  }
  #[cfg(unix)]
  for broad in ["/Users/bob", "/home/bob/projects", "/Users"] {
    assert_eq!(
      similar(Path::new(broad)),
      Err(SimilarUnavailable::BroadFolder),
      "{broad}"
    );
  }
  for narrow in [
    root().join("deps").join("serde"),
    home.join(".cargo").join("registry"),
    profiles.join("bob").join("projects").join("deps"),
  ] {
    assert!(similar(&narrow).is_ok(), "{}", narrow.display());
  }
  let search = ApprovalSubject::of(&request("search_external_file", &text(&home)), &scope);
  assert_eq!(RuleScope::similar(&search), Err(SimilarUnavailable::BroadFolder));
  let shouted = PathBuf::from(text(&home.join("projects")).to_uppercase());
  assert_eq!(
    similar(&shouted),
    Err(SimilarUnavailable::BroadFolder),
    "{}",
    shouted.display()
  );
}

fn check(name: &str, approval: ApprovalMode) -> ProjectTool {
  let action = CommandAction::new(CommandProgram::new("cargo").expect("a program"), Vec::new());
  ProjectTool::new(name, "", action, approval)
}

#[test]
fn approval_reach_keys_a_checks_rule_to_the_check() {
  let scope = CommandScope::new(&root().join("repo"), &[], None, None);
  let checks = [
    check("tests", ApprovalMode::OncePerRun),
    check("deploy", ApprovalMode::EveryCall),
    check("lint", ApprovalMode::Never),
  ];
  let subject = |name: &str| ApprovalSubject::of(&request("run_check", name), &scope).with_checks(&checks);
  let tests = RuleScope::similar(&subject("tests")).expect("similar");
  assert!(matches!(&tests, RuleScope::SimilarMcpTool { check: Some(name), .. } if name == "tests"));
  let rules = [rule(tests)];
  assert!(answering(&rules, &subject("tests")).is_some());
  assert!(answering(&rules, &subject("lint")).is_none());
  assert_eq!(
    RuleScope::similar(&subject("deploy")),
    Err(SimilarUnavailable::EveryCall)
  );
  assert_eq!(
    RuleScope::similar(&subject("unknown")),
    Err(SimilarUnavailable::EveryCall)
  );
  // Without the step's checks a check is unknown: no rule, the safe side.
  let unknown = ApprovalSubject::of(&request("run_check", "tests"), &scope);
  assert_eq!(RuleScope::similar(&unknown), Err(SimilarUnavailable::EveryCall));
}
