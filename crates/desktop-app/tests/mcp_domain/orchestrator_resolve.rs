//! The Orchestrator chat sends a run whose merge stopped at conflicts back to an agent (`continue_run`'s
//! `resolve_conflicts`, `docs/design/orchestrator-toolset.md`, decision 5), on the seeded Kontur's run that conflicts
//! with main: the chat's `merge_run` that stopped there names it as the next step; the resolution is always a proposal
//! naming the run, the writing step, the conflicting files and the note, which the person applies as their own
//! decision through `continue_run` and the same return the task page's *Ask an agent to resolve* sends: the runner
//! starts the writing step with the conflict's note, the chat's note after it, and the files as data. A proposal the
//! run moved on from is refused, and so is a resolution of a run whose merge did not conflict, to a step that may not
//! write, or of a run merged or still at work.

use orchester_core::{
  orchestrator::{
    ActionOutcome,
    change::{Change, ContinueChoice},
  },
  run::AttemptReason,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::{
  app_mcp::ToolError,
  orchestrator::{proposal::Decision, tools::ChatScope},
  tasks::{detail::TaskDetail, return_work},
  workspace::OrchestratorConsent,
};
use serde_json::{Value, json};

use super::{
  Served,
  kontur::{self, SeedRequest, Seeded, scenarios},
  orchestrator_chat::{actions, chat, chat_call},
  orchestrator_continue::decide,
  resolve::{latest, wait_for},
};

/// The seeded task whose ready run conflicts with main.
const CONFLICTING: &str = "ready for review, conflicts with main";
/// The files it conflicts in.
const FILES: [&str; 2] = ["src/webhooks/sender.rs", "src/webhooks/settings.rs"];
/// What the chat adds for the agent.
const NOTE: &str = "Keep both retry settings; main's default first.";

fn resolve(run: u32, step: Option<&str>) -> Value {
  let mut choice = json!({ "kind": "resolve_conflicts", "note": NOTE });
  if let Some(step) = step {
    choice["step"] = json!(step);
  }
  json!({ "project": "Kontur", "run": run.to_string(), "choice": choice })
}

/// The code of a refusal of the chat's tool.
fn code(error: &ToolError) -> &str {
  match error {
    ToolError::Refused { code, .. } => code,
    ToolError::InvalidArguments(_) => "invalid_arguments",
    ToolError::NotFound { .. } => "not_found",
    ToolError::AmbiguousProject(_) => "ambiguous_project",
  }
}

/// The chat merges run `number` of `scope`'s project, as the person asked; the merge stops at conflicts.
fn merge_conflicts(served: &mut Served, scope: &ChatScope, number: u32) -> ToolError {
  served
    .services()
    .workspace
    .set_orchestrator_allows(OrchestratorConsent::MergeRuns, true)
    .expect("the person lets the chat merge");
  let merge = json!({ "project": "Kontur", "run": number.to_string() });
  chat_call(served, scope, "merge_run", merge).expect_err("the merge conflicts")
}

/// The newest action of `scope`'s conversation.
fn newest(db: &ProjectDb, scope: &ChatScope) -> i64 {
  db.chat_entries(scope.chat)
    .expect("entries")
    .into_iter()
    .rev()
    .find(|entry| matches!(entry.kind, orchester_core::orchestrator::EntryKind::Action(_)))
    .expect("an action")
    .seq
}

#[test]
fn chat_merge_failed_names_the_resolution_and_proposes_it_with_run_step_files_and_note() {
  let (mut served, seeded) = Served::seeded("decisions");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let (key, run) = latest(&seeded, CONFLICTING);
  let number = run.number().get();
  let scope = chat(&served, &mut db);
  let early = chat_call(&mut served, &scope, "continue_run", resolve(number, None)).expect_err("nothing conflicted");
  assert_eq!(code(&early), "merge_not_conflicted", "{early:?}");

  let refused = merge_conflicts(&mut served, &scope, number);
  let ToolError::Refused { code: refusal, reasons } = &refused else {
    panic!("a refusal: {refused:?}");
  };
  assert_eq!(refusal, "merge_failed");
  let next = reasons
    .iter()
    .find(|reason| reason.key == "orchestrator.tools.mergeConflictsNext")
    .unwrap_or_else(|| panic!("the next step is named: {reasons:?}"));
  assert!(
    next
      .args
      .iter()
      .any(|(name, value)| *name == "files" && value.contains(FILES[0]) && value.contains(FILES[1])),
    "it names the conflicting files: {next:?}"
  );

  let told = chat_call(&mut served, &scope, "continue_run", resolve(number, None)).expect("proposed");
  assert_eq!(told["waits_for_person"], true, "{told}");
  let proposed = &told["proposed"]["continue"];
  assert_eq!(proposed["run"], number, "{told}");
  assert_eq!(proposed["choice"]["kind"], "resolve_conflicts", "{told}");
  assert_eq!(proposed["choice"]["step"], "implement", "the implementing step: {told}");
  assert_eq!(proposed["choice"]["files"], json!(FILES), "{told}");
  assert_eq!(proposed["choice"]["note"], NOTE, "{told}");

  let action = actions(&db, &scope).pop().expect("the proposal");
  assert_eq!(
    action.outcome,
    ActionOutcome::Waits,
    "always a proposal, whatever the switches"
  );
  assert_eq!(action.task.as_deref(), Some(key.as_str()));
  let Some(Change::Continue {
    choice: ContinueChoice::ResolveConflicts {
      step,
      to,
      base,
      files,
      note,
    },
    ..
  }) = action.change.map(|change| change.change)
  else {
    panic!("a resolution");
  };
  assert_eq!((step.as_str(), base.as_str()), ("implement", "main"));
  assert_eq!(
    to.map(|to| to.name).as_deref(),
    Some("Implement"),
    "the step as people read it"
  );
  assert_eq!(files, FILES);
  assert_eq!(note.as_deref(), Some(NOTE));

  // The person resolves the conflicts on the task page meanwhile: the merge that failed no longer stands.
  let project = served.services().workspace.projects().expect("projects")[0].id;
  served.services().jobs.of(project).of(run.id()).forget_merge_failure();
  decide(
    &mut served,
    &scope,
    newest(&db, &scope),
    Decision::Accept { widens: Vec::new() },
  )
  .expect("decided");
  let ActionOutcome::Refused { code, .. } = actions(&db, &scope).pop().expect("it").outcome else {
    panic!("refused once the run moved on");
  };
  assert_eq!(code, "changed_since_proposed");
}

#[test]
fn chat_resolution_refuses_what_the_task_page_would_not_offer() {
  let (mut served, seeded) = Served::seeded("decisions");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let scope = chat(&served, &mut db);
  let (_, clean) = latest(&seeded, "ready for review, merges cleanly");
  let refused = chat_call(&mut served, &scope, "continue_run", resolve(clean.number().get(), None)).expect_err("clean");
  assert_eq!(code(&refused), "merge_not_conflicted", "{refused:?}");

  let (_, run) = latest(&seeded, CONFLICTING);
  let number = run.number().get();
  merge_conflicts(&mut served, &scope, number);
  let detail = TaskDetail::load_in(&seeded.repo, run.task()).expect("the task");
  let view = detail.run.as_ref().expect("its run");
  let writing: Vec<_> = return_work::writing_targets(view)
    .iter()
    .map(|step| step.key.clone())
    .collect();
  let reading = view
    .steps
    .iter()
    .find(|step| !writing.contains(&step.key))
    .expect("a step whose agent may not write");
  let refused = chat_call(
    &mut served,
    &scope,
    "continue_run",
    resolve(number, Some(reading.name.as_ref())),
  )
  .expect_err("it may not write");
  assert_eq!(code(&refused), "step_cannot_write", "{refused:?}");
  let refused = chat_call(&mut served, &scope, "continue_run", resolve(number, Some("deploy"))).expect_err("no step");
  assert_eq!(code(&refused), "step_not_offered", "{refused:?}");

  let (mut served, seeded) = Served::seeded("task-states");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let scope = chat(&served, &mut db);
  for (state, expected) in [("merged", "run_merged"), ("running", "run_not_ended")] {
    let (_, run) = latest(&seeded, state);
    let refused = chat_call(&mut served, &scope, "continue_run", resolve(run.number().get(), None)).expect_err(state);
    assert_eq!(code(&refused), expected, "{state}: {refused:?}");
  }
  assert!(
    actions(&db, &scope)
      .iter()
      .all(|action| matches!(action.outcome, ActionOutcome::Refused { .. })),
    "each refusal is kept, nothing waits"
  );
}

#[test]
fn chat_resolution_applied_returns_the_work_as_ask_an_agent_does_as_the_persons() {
  let folder = tempfile::tempdir().expect("a temporary folder");
  let seeded: Seeded = kontur::seed(&SeedRequest {
    data_dir: folder.path().join("bin"),
    repo: folder.path().join("repo-decisions"),
    scenario: scenarios::named("decisions").expect("a known scenario"),
    fake_codex: None,
    now: Timestamp::now(),
  })
  .expect("the scenario seeds");
  let (key, run) = latest(&seeded, CONFLICTING);
  // The real runner, which records the return and starts the step's attempt.
  let runner = super::runner_programs::programs().runner.clone();
  let mut served = Served::start_with(folder, &seeded.app_database, runner, true);
  served.ok(
    "navigate",
    &json!({ "route": "task", "project": seeded.project.to_string(), "key": key }),
  );
  served.until("the task page", |served| served.shows("task-merge"));
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let scope = chat(&served, &mut db);
  let number = run.number().get();
  merge_conflicts(&mut served, &scope, number);
  chat_call(&mut served, &scope, "continue_run", resolve(number, Some("Implement"))).expect("proposed");
  // Let automation answer approvals stays off: the person's Apply is their own decision.
  decide(
    &mut served,
    &scope,
    newest(&db, &scope),
    Decision::Accept { widens: Vec::new() },
  )
  .expect("decided");
  assert_eq!(
    actions(&db, &scope).pop().expect("it").outcome,
    ActionOutcome::Applied,
    "applied as the person's"
  );
  let returned = wait_for(&mut served, &seeded, run.id(), |stored| {
    stored.attempts().len() > run.attempts().len()
  });
  let AttemptReason::ReturnedAfterEnd {
    return_to,
    note,
    conflicts,
  } = returned.attempts()[run.attempts().len()].reason().clone()
  else {
    panic!("a return after the run ended: {:?}", returned.attempts().last());
  };
  assert_eq!(return_to.as_str(), "implement");
  let base = run.base_branch().expect("a base branch");
  let page = return_work::conflict_note(base);
  assert!(
    note.as_str().starts_with(page.as_str()),
    "the page's words lead: {}",
    note.as_str()
  );
  assert!(
    note.as_str().ends_with(NOTE),
    "the chat's note follows: {}",
    note.as_str()
  );
  let conflicts = conflicts.expect("what the return resolves");
  assert_eq!(conflicts.base.as_str(), "main");
  assert_eq!(conflicts.files, FILES);
  let project = served.services().workspace.projects().expect("projects")[0].id;
  assert!(
    served
      .services()
      .jobs
      .of(project)
      .of(run.id())
      .state()
      .conflicts
      .is_empty(),
    "the merge that failed is forgotten, as after Ask an agent to resolve"
  );
}
