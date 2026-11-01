//! Installing a ready update (`app_updates::install`): the databases copied aside with the last two backups kept,
//! what *Restart to update* does with and without runs at work, the installer's silent arguments, the handover and
//! every failure before it, which leaves this version running with the reason, and what the next start reads.
//!
//! `cargo test -p orchester-workspace --test app_updates update_install`

use std::{
  fs, io,
  num::NonZeroU32,
  path::{Path, PathBuf},
  process::{Child, Command, Stdio},
  time::Duration,
};

use orchester_core::{
  connection::AgentProvider,
  id::{RunId, TaskId},
};
use orchester_ipc::{RunWork, WorkState, WorkSummary};
use orchester_store::SessionEnd;
use orchester_workspace::{
  activity::Activity,
  app_updates::{
    AppUpdates, ReadyUpdate, ThisApp, ThisCopy, UpdateState,
    install::{
      AfterInstall, BACKUPS_FOLDER, Databases, Handover, InstallRecord, InstallStep, Installer, JustUpdated,
      RECORD_FILE, RESULT_FILE, RestartPlan, RunsAtWork, after_install, back_up, backups,
    },
  },
  runner::Runners,
};

use super::{Copy, NEWER, Offer, RUNNING, installer_bytes, keys, offering, version, written};

/// Runners with no project, as a window with no project open has them.
fn no_runners() -> Runners {
  Runners::new(
    PathBuf::from("orchester-runner-not-started"),
    tokio::runtime::Handle::current(),
    Activity::new(),
    orchester_ipc::SessionEnd::Clean,
  )
}

/// A copy with a verified update of [`NEWER`] ready.
async fn ready_copy() -> (Copy, super::stand::Stand) {
  let bytes = installer_bytes();
  let stand = offering(Offer::of(&bytes), bytes);
  let copy = Copy::installed(&stand.api);
  copy.updates.check_now().await;
  assert!(
    matches!(copy.updates.state(), UpdateState::Ready(_)),
    "ready: {:?}",
    copy.updates.state()
  );
  (copy, stand)
}

fn installer(copy: &Copy, start: fn(&Handover) -> io::Result<Child>) -> Installer {
  Installer {
    program_folder: copy.dir.path().to_path_buf(),
    pid: std::process::id(),
    start,
  }
}

/// A stand-in installer that waits for the app, as the real one does: it runs for a few seconds.
fn waits(_: &Handover) -> io::Result<Child> {
  let mut command = if cfg!(windows) {
    let mut command = Command::new("ping");
    command.args(["-n", "5", "127.0.0.1"]);
    command
  } else {
    let mut command = Command::new("sleep");
    command.arg("4");
    command
  };
  command.stdout(Stdio::null()).stderr(Stdio::null()).spawn()
}

/// A stand-in installer that ends at once with exit code 3.
fn ends_at_once(_: &Handover) -> io::Result<Child> {
  let mut command = if cfg!(windows) {
    let mut command = Command::new("cmd");
    command.args(["/C", "exit 3"]);
    command
  } else {
    let mut command = Command::new("sh");
    command.args(["-c", "exit 3"]);
    command
  };
  command.stdout(Stdio::null()).stderr(Stdio::null()).spawn()
}

fn cannot_start(_: &Handover) -> io::Result<Child> {
  Err(io::Error::new(io::ErrorKind::NotFound, "no installer here"))
}

/// A project repository with a database and its write-ahead log.
fn project(root: &Path, name: &str, bytes: &[u8]) -> PathBuf {
  let repo = root.join(name);
  written(&repo.join(".orchester").join("orchester.db"), bytes);
  written(&repo.join(".orchester").join("orchester.db-wal"), b"project wal");
  repo
}

#[test]
fn update_install_copies_the_databases_aside_and_keeps_the_last_two_backups() {
  let data = tempfile::tempdir().expect("a folder");
  let app = data.path().join("orchester.db");
  written(&app, b"app database");
  written(&data.path().join("orchester.db-wal"), b"app wal");
  let repos = tempfile::tempdir().expect("a folder");
  let first = project(repos.path(), "kontur", b"kontur database");
  let empty = repos.path().join("no-database");
  fs::create_dir_all(&empty).expect("a folder");
  let databases = Databases {
    app: Some(app),
    projects: vec![first.clone(), empty],
  };

  for running in ["0.1.0", "0.2.0", "0.3.0"] {
    back_up(data.path(), &version(running), &databases).expect("backed up");
  }

  assert_eq!(
    backups(data.path()).expect("the backups"),
    [version("0.2.0"), version("0.3.0")],
    "the last two backups stay"
  );
  let folder = data.path().join(BACKUPS_FOLDER).join("0.3.0");
  assert_eq!(fs::read(folder.join("orchester.db")).expect("app"), b"app database");
  assert_eq!(fs::read(folder.join("orchester.db-wal")).expect("wal"), b"app wal");
  let kontur = folder.join("projects").join("1-kontur");
  assert_eq!(
    fs::read(kontur.join("orchester.db")).expect("project"),
    b"kontur database"
  );
  assert_eq!(fs::read(kontur.join("orchester.db-wal")).expect("wal"), b"project wal");
  let list = fs::read_to_string(folder.join("projects.txt")).expect("the list");
  assert_eq!(
    list,
    format!("1-kontur\t{}\n", first.display()),
    "a project without a database is left out"
  );
}

#[test]
fn update_install_a_second_backup_of_one_version_replaces_the_first() {
  let data = tempfile::tempdir().expect("a folder");
  let app = data.path().join("orchester.db");
  let databases = Databases {
    app: Some(app.clone()),
    projects: Vec::new(),
  };
  written(&app, b"before");
  back_up(data.path(), &version(RUNNING), &databases).expect("backed up");
  written(&app, b"after");
  fs::remove_file(data.path().join("orchester.db-wal")).ok();

  let folder = back_up(data.path(), &version(RUNNING), &databases).expect("backed up again");

  assert_eq!(fs::read(folder.join("orchester.db")).expect("app"), b"after");
  assert!(!folder.join("projects.txt").exists());
}

#[test]
fn update_install_the_installer_runs_silently_waits_for_the_app_and_opens_it_again() {
  let handover = Handover {
    installer: PathBuf::from(r"C:\Orchester\updates\Orchester-0.4.0-setup.exe"),
    program_folder: PathBuf::from(r"C:\Program Files\Orchester"),
    pid: 4242,
    log: PathBuf::from(r"C:\Orchester\logs\update-0.4.0.log"),
    result: PathBuf::from(r"C:\Orchester\installing-update-result.txt"),
  };

  assert_eq!(
    handover.arguments(),
    [
      "/VERYSILENT",
      "/SUPPRESSMSGBOXES",
      "/NORESTART",
      r#"/DIR="C:\Program Files\Orchester""#,
      r#"/LOG="C:\Orchester\logs\update-0.4.0.log""#,
      "/WAITPID=4242",
      "/RELAUNCH",
      r#"/UPDATERESULT="C:\Orchester\installing-update-result.txt""#,
    ]
  );
}

#[test]
fn update_install_the_next_start_of_the_new_version_says_updated() {
  let data = tempfile::tempdir().expect("a folder");
  let record = InstallRecord::new(
    data.path(),
    version(RUNNING),
    version(NEWER),
    "Runs start faster.".to_owned(),
  );
  record.keep(data.path()).expect("kept");

  let after = after_install(data.path(), &version(NEWER));

  let Some(AfterInstall::Updated(updated)) = after else {
    panic!("updated: {after:?}");
  };
  assert_eq!(updated.version, version(NEWER));
  assert_eq!(updated.notes, "Runs start faster.");
  assert!(!data.path().join(RECORD_FILE).exists(), "it is found once");
  assert_eq!(after_install(data.path(), &version(NEWER)), None);
}

#[test]
fn update_install_a_failed_installer_leaves_the_old_version_with_the_reason_and_the_log() {
  let data = tempfile::tempdir().expect("a folder");
  let record = InstallRecord::new(data.path(), version(RUNNING), version(NEWER), String::new());
  record.keep(data.path()).expect("kept");
  written(&record.log, b"Inno Setup log");
  written(
    &data.path().join(RESULT_FILE),
    b"installing the files failed and the changes were rolled back\r\n",
  );

  let after = after_install(data.path(), &version(RUNNING));

  let Some(AfterInstall::Failed {
    version: failed,
    failure,
  }) = after
  else {
    panic!("failed: {after:?}");
  };
  assert_eq!(failed, version(NEWER));
  assert_eq!(
    failure.reason,
    "installing the files failed and the changes were rolled back"
  );
  assert_eq!(failure.log, Some(record.log));
  assert!(!data.path().join(RESULT_FILE).exists());
}

#[test]
fn update_install_an_installer_that_wrote_nothing_still_says_it_stopped() {
  let data = tempfile::tempdir().expect("a folder");
  InstallRecord::new(data.path(), version(RUNNING), version(NEWER), String::new())
    .keep(data.path())
    .expect("kept");

  let Some(AfterInstall::Failed { failure, .. }) = after_install(data.path(), &version(RUNNING)) else {
    panic!("failed");
  };

  assert_eq!(failure.reason, "the installer stopped before it finished");
  assert_eq!(failure.log, None, "no log was written");
}

#[test]
fn update_install_a_new_start_shows_updated_to_or_the_failure() {
  let data = tempfile::tempdir().expect("a folder");
  let open = |running: &str| {
    let db = orchester_store::AppDb::open(&data.path().join("orchester.db")).expect("a database");
    let app = ThisApp {
      version: version(running),
      target: None,
      copy: ThisCopy::Installer,
    };
    AppUpdates::new(
      orchester_workspace::workspace::Workspace::new(db),
      app,
      keys(),
      data.path(),
    )
  };
  let keep = || {
    InstallRecord::new(data.path(), version(RUNNING), version(NEWER), "Notes".to_owned())
      .keep(data.path())
      .expect("kept");
  };

  keep();
  let updated = open(NEWER);
  assert_eq!(
    updated.just_updated().map(|updated| updated.version),
    Some(version(NEWER))
  );
  drop(updated);

  keep();
  let failed = open(RUNNING);
  assert_eq!(failed.just_updated(), None);
  assert!(
    matches!(&failed.state(), UpdateState::InstallFailed { version: v, .. } if *v == version(NEWER)),
    "{:?}",
    failed.state()
  );
}

#[tokio::test]
async fn update_install_with_no_run_at_work_installs_at_once() {
  let (copy, _stand) = ready_copy().await;
  written(&copy.dir.path().join("orchester.db-wal"), b"");
  let runners = no_runners();
  assert!(matches!(
    copy.updates.restart_plan(&runners),
    RestartPlan::InstallNow(_)
  ));

  let handed = copy
    .updates
    .install(&runners, RunsAtWork::WaitForThem, &installer(&copy, waits))
    .await;

  assert_eq!(handed.map(|handed| handed.end), Some(SessionEnd::Clean));
  assert!(
    matches!(
      copy.updates.state(),
      UpdateState::Installing {
        step: InstallStep::HandingOver,
        ..
      }
    ),
    "{:?}",
    copy.updates.state()
  );
  assert_eq!(backups(copy.dir.path()).expect("the backups"), [version(RUNNING)]);
  let record = fs::read_to_string(copy.dir.path().join(RECORD_FILE)).expect("the note for the next start");
  assert!(record.contains(&format!("\"to\": \"{NEWER}\"")), "{record}");
  assert!(
    copy
      .updates
      .install(&runners, RunsAtWork::StopThem, &installer(&copy, waits))
      .await
      .is_none(),
    "one install at a time"
  );
}

#[tokio::test]
async fn update_install_an_installer_that_cannot_start_leaves_this_version_running() {
  let (copy, _stand) = ready_copy().await;
  let runners = no_runners();

  let handed = copy
    .updates
    .install(&runners, RunsAtWork::StopThem, &installer(&copy, cannot_start))
    .await;

  assert_eq!(handed, None);
  let UpdateState::InstallFailed {
    version: failed,
    failure,
  } = copy.updates.state()
  else {
    panic!("failed: {:?}", copy.updates.state());
  };
  assert_eq!(failed, version(NEWER));
  assert!(
    failure.reason.contains("cannot start the installer"),
    "{}",
    failure.reason
  );
  assert_eq!(failure.exit_code, None);
  assert!(
    !copy.dir.path().join(RECORD_FILE).exists(),
    "no update is noted for the next start"
  );
  assert_eq!(
    backups(copy.dir.path()).expect("the backups"),
    [version(RUNNING)],
    "the backup stays"
  );

  copy.updates.check_now().await;
  assert!(
    matches!(copy.updates.state(), UpdateState::InstallFailed { .. }),
    "the cloud offering it again does not hide the failure: {:?}",
    copy.updates.state()
  );
}

#[tokio::test]
async fn update_install_an_installer_that_ends_before_the_app_failed_with_its_exit_code() {
  let (copy, _stand) = ready_copy().await;

  let handed = copy
    .updates
    .install(&no_runners(), RunsAtWork::WaitForThem, &installer(&copy, ends_at_once))
    .await;

  assert_eq!(handed, None);
  let UpdateState::InstallFailed { failure, .. } = copy.updates.state() else {
    panic!("failed: {:?}", copy.updates.state());
  };
  assert_eq!(failure.exit_code, Some(3));
}

#[tokio::test]
async fn update_install_without_a_ready_update_does_nothing() {
  let copy = Copy::installed(&super::nobody());
  let runners = no_runners();

  assert_eq!(copy.updates.restart_plan(&runners), RestartPlan::NothingReady);
  let handed = copy
    .updates
    .install(&runners, RunsAtWork::WaitForThem, &installer(&copy, waits))
    .await;

  assert_eq!(handed, None);
  assert_eq!(backups(copy.dir.path()).expect("the backups"), Vec::new());
}

/// Runs of tasks `keys` at work in project Kontur, as its runner reports them.
fn at_work(runners: &Runners, root: &Path, keys: &[&str]) {
  let runs = keys
    .iter()
    .map(|key| RunWork {
      run: RunId::generate(),
      task: TaskId::generate(),
      task_key: (*key).to_owned(),
      task_title: "Sign in with GitHub".to_owned(),
      step: "Implement".to_owned(),
      agent: "Builder".to_owned(),
      provider: AgentProvider::ClaudeCodeCli,
      model: "claude-opus-5-5".to_owned(),
      state: WorkState::Working,
    })
    .collect();
  let work = WorkSummary {
    agent_slots: NonZeroU32::new(3).expect("slots"),
    runs,
    slot_waits: Vec::new(),
  };
  runners.report_work_for_test(root, "Kontur", work);
}

/// Waits until `done` says the state is there.
async fn state_when(updates: &AppUpdates, done: impl Fn(&UpdateState) -> bool) {
  let mut changes = updates.subscribe();
  tokio::time::timeout(Duration::from_secs(10), changes.wait_for(|state| done(state)))
    .await
    .expect("the state in time")
    .expect("the updates");
}

fn waits_for(state: &UpdateState, keys: &[&str]) -> bool {
  matches!(state, UpdateState::Installing { step: InstallStep::WaitingForRuns(runs), .. } if runs == keys)
}

#[tokio::test]
async fn update_install_with_runs_at_work_asks_and_waits_for_them_to_end() {
  let (copy, _stand) = ready_copy().await;
  let runners = no_runners();
  let repo = tempfile::tempdir().expect("a folder");
  at_work(&runners, repo.path(), &["KON-5", "KON-7"]);
  let RestartPlan::Ask { runs, .. } = copy.updates.restart_plan(&runners) else {
    panic!("asks: {:?}", copy.updates.restart_plan(&runners));
  };
  assert_eq!(runs.len(), 2);

  let install = tokio::spawn({
    let (updates, runners, installer) = (copy.updates.clone(), runners.clone(), installer(&copy, waits));
    async move { updates.install(&runners, RunsAtWork::WaitForThem, &installer).await }
  });
  state_when(&copy.updates, |state| waits_for(state, &["KON-5", "KON-7"])).await;
  at_work(&runners, repo.path(), &["KON-7"]);
  state_when(&copy.updates, |state| waits_for(state, &["KON-7"])).await;
  assert_eq!(
    backups(copy.dir.path()).expect("the backups"),
    Vec::new(),
    "nothing installs yet"
  );
  at_work(&runners, repo.path(), &[]);

  let handed = install.await.expect("the install");
  assert_eq!(
    handed.map(|handed| handed.end),
    Some(SessionEnd::Clean),
    "it installs once they ended"
  );
  assert_eq!(backups(copy.dir.path()).expect("the backups"), [version(RUNNING)]);
}

#[tokio::test]
async fn update_install_pressed_while_it_waits_asks_again_and_stops_the_runs_on_consent() {
  let (copy, _stand) = ready_copy().await;
  let runners = no_runners();
  let repo = tempfile::tempdir().expect("a folder");
  at_work(&runners, repo.path(), &["KON-5"]);
  let install = tokio::spawn({
    let (updates, runners, installer) = (copy.updates.clone(), runners.clone(), installer(&copy, waits));
    async move { updates.install(&runners, RunsAtWork::WaitForThem, &installer).await }
  });
  state_when(&copy.updates, |state| waits_for(state, &["KON-5"])).await;

  let RestartPlan::Ask { update, runs } = copy.updates.restart_plan(&runners) else {
    panic!("asks again: {:?}", copy.updates.restart_plan(&runners));
  };
  assert_eq!((update.version, runs.len()), (version(NEWER), 1));
  let again = installer(&copy, waits);
  assert_eq!(
    copy.updates.install(&runners, RunsAtWork::WaitForThem, &again).await,
    None,
    "waiting again changes nothing"
  );
  assert!(
    waits_for(&copy.updates.state(), &["KON-5"]),
    "{:?}",
    copy.updates.state()
  );
  assert_eq!(
    copy.updates.install(&runners, RunsAtWork::StopThem, &again).await,
    None,
    "the waiting install stops them"
  );

  let handed = tokio::time::timeout(Duration::from_secs(30), install)
    .await
    .expect("it no longer waits for the run")
    .expect("the install");
  assert!(handed.is_some(), "the installer took over: {:?}", copy.updates.state());
  assert_eq!(backups(copy.dir.path()).expect("the backups"), [version(RUNNING)]);
}

/// ORC-583: a state set for testing shows in place of the real one, installs nothing, and leaves the real state as it
/// was until it shows again.
#[tokio::test]
async fn update_install_a_state_set_for_testing_installs_nothing() {
  let (copy, _stand) = ready_copy().await;
  let real = copy.updates.state();
  let changes = copy.updates.subscribe();
  let set = UpdateState::WaitingForRuns {
    update: ReadyUpdate {
      version: version("9.9.9"),
      notes: String::new(),
      critical: true,
      installer: PathBuf::new(),
    },
    runs: vec!["KON-5".to_owned()],
  };
  let updated = JustUpdated {
    version: version("9.9.8"),
    notes: "Runs start faster.".to_owned(),
  };
  copy.updates.set_for_testing(set.clone(), Some(updated.clone()));
  assert!(
    changes.has_changed().expect("the state is followed"),
    "what follows it wakes"
  );
  assert_eq!(
    (copy.updates.state(), copy.updates.just_updated()),
    (set.clone(), Some(updated))
  );

  let handed = copy
    .updates
    .install(&no_runners(), RunsAtWork::StopThem, &installer(&copy, cannot_start))
    .await;
  assert_eq!(handed, None);
  assert_eq!(copy.updates.state(), set, "no install began");
  assert!(
    backups(copy.dir.path()).expect("the backups").is_empty(),
    "nothing was copied aside"
  );
  copy.updates.set_runs_at_work(vec!["KON-6".to_owned()]);
  assert_eq!(copy.updates.state(), set, "the real state changes behind it");

  copy.updates.show_real_state();
  assert!(!copy.updates.is_set_for_testing());
  assert_eq!(copy.updates.just_updated(), None);
  let UpdateState::WaitingForRuns { update, runs } = copy.updates.state() else {
    panic!("the real ready update, waiting for the run: {:?}", copy.updates.state());
  };
  assert_eq!((Some(&update), runs), (real.ready(), vec!["KON-6".to_owned()]));
}
