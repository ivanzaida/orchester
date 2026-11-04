//! The settings, usage and base notices areas of the coordinator link (`docs/architecture/remote-runners.md`, "Rules
//! of the boundary"): their requests and answers come back the same from their JSON, and on a fixture project the
//! link, asked and answered through JSON, says what the project database says. Run with
//! `cargo test -p orchester-runner --test coordinator_link_settings`.

use std::{fmt::Debug, path::Path};

use orchester_core::{
  agent_files::RepositoryAgentFiles,
  base_move::{BaseNotice, LandedCommit, LandedTask, NoticeDelivery},
  code_index::CodeIndexSetting,
  id::{ConnectionId, InvocationId, NoticeId, ProjectId, RunId, StepExecutionId, TaskId},
  limits::{ProjectLimits, QaSlots},
  name::Name,
  permission::ApprovalMode,
  project::{Project, ProjectKey},
  resource::ModelId,
  revision::Revision,
  run::{
    BranchName, EffectiveMode, ManualBecause,
    review::{AutoModeSettings, ReviewerChoice},
  },
  store::{SettingsStore, WorkspaceStore},
  time::Timestamp,
  tokens::TokensUsed,
  workspaces::{WarmChoice, WarmFolder, WarmPattern, WorkspaceSettings},
};
use orchester_runner::execution::{
  LocalLinks,
  link::{
    base_notices::{PendingNoticeRequest, TellsBaseMovesRequest},
    settings::{
      AgentFilesRequest, ApprovalModeRequest, AutoModeRequest, CodeIndexRequest, QaAppsAtOnce, QaAppsRequest,
      WarmFolders, WarmFoldersRequest,
    },
    usage::{ExecutionUsage, StepUsage, StepUsageRequest},
  },
};
use orchester_store::ProjectDb;
use serde::{Serialize, de::DeserializeOwned};

/// `value`, written as JSON and read back.
fn through_json<T: Serialize + DeserializeOwned>(value: &T) -> T {
  let json = serde_json::to_string(value).expect("the value serializes");
  serde_json::from_str(&json).expect("the JSON reads back")
}

fn round_trip<T: Serialize + DeserializeOwned + PartialEq + Debug>(value: &T) {
  assert_eq!(&through_json(value), value);
}

fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

fn revision(digit: char) -> Revision {
  Revision::new(&digit.to_string().repeat(Revision::LEN)).expect("a revision")
}

fn warm_folder() -> WarmFolder {
  WarmFolder::new(WarmPattern::new("target/").expect("a pattern"), WarmChoice::Keep)
}

fn reviewer() -> AutoModeSettings {
  AutoModeSettings {
    reviewer: Some(ReviewerChoice {
      connection: ConnectionId::generate(),
      model: ModelId::new("claude-haiku-4-5-20251001").expect("a model"),
      effort: Some("low".to_owned()),
    }),
  }
}

fn notice(delivery: NoticeDelivery) -> BaseNotice {
  BaseNotice {
    id: NoticeId::generate(),
    run: RunId::generate(),
    task: TaskId::generate(),
    branch: BranchName::new("master").expect("a branch"),
    revision: revision('a'),
    commits: vec![LandedCommit {
      revision: revision('b'),
      subject: "ORC-2: Implement".to_owned(),
    }],
    tasks: vec![LandedTask {
      key: "ORC-2".to_owned(),
      title: "Billing schema migration".to_owned(),
    }],
    overlapping: vec!["src/billing.rs".to_owned()],
    noticed_at: at(10),
    deliver_after: at(120_010),
    delivery,
  }
}

/// A project whose settings are none of their defaults.
fn fixture(root: &Path) -> ProjectDb {
  let status = std::process::Command::new("git")
    .arg("-C")
    .arg(root)
    .args(["init", "--quiet"])
    .status()
    .expect("run git");
  assert!(status.success(), "git init: {status}");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("a key"),
    Name::new("Kontur").expect("a name"),
  );
  let mut db = ProjectDb::create(root, project).expect("create the project");
  db.save_code_index(CodeIndexSetting::On)
    .expect("the code index is saved");
  db.save_repository_agent_files(RepositoryAgentFiles::Off)
    .expect("the agent files are saved");
  db.save_auto_mode_settings(&reviewer()).expect("the reviewer is saved");
  db.save_tells_base_moves(false).expect("the switch is saved");
  let limits = ProjectLimits::DEFAULT.with_qa_slots(QaSlots::new(5).expect("five slots"));
  db.save_project_limits(&limits).expect("the limits are saved");
  let workspaces = WorkspaceSettings {
    enabled: true,
    warm_up: None,
    folders: vec![warm_folder()],
  };
  db.save_workspace_settings(&workspaces)
    .expect("the workspace settings are saved");
  db
}

#[test]
fn coordinator_link_settings_requests_and_answers_round_trip() {
  round_trip(&CodeIndexRequest {});
  round_trip(&CodeIndexSetting::On);
  round_trip(&AgentFilesRequest {});
  round_trip(&RepositoryAgentFiles::Off);
  round_trip(&ApprovalModeRequest {
    run: RunId::generate(),
    profile: ApprovalMode::Autopilot,
  });
  round_trip(&EffectiveMode::Autopilot);
  round_trip(&EffectiveMode::Manual(ManualBecause::Run));
  round_trip(&AutoModeRequest {});
  round_trip(&AutoModeSettings::default());
  round_trip(&reviewer());
  round_trip(&WarmFoldersRequest {});
  round_trip(&WarmFolders {
    folders: vec![warm_folder()],
  });
  round_trip(&QaAppsRequest {});
  round_trip(&QaAppsAtOnce::of(QaSlots::new(3).expect("three slots")));
}

#[test]
fn coordinator_link_settings_usage_and_base_notices_round_trip() {
  let execution = StepExecutionId::generate();
  round_trip(&StepUsageRequest {
    run: RunId::generate(),
    executions: vec![execution, StepExecutionId::generate()],
  });
  round_trip(&StepUsage {
    used: vec![ExecutionUsage {
      execution,
      tokens: TokensUsed::new(12_345),
    }],
  });
  round_trip(&TellsBaseMovesRequest {});
  round_trip(&PendingNoticeRequest { run: RunId::generate() });
  round_trip(&Some(notice(NoticeDelivery::Pending)));
  round_trip(&notice(NoticeDelivery::Delivered {
    execution,
    invocation: InvocationId::generate(),
    at: at(130_000),
  }));
  round_trip(&notice(NoticeDelivery::Merged {
    execution,
    revision: revision('c'),
    at: at(130_000),
  }));
}

#[test]
fn coordinator_link_settings_qa_apps_at_once_read_back_only_within_their_range() {
  let none = QaAppsAtOnce { count: 0 };
  assert!(through_json(&none).slots().is_err());
  let too_many = QaAppsAtOnce {
    count: QaSlots::MAX + 1,
  };
  assert!(through_json(&too_many).slots().is_err());
  let five = through_json(&QaAppsAtOnce { count: 5 });
  assert_eq!(five.slots().expect("in range"), QaSlots::new(5).expect("five slots"));
}

#[test]
fn coordinator_link_settings_answers_what_the_project_database_says() {
  let repo = tempfile::tempdir().expect("a temporary directory");
  let db = fixture(repo.path());
  let link = LocalLinks::new(repo.path()).open().expect("a link");

  let code_index = through_json(
    &link
      .code_index(&through_json(&CodeIndexRequest {}))
      .expect("the setting"),
  );
  assert_eq!(code_index, db.code_index().expect("the setting"));
  assert_eq!(code_index, CodeIndexSetting::On);
  let agent_files = through_json(
    &link
      .agent_files(&through_json(&AgentFilesRequest {}))
      .expect("the setting"),
  );
  assert_eq!(agent_files, db.repository_agent_files().expect("the setting"));
  assert_eq!(agent_files, RepositoryAgentFiles::Off);
  let auto_mode = through_json(&link.auto_mode(&through_json(&AutoModeRequest {})).expect("the setting"));
  assert_eq!(auto_mode, db.auto_mode_settings().expect("the setting"));
  assert!(auto_mode.reviewer.is_some());
  let warm = through_json(
    &link
      .warm_folders(&through_json(&WarmFoldersRequest {}))
      .expect("the folders"),
  );
  assert_eq!(warm.folders, db.workspace_settings().expect("the setting").folders);
  assert_eq!(warm.folders, [warm_folder()]);
  let qa = through_json(
    &link
      .qa_apps_at_once(&through_json(&QaAppsRequest {}))
      .expect("the setting"),
  );
  assert_eq!(
    qa.slots().expect("in range"),
    db.project_limits().expect("the limits").qa_slots()
  );
  assert_eq!(qa.count, 5);
  let tells = through_json(
    &link
      .tells_base_moves(&through_json(&TellsBaseMovesRequest {}))
      .expect("the switch"),
  );
  assert_eq!(tells, db.tells_base_moves().expect("the switch"));
  assert!(!tells);
}

#[test]
fn coordinator_link_settings_answers_a_run_the_project_does_not_have_as_the_database_does() {
  let repo = tempfile::tempdir().expect("a temporary directory");
  let db = fixture(repo.path());
  let link = LocalLinks::new(repo.path()).open().expect("a link");
  let run = RunId::generate();
  let execution = StepExecutionId::generate();

  let notice = link
    .pending_notice(&through_json(&PendingNoticeRequest { run }))
    .expect("the notice");
  assert_eq!(through_json(&notice), db.pending_base_notice(run).expect("the notice"));
  assert_eq!(notice, None);

  let usage = link
    .step_usage(&through_json(&StepUsageRequest {
      run,
      executions: vec![execution],
    }))
    .expect("the usage");
  let usage = through_json(&usage);
  assert_eq!(usage.of(execution), TokensUsed::default());
  assert_eq!(usage.of(StepExecutionId::generate()), TokensUsed::default());

  // A run the project does not store has no mode: the link refuses it as the database does, with its kind.
  let request = ApprovalModeRequest {
    run,
    profile: ApprovalMode::Autopilot,
  };
  let refused = link.approval_mode(&through_json(&request)).expect_err("no such run");
  let direct = db.approval_mode(run, ApprovalMode::Autopilot).expect_err("no such run");
  assert_eq!(refused.detail, direct.to_string());
  assert_eq!(refused.failure, direct.runner_failure());
}
