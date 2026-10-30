//! The bar in the real window follows the accounts and the project's connections: an account renamed is read
//! again, and a connection moved to it shows its plan limits at once, without the project being opened again. Its
//! readings are handed on as a runner would (`Runners::observe_plan_limits`).

use orchester_app::routes::{ProjectPage, Route, TaskList};
use orchester_core::{
  account::{AccountCli, AccountFolder, AgentAccount},
  connection::{
    AgentProvider, ConnectionKind,
    plan_limits::{LimitReading, LimitState, LimitWindow, PlanLimits, ReadingSource},
  },
  id::AgentAccountId,
  name::Name,
  store::ConnectionStore,
  time::Timestamp,
};
use orchester_store::{AppDb, ProjectDb};
use orchester_workspace::{connections::data::set_account_in, tasks::outside::Outside};

use super::{
  kontur::{self, SeedRequest, scenarios},
  support::{Data, Window, reads::Reads},
};

/// What each reading of the accounts takes in the window: one read by the status bar, one by the notifications' host.
const READERS: usize = 2;

/// A Codex reading of `account` with its five-hour window `used` percent full.
fn reading(account: AgentAccountId, used: f64) -> LimitReading {
  LimitReading {
    provider: AgentProvider::CodexCli,
    account: Some(account),
    at: Timestamp::now(),
    limits: PlanLimits {
      windows: vec![LimitWindow::new(300, Some(used), None, LimitState::Normal, false).expect("a window")],
      plan: None,
      credits: None,
      blocked: None,
    },
  }
}

#[test]
fn status_plan_limits_follow_the_accounts_and_the_connections() {
  let reads = Reads::count();
  let data = Data::new();
  let repo = data.dir.path().join("repo-connections");
  let seeded = kontur::seed(&SeedRequest {
    data_dir: data.dir.path().to_path_buf(),
    repo: repo.clone(),
    scenario: scenarios::named("connections").expect("a known scenario"),
    fake_codex: None,
    now: Timestamp::now(),
  })
  .expect("the scenario seeds");
  let folder = data.dir.path().join("codex-client");
  std::fs::create_dir_all(&folder).expect("the account's folder");
  let launch = data.launch();
  let mut window = Window::of(data, launch);
  let workspace = window.launch.workspace().expect("the session started").clone();
  let services = window.launch.services().expect("the session started").clone();
  let default = workspace
    .agent_accounts()
    .expect("the accounts")
    .into_iter()
    .find(|account| account.cli() == AccountCli::Codex && account.is_default())
    .expect("Codex's Default account");
  window
    .navigation()
    .go(Route::Project(seeded.project, ProjectPage::Tasks(TaskList::default())));
  services
    .runners
    .observe_plan_limits(reading(default.id(), 12.0), ReadingSource::Turn);
  window.until("Default's limits", |window| window.text("status-bar").contains("12%"));

  // An account the window was not told of (added to the database, as a test only can: the workspace refuses a
  // folder inside a repository, which the tests' scratch folder is), then renamed as Settings › Accounts does.
  let client = AgentAccount::new(
    AgentAccountId::generate(),
    AccountCli::Codex,
    Name::new("Client").expect("a name"),
    AccountFolder::Created(folder),
    Timestamp::now(),
  )
  .expect("an account");
  AppDb::open(&window.data.database())
    .expect("the application database")
    .add_agent_account(&client)
    .expect("added");
  reads.take();
  workspace
    .rename_agent_account(client.id(), Name::new("Work").expect("a name"))
    .expect("renamed");
  window.until("the accounts read again", |_| reads.total() > 0);
  services
    .runners
    .observe_plan_limits(reading(client.id(), 77.0), ReadingSource::Turn);
  let codex = ProjectDb::open(&repo)
    .expect("the project's database")
    .connections()
    .expect("connections")
    .into_iter()
    .find(|connection| connection.kind() == &ConnectionKind::Agent(AgentProvider::CodexCli))
    .expect("the seed's Codex connection");
  set_account_in(&repo, codex.id(), &client).expect("the connection runs on Client");
  services.changes.record(seeded.project, Outside::Connections);
  window.until("Client's limits in place of Default's", |window| {
    let bar = window.text("status-bar");
    bar.contains("77%") && !bar.contains("12%")
  });

  let renamed = reads.take();
  assert_eq!(
    renamed.get("accounts"),
    Some(&READERS),
    "the rename read the accounts again: {renamed:?}"
  );
  assert_eq!(
    renamed.get("used accounts"),
    Some(&2),
    "the rename and the connection's move read what the project uses: {renamed:?}"
  );
}

#[test]
fn status_accounts_changed_before_the_window_opened_are_read_once_by_each_reader() {
  let reads = Reads::count();
  let data = Data::new();
  let launch = data.launch();
  let workspace = launch.workspace().expect("the session started").clone();
  let folder = data.dir.path().join("codex-client");
  std::fs::create_dir_all(&folder).expect("the account's folder");
  let client = AgentAccount::new(
    AgentAccountId::generate(),
    AccountCli::Codex,
    Name::new("Client").expect("a name"),
    AccountFolder::Created(folder),
    Timestamp::now(),
  )
  .expect("an account");
  AppDb::open(&data.database())
    .expect("the application database")
    .add_agent_account(&client)
    .expect("added");
  workspace
    .rename_agent_account(client.id(), Name::new("Work").expect("a name"))
    .expect("renamed");
  let mut window = Window::of(data, launch);
  let until = std::time::Instant::now() + std::time::Duration::from_millis(500);
  while std::time::Instant::now() < until {
    window.tree.tick_futures();
    window.pass();
    std::thread::sleep(std::time::Duration::from_millis(5));
  }
  let opened = reads.take();
  assert_eq!(opened.get("accounts"), Some(&READERS), "{opened:?}");
}
