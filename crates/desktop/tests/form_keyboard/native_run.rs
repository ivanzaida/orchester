//! What the task page shows of an agent Orchester runs itself (04.H9 `wvh5j`, H10 `IcGvI`, H11 `ZvWcK`):
//! the approval of a shell command, the invocation panel of the Activity tab and the note of what could
//! not be enforced. The run, its invocation, its transcript and its pending approval are seeded into the
//! project database as the runner records them, on a DeepSeek connection, which runs no agent yet.

use std::{
  collections::BTreeMap,
  fs,
  path::{Path, PathBuf},
  sync::Arc,
  time::Duration,
};

use orchester_core::{
  command::{Command, CommandError, CommandOutput, CommandRunner},
  connection::{
    AgentProvider, EndpointPreset, ModelSource, NativeModels,
    invocation::Platform,
    stream::{AgentEvent, CommandStatus},
  },
  id::{IdempotencyKey, ResourceId},
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess,
    StepPermissions,
  },
  resource::{AgentProfile, ModelId, Pipeline, PromptText, Slug, Step, StepKey, StepKind},
  run::{
    CommandShell, Invocation, NativeCommandAsk, PendingToolApproval, PublishedVersions, Run, RunSnapshot, TokenUsage,
  },
  secret::Environment,
  store::{ActivityStore, ResourceStore, RunStore, TaskStore},
  task::{TaskDetails, TaskLimits},
  time::Timestamp,
  tokens::TokenBudget,
};
use orchester_desktop::programs::Programs;
use orchester_store::ProjectDb;

use crate::{
  github::shows,
  harness::Desktop,
  native_connections::{NativeSeed, key_secret, model, native_connection, profile},
  task_run::has,
  unreadable_run::{at, name, started},
};

const DIALOG: &str = "tool-approval-dialog";
const SHELL_COMMAND: &str = "Get-ChildItem src -Recurse -Filter *.test.ts | ForEach-Object { pnpm vitest run \
                             $_.FullName --reporter=dot } ; Remove-Item .vitest-cache -Recurse -Force";

/// Builder's permissions: edits the worktree, three listed commands, network off.
fn builder_permissions() -> AgentPermissions {
  let listed = ["git", "pnpm test", "pnpm lint"]
    .map(|command| CommandPrefix::new(command).expect("prefix"))
    .to_vec();
  AgentPermissions::new(
    RepositoryAccess::Write,
    CommandPermission::Allowed(AllowedCommands::new(listed).expect("list")),
    NetworkAccess::Off,
  )
}

/// *DeepSeek · Kontur* and the published Builder profile on it.
fn deepseek_builder(db: &mut ProjectDb) -> ResourceId {
  let key = key_secret(db, "DEEPSEEK_API_KEY", Environment::Development, false);
  let models = NativeModels::new(
    vec![model("deepseek-v4-pro", None, &[], ModelSource::Preset)],
    None,
    None,
  )
  .expect("models");
  let connection = native_connection(
    db,
    NativeSeed {
      name: "DeepSeek · Kontur",
      provider: AgentProvider::OpenAiApi,
      preset: EndpointPreset::DeepSeek,
      url: "https://api.deepseek.com",
      key,
      models,
      check: None,
    },
  );
  profile(db, "Builder", connection, "deepseek-v4-pro", builder_permissions())
}

/// The published one-step pipeline "Feature delivery", whose step Implement runs `builder`.
fn snapshot(db: &mut ProjectDb, builder: ResourceId) -> RunSnapshot {
  let version = db
    .latest_version::<AgentProfile>(builder)
    .expect("read")
    .expect("published profile");
  let profiles = BTreeMap::from([(builder, version)]);
  let step = Step {
    key: StepKey::new("implement").expect("valid key"),
    name: name("Implement"),
    kind: StepKind::Agent {
      profile: Some(builder),
      repository: RepositoryAccess::Write,
      model: None,
      permissions: StepPermissions::default(),
    },
    instructions: PromptText::new("").expect("valid prompt"),
    depends_on: Default::default(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  };
  let pipeline = Pipeline::new(vec![step]).expect("valid pipeline");
  let delivery = db
    .create_resource(
      Slug::new("feature-delivery").expect("slug"),
      name("Feature delivery"),
      &pipeline,
      at(3),
    )
    .expect("pipeline")
    .id();
  let version = db
    .publish_draft::<Pipeline>(delivery, at(4))
    .expect("publish the pipeline");
  let (tools, instructions, skills) = (BTreeMap::new(), BTreeMap::new(), BTreeMap::new());
  let published = PublishedVersions {
    profiles: &profiles,
    tools: &tools,
    instructions: &instructions,
    skills: &skills,
  };
  RunSnapshot::build(delivery, &version, published).expect("snapshot")
}

/// KON-1 with run #1, whose Implement step started on DeepSeek with Builder's launch permissions as
/// the runner records them, and that invocation.
pub(crate) fn native_run(repo: &Path) -> (Run, Invocation) {
  let mut db = ProjectDb::open(repo).expect("open the project");
  let builder = deepseek_builder(&mut db);
  let snapshot = snapshot(&mut db, builder);
  let details = TaskDetails {
    title: name("Add switcher keyboard shortcuts"),
    brief: String::new(),
    acceptance_criteria: Vec::new(),
    pipeline: None,
    limits: TaskLimits::new(
      Some(TokenBudget::new(1_400_000).expect("above zero")),
      Some(Duration::from_secs(1_800)),
    )
    .expect("limits"),
  };
  let task = db
    .create_tasks(IdempotencyKey::generate(), vec![details], at(5))
    .expect("task")[0]
    .id();
  let mut run = started(&mut db, (task, &snapshot), 1, 10);
  let execution = run.attempts()[0].executions()[0].id();
  let events = run.start_step(execution, at(11)).expect("start the step");
  db.append_events(&run, &events).expect("append");
  let invocation = db
    .start_invocation(
      run.id(),
      execution,
      &ModelId::new("deepseek-v4-pro").expect("model"),
      at(12),
    )
    .expect("invocation");
  let launch = builder_permissions()
    .enforced_on(AgentProvider::OpenAiApi, Platform::current())
    .expect("enforced by Orchester")
    .launch();
  db.save_invocation_permissions(invocation.id, &launch)
    .expect("launch permissions");
  (run, invocation)
}

fn open_task(desktop: &mut Desktop) {
  desktop.wait_for("nav-tasks");
  desktop.press("nav-tasks");
  desktop.wait_for("task-row-KON-1");
  desktop.click("task-row-KON-1");
  desktop.wait_for("task-page");
}

#[test]
fn native_shell_command_asks_with_its_shell_full_string_and_time_limit() {
  let mut desktop = Desktop::with_seeded_project(|repo| {
    let (run, invocation) = native_run(repo);
    let approval = PendingToolApproval {
      id: format!("{}:call_7", invocation.id),
      run: run.id(),
      execution: invocation.execution,
      invocation: invocation.id,
      agent: "Builder".to_owned(),
      command: true,
      scope: None,
      server: "Orchester".to_owned(),
      action: SHELL_COMMAND.to_owned(),
      target: Some(".orchester/worktrees/01J8Z9B2".to_owned()),
      requested_at: at(20),
      native: Some(NativeCommandAsk {
        shell: Some(CommandShell::Pwsh),
        time_limit_seconds: 120,
      }),
    };
    ProjectDb::open(repo)
      .expect("open the project")
      .request_tool_approval(&approval)
      .expect("pending approval");
  });
  desktop.wait_for(DIALOG);
  assert!(shows(&mut desktop, DIALOG, "Approve command?"));
  assert!(shows(
    &mut desktop,
    DIALOG,
    "Builder asks to run this command through a shell in the task worktree"
  ));
  // The agent row names what it runs on; the explanation names its role only.
  assert!(shows(&mut desktop, DIALOG, "Builder · DeepSeek / deepseek-v4-pro"));
  assert!(shows(
    &mut desktop,
    DIALOG,
    "Deny tells the agent, and it continues without the command"
  ));
  assert!(shows(
    &mut desktop,
    DIALOG,
    "pwsh · -NoProfile -NonInteractive -Command"
  ));
  assert!(shows(
    &mut desktop,
    "tool-approval-shell-command",
    "the full string the shell receives"
  ));
  // The whole string, never shortened.
  assert!(shows(&mut desktop, "tool-approval-shell-command", SHELL_COMMAND));
  assert!(shows(&mut desktop, DIALOG, ".orchester/worktrees/01J8Z9B2"));
  assert!(shows(&mut desktop, DIALOG, "120 s"));
  assert!(shows(
    &mut desktop,
    "tool-approval-unsandboxed",
    "Runs outside any sandbox"
  ));
  // Deny tells the agent and it goes on (decision 2); the wider answers sit above the footer.
  assert!(shows(
    &mut desktop,
    "tool-approval-deny-tells",
    "Denied by the person reviewing this run"
  ));
  let width = desktop.bounds(DIALOG).width;
  assert!((width - 640.0).abs() < 0.5, "{width} px wide");
}

#[test]
fn native_command_without_a_shell_asks_without_a_shell_row() {
  let mut desktop = Desktop::with_seeded_project(|repo| {
    let (run, invocation) = native_run(repo);
    let approval = PendingToolApproval {
      id: format!("{}:call_3", invocation.id),
      run: run.id(),
      execution: invocation.execution,
      invocation: invocation.id,
      agent: "Builder".to_owned(),
      command: true,
      scope: None,
      server: "Orchester".to_owned(),
      action: "pnpm add @kontur/keymap".to_owned(),
      target: None,
      requested_at: at(20),
      native: Some(NativeCommandAsk {
        shell: None,
        time_limit_seconds: 300,
      }),
    };
    ProjectDb::open(repo)
      .expect("open the project")
      .request_tool_approval(&approval)
      .expect("pending approval");
  });
  desktop.wait_for(DIALOG);
  assert!(shows(
    &mut desktop,
    DIALOG,
    "not on the allowed list and not known to be safe"
  ));
  assert!(shows(&mut desktop, DIALOG, "pnpm add @kontur/keymap"));
  assert!(shows(&mut desktop, DIALOG, "300 s"));
  assert!(!shows(&mut desktop, DIALOG, "Shell"));
  assert!(desktop.tree.get_element_by_id("tool-approval-shell-command").is_none());
}

#[test]
fn native_activity_panel_names_the_endpoint_network_and_tool_calls() {
  let mut desktop = Desktop::with_seeded_project(|repo| {
    let (_, invocation) = native_run(repo);
    let usage = TokenUsage {
      input: 46_000,
      cached_input: 0,
      output: 18_000,
    };
    let command = |item: &str, text: &str| AgentEvent::CommandStarted {
      item: item.to_owned(),
      command: text.to_owned(),
    };
    let finished = |item: &str, text: &str| AgentEvent::CommandFinished {
      item: item.to_owned(),
      command: text.to_owned(),
      output: String::new(),
      exit_code: Some(0),
      status: CommandStatus::Completed,
    };
    let events = [
      AgentEvent::TurnStarted,
      command("call_1", "pnpm test src/switcher"),
      finished("call_1", "pnpm test src/switcher"),
      command("call_2", "pnpm add @kontur/keymap"),
      finished("call_2", "pnpm add @kontur/keymap"),
      AgentEvent::PermissionDenied {
        item: "call_3".to_owned(),
        command: SHELL_COMMAND.to_owned(),
      },
      AgentEvent::TurnCompleted { usage },
    ];
    let events: Vec<(Timestamp, AgentEvent)> = events
      .into_iter()
      .zip(13..)
      .map(|(event, second)| (at(second * 1_000), event))
      .collect();
    ProjectDb::open(repo)
      .expect("open the project")
      .append_activity(&invocation, &events)
      .expect("transcript");
  });
  open_task(&mut desktop);
  desktop.click("task-tab-conversation");
  desktop.wait_for("conversation-details");
  desktop.click("conversation-details");
  desktop.wait_for("activity-invocation");
  let panel = "activity-invocation";
  assert!(shows(
    &mut desktop,
    "activity-step-header",
    "DeepSeek / deepseek-v4-pro · native"
  ));
  assert!(shows(&mut desktop, "activity-invocation-title", "Implement invocation"));
  assert!(shows(&mut desktop, panel, "Builder · deepseek-v4-pro"));
  assert!(shows(&mut desktop, panel, "Run budget"));
  assert!(shows(&mut desktop, panel, " / 1M tokens"));
  assert!(shows(
    &mut desktop,
    "activity-invocation-permissions",
    "Edit files · 3 commands ↗"
  ));
  assert!(!shows(&mut desktop, panel, "Invocation"), "no invocation number row");
  assert!(shows(&mut desktop, panel, "This computer (Orchester)"));
  assert!(shows(&mut desktop, panel, "Endpoint now"));
  assert!(shows(&mut desktop, panel, "https://api.deepseek.com"));
  assert!(shows(&mut desktop, panel, "Stays on · off not enforceable"));
  assert!(shows(&mut desktop, panel, "3 · 1 denied"));
  assert!(shows(&mut desktop, panel, "the usage the endpoint reports"));
  denied_command_is_a_titled_row(&mut desktop);
}

/// The denied command is a titled problem row that explains itself, without a command box.
fn denied_command_is_a_titled_row(desktop: &mut Desktop) {
  assert!(shows(desktop, "conversation", "Command denied"));
  assert!(shows(desktop, "conversation", "permission_denied"));
  assert!(shows(
    desktop,
    "conversation",
    "did not run: this run’s permissions do not allow it."
  ));
  assert!(
    desktop
      .tree
      .find_element(|element| element.id().is_some_and(|id| id.starts_with("activity-command-copy-")))
      .is_none(),
    "no Copy beside the denied command"
  );
}

/// Git as the Summary of a finished run asks it what the run left behind: no worktree, no branch.
/// Any other program was never meant to run.
struct NothingLeft;

impl CommandRunner for NothingLeft {
  fn run(&self, command: &Command) -> Result<CommandOutput, CommandError> {
    let git = command
      .program()
      .file_stem()
      .is_some_and(|program| program.eq_ignore_ascii_case("git"));
    assert!(git, "`{}` is not git", command.program().display());
    Ok(CommandOutput {
      code: Some(0),
      stdout: String::new(),
      stderr: String::new(),
    })
  }
}

#[test]
fn native_run_summary_says_what_orchester_could_not_enforce() {
  let mut desktop = Desktop::with_runner_and_programs(
    PathBuf::from("no-runner.exe"),
    Programs::with_runner(Arc::new(NothingLeft)),
    |repo| fs::create_dir_all(repo.join(".git")).expect("create .git"),
    |repo| {
      let (mut run, invocation) = native_run(repo);
      let mut db = ProjectDb::open(repo).expect("open the project");
      let events = run
        .complete_step(invocation.execution, at(40))
        .expect("complete the step");
      db.append_events(&run, &events).expect("append");
    },
  );
  open_task(&mut desktop);
  desktop.wait_for("task-not-enforced");
  let note = "task-not-enforced";
  assert!(shows(
    &mut desktop,
    note,
    "What Orchester could not enforce for Builder"
  ));
  assert!(shows(
    &mut desktop,
    note,
    "Network stayed on. Builder’s profile asks for network off"
  ));
  assert!(shows(
    &mut desktop,
    note,
    "Listed commands were not confined. git, pnpm test and pnpm lint ran"
  ));
  assert!(shows(&mut desktop, note, "Builder’s permissions ↗"));
  // It sits under the readiness card.
  let (ready, card) = (desktop.bounds("task-ready-notice"), desktop.bounds(note));
  assert!(card.y > ready.y, "the note follows the readiness card");
  desktop.click("task-not-enforced-profile");
  desktop.wait_until("the profile opens", |desktop| {
    has(desktop, "agent-editor") && shows(desktop, "agent-editor", "Builder")
  });
}
