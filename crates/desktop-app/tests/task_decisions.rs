//! The decisions a task's Summary waits for, headless at 1440 × 900 on the QA seed's `task-states` and
//! `decisions` scenarios: the plan review with its versions and the note Request changes needs, raising
//! the limit with its read-back, a daily budget's stop that only Resume continues, the decision after a
//! step's retries, and Ready for review with Merge into main merging, refused at a conflict, unavailable
//! without a branch to merge into, and merged. No runner is connected here (its program does not exist):
//! each decision the runner carries out comes back refused, in so many words. Needs `git`.

#[path = "../examples/seed/kontur/mod.rs"]
#[allow(dead_code, reason = "the seed's helpers the other scenarios use")]
mod kontur;
mod support;

use std::path::{Path, PathBuf};

use orchester_app::{
  routes::{ProjectPage, Route, TaskTab, TaskView},
  ui::icon::Icon,
};
use orchester_core::{
  account::{AccountCli, AccountFolder, AgentAccount},
  connection::{
    AgentConnection, AgentProvider, CatalogSource, CheckResult, ConnectionAccount, ConnectionCheck, ModelCatalog,
    SignIn,
  },
  id::{AgentAccountId, ConnectionId, ProjectId},
  name::Name,
  store::ConnectionStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use support::{Data, Window};
use tempfile::TempDir;
use tokio::runtime::Runtime;

/// Why a decision waits: no runner is connected to carry it out (the header's own words).
const OFFLINE: &str = "The runner is not connected; the run's controls return once it is online.";

/// The window on a seeded scenario's project, and the seeded tasks: key and state.
struct Page {
  window: Window,
  project: ProjectId,
  tasks: Vec<(String, &'static str)>,
}

impl Page {
  fn seeded(scenario: &str) -> Self {
    Self::seeded_with(scenario, |_, _| None)
  }

  /// `task-states` with its Codex connection (on Default, its program a file that is never run) and, with
  /// `other_account`, a second Codex connection, *Codex · Work*, on account Work, whose folder exists.
  fn with_codex(other_account: bool) -> Self {
    Self::with_codex_in("task-states", other_account)
  }

  /// [`Self::with_codex`] on `scenario`.
  fn with_codex_in(scenario: &str, other_account: bool) -> Self {
    Self::seeded_with(scenario, |dir, repo| {
      let Some(repo) = repo else {
        let program = dir.join("codex.exe");
        std::fs::write(&program, "").expect("the stand-in program");
        return Some(program);
      };
      if !other_account {
        return None;
      }
      let folder = dir.join("accounts").join("work");
      std::fs::create_dir_all(&folder).expect("the account's folder");
      let now = Timestamp::now();
      let account = AgentAccount::new(
        AgentAccountId::generate(),
        AccountCli::Codex,
        Name::new("Work").expect("a name"),
        AccountFolder::Created(folder),
        now,
      )
      .expect("an account");
      let mut connection = AgentConnection::new(
        ConnectionId::generate(),
        Name::new("Codex · Work").expect("a name"),
        AgentProvider::CodexCli,
        dir.join("codex.exe"),
        now,
      )
      .expect("a connection");
      // Choosing an account drops the connection's check, which is then of the account.
      connection
        .set_account(ConnectionAccount::of(&account))
        .expect("a Codex account");
      connection
        .record_check(ConnectionCheck {
          at: now,
          result: CheckResult::Available {
            version: "0.139.0".to_owned(),
            sign_in: SignIn::SignedIn,
            catalog: ModelCatalog::new(CatalogSource::Provider, Vec::new()),
          },
        })
        .expect("a check");
      ProjectDb::open(repo)
        .expect("the project database")
        .create_connection(&connection.into_connection())
        .expect("the connection is saved");
      None
    })
  }

  /// Seeds `scenario`; `prepare` is called with the temporary folder before seeding (no repository yet), and
  /// returns the fake Codex program to seed with, then again with the repository once seeded.
  fn seeded_with(scenario: &str, prepare: impl Fn(&Path, Option<&Path>) -> Option<PathBuf>) -> Self {
    let dir = TempDir::new().expect("a temporary folder");
    let request = kontur::SeedRequest {
      data_dir: dir.path().to_path_buf(),
      repo: dir.path().join(format!("repo-{scenario}")),
      scenario: kontur::scenarios::named(scenario).expect("a known scenario"),
      fake_codex: prepare(dir.path(), None),
      now: Timestamp::now(),
    };
    let seeded = kontur::seed(&request).expect("the scenario seeds");
    prepare(dir.path(), Some(&seeded.repo));
    let data = Data {
      dir,
      runtime: Runtime::new().expect("tokio runtime"),
    };
    let launch = data.launch();
    let window = Window::of(data, launch);
    let tasks = seeded.tasks.into_iter().map(|task| (task.key, task.state)).collect();
    Self {
      window,
      project: seeded.project,
      tasks,
    }
  }

  /// Opens the Summary of the task left in `state` and waits until `ready` is shown.
  fn open(&mut self, state: &str, ready: &str) {
    let (key, _) = self
      .tasks
      .iter()
      .find(|(_, seeded)| *seeded == state)
      .unwrap_or_else(|| panic!("a task {state}"))
      .clone();
    let page = ProjectPage::Task(key.parse().expect("a task key"), TaskView::of(TaskTab::Summary));
    self.window.navigation().go(Route::Project(self.project, page));
    self
      .window
      .until(&format!("{ready} on {key}"), |window| window.texts(ready).is_some());
  }

  fn text(&mut self, id: &str) -> String {
    self
      .window
      .texts(id)
      .unwrap_or_else(|| panic!("{id} is shown"))
      .join(" ")
  }

  fn has(&mut self, id: &str) -> bool {
    self.window.tree.get_element_by_id(id).is_some()
  }

  fn click(&mut self, id: &str) {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .unwrap_or_else(|| panic!("{id} is shown"))
      .click();
    self.window.pass();
  }

  fn set(&mut self, id: &str, value: &str) {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.as_text_input())
      .unwrap_or_else(|| panic!("{id} is a text input"))
      .set_value(value);
    self.window.pass();
  }

  fn value(&mut self, id: &str) -> String {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.as_text_input())
      .unwrap_or_else(|| panic!("{id} is a text input"))
      .value()
  }

  /// The labels of the select `id`.
  fn options(&mut self, id: &str) -> Vec<String> {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.as_select())
      .unwrap_or_else(|| panic!("{id} is a select"))
      .labels()
      .iter()
      .map(ToString::to_string)
      .collect()
  }

  fn choose(&mut self, id: &str, label: &str) {
    let mut select = self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.as_select())
      .unwrap_or_else(|| panic!("{id} is a select"));
    let index = select
      .labels()
      .iter()
      .position(|option| &**option == label)
      .unwrap_or_else(|| panic!("an option {label}"));
    select.commit(index);
    self.window.pass();
  }

  /// Whether the button `id` shows the lock of an unavailable action in place of its icon.
  fn locked(&mut self, id: &str) -> bool {
    self
      .window
      .tree
      .get_element_by_id(&format!("{id}-icon"))
      .and_then(|icon| icon.text_content().map(str::to_owned))
      .is_some_and(|glyph| glyph == Icon::Lock.glyph().to_string())
  }

  /// Waits until the element `id` says `words`.
  fn until_says(&mut self, id: &str, words: &str) {
    self.window.until(&format!("{id} to say {words}"), |window| {
      window.texts(id).is_some_and(|texts| texts.join(" ").contains(words))
    });
  }
}

#[test]
fn decision_plan_review_lists_versions_and_request_changes_needs_a_note() {
  let mut page = Page::seeded("task-states");
  page.open("plan v2 waiting", "plan-review");
  assert_eq!(
    page.text("plan-review-title"),
    "Plan v2 · Back off when GitHub rate-limits the sync"
  );
  assert_eq!(
    page.options("plan-review-version"),
    ["v2 · latest", "v1 · changes requested"]
  );
  let changed = page.text("plan-review-changed");
  assert!(
    changed.starts_with("Changed since v1, from your note “Don't drop messages while we wait"),
    "{changed}"
  );
  let key = key_of(&page, "plan v2 waiting");
  assert_eq!(
    page.text("plan-review-criteria"),
    format!("Acceptance criteria of {key} 1 The change is covered by tests. 2 Nothing changes for existing users.")
  );
  assert!(page.has("task-stages"), "the run's stages in place of its step rows");
  assert!(!page.has("task-steps"), "no step rows");
  assert!(!page.has("task-brief"), "the plan's card holds what the task asks for");
  page.choose("plan-review-version", "v1 · changes requested");
  page.until_says("plan-review-title", "Plan v1");
  assert_eq!(
    page.text("plan-review-changed"),
    "This is v1, an earlier plan. Plan v2 waits for your decision."
  );
  assert_eq!(
    page.text("plan-review-approve-hint"),
    "Switch to v2 to approve.",
    "approving decides the version that waits"
  );
  page.choose("plan-review-version", "v2 · latest");
  page.until_says("plan-review-title", "Plan v2");
  page.set("plan-review-note", "Show how many messages wait.");
  assert_eq!(
    page.text("plan-review-approve-hint"),
    "Clear the note to approve, or request changes."
  );
  // Deciding needs the runner, which is not connected here: both wait, and say why.
  assert!(page.locked("plan-approve"));
  assert!(page.locked("plan-request-changes"));
  assert_eq!(page.text("plan-review-offline"), OFFLINE);
  page.open("plan v1 waiting", "plan-review");
  assert_eq!(page.options("plan-review-version"), ["v1 · latest"]);
  assert!(!page.has("plan-review-changed"));
  assert_eq!(
    page.text("plan-review-approve-hint"),
    "Approving starts 03 Implement with Builder."
  );
}

#[test]
fn decision_plan_limit_offers_the_reset_and_no_other_account_without_one() {
  let mut page = Page::with_codex(false);
  page.open("stopped at its account's plan limit", "plan-limit-choice");
  let notice = page.text("task-notice");
  assert!(notice.starts_with("Stopped at Default's plan limit"), "{notice}");
  assert_eq!(page.text("plan-limit-title"), "Continue this run");
  assert!(page.text("plan-limit-account").starts_with("Default"));
  let windows = page.text("plan-limit-windows");
  assert!(
    windows.contains("5-hour") && windows.contains("limit reached"),
    "{windows}"
  );
  let wait = page.text("plan-limit-wait-title");
  assert_eq!(wait, "Wait for Default, then continue");
  // The seed has one Codex connection, on Default: no other account to continue on.
  assert_eq!(
    page.text("plan-limit-other-reason"),
    "No other codex connection runs on another account that can start now. Add one with another account in \
     Connections."
  );
  assert!(!page.has("plan-limit-connection"));
  // Waiting is the default: *Continue at …* confirms it, the time alone when the reset is today.
  let continue_at = page.text("plan-limit-continue");
  assert!(continue_at.starts_with("Continue at "), "{continue_at}");
  assert!(
    !page.locked("plan-limit-continue"),
    "a person's confirmation, not a locked label"
  );
  assert_eq!(page.text("plan-limit-offline"), OFFLINE);
  let accounts = page.text("plan-limit-accounts");
  assert!(
    accounts.contains("account Default · stopped at its plan limit"),
    "{accounts}"
  );
  assert!(page.text("task-status").contains("Stopped at plan limit"));
  assert!(page.text("task-this-run-account-value").ends_with("limit reached"));
}

#[test]
fn decision_plan_limit_a_new_run_waits_for_its_accounts_reset_before_its_agent_starts() {
  let mut page = Page::with_codex_in("accounts", false);
  page.open("waiting for its account's plan limit", "plan-limit-waiting");
  let title = page.text("plan-limit-waiting-title");
  assert!(
    title.starts_with("Waiting for Default's plan limit · starts at "),
    "{title}"
  );
  let body = page.text("plan-limit-waiting-body");
  assert!(
    body.contains("starts 01 Research") && body.contains("holds no agent slot"),
    "{body}"
  );
  assert!(
    page.has("plan-limit-waiting-none"),
    "no other account's connection to start on"
  );
  assert!(!page.has("plan-limit-choice"), "not a stop of a running agent");
}

#[test]
fn decision_plan_limit_a_waiting_run_offers_start_now_on_another_account() {
  let mut page = Page::with_codex_in("accounts", true);
  page.open("waiting for its account's plan limit", "plan-limit-waiting");
  assert_eq!(page.options("plan-limit-waiting-connection"), ["Codex · Work"]);
  // Work is not among this window's accounts, so the connection names it.
  assert_eq!(page.text("plan-limit-start-now"), "Start now on Codex · Work");
}

#[test]
fn decision_plan_limit_continues_on_another_accounts_connection_only_when_chosen() {
  let mut page = Page::with_codex(true);
  page.open("stopped at its account's plan limit", "plan-limit-choice");
  assert!(!page.has("plan-limit-other-reason"), "another account can start now");
  assert!(!page.has("plan-limit-connection"), "waiting is the default");
  page.click("plan-limit-other");
  page.window.until("the other connection", |window| {
    window.texts("plan-limit-connection").is_some()
  });
  assert_eq!(page.options("plan-limit-connection"), ["Codex · Work"]);
  assert_eq!(page.text("plan-limit-continue"), "Continue now on Codex · Work");
  assert_eq!(page.text("plan-limit-offline"), OFFLINE);
  page.click("plan-limit-wait");
  page.until_says("plan-limit-continue", "Continue at ");
  assert!(!page.has("plan-limit-connection"));
}

#[test]
fn decision_raise_reads_back_the_new_limit_and_asks_the_runner() {
  let mut page = Page::seeded("task-states");
  page.open("stopped at its token limit", "raise-limit");
  assert_eq!(
    page.text("raise-limit-tokens"),
    "Used 500k of 500k · Implement stopped mid-step"
  );
  assert!(page.has("raise-limit-meter"));
  assert_eq!(page.value("raise-limit-input"), "800,000");
  assert_eq!(
    page.text("raise-limit-field-hint"),
    "= 800k tokens · 300k more than now"
  );
  assert_eq!(page.text("raise-limit-continue-label"), "Continue with 800k tokens");
  page.set("raise-limit-input", "400,000");
  page.until_says(
    "raise-limit-input-note",
    "Enter more than the 500k tokens this run already used.",
  );
  // No maximum: far above any project's maximum is read as it is, up to what the project database holds.
  page.set("raise-limit-input", "90,000,000");
  page.until_says("raise-limit-field-hint", "= 90M tokens · 89.5M more than now");
  page.set("raise-limit-input", "10,000,000,000,000,000,000");
  page.until_says(
    "raise-limit-input-note",
    "Enter a smaller number: a budget can be at most 9,223,372,036,854,775,807 tokens.",
  );
  assert!(page.locked("raise-limit-continue"), "continuing waits for a runner");
  assert_eq!(page.text("raise-limit-offline"), OFFLINE);
  // Removing the limit continues the run without it, which needs the runner too.
  assert!(page.locked("raise-limit-remove"), "removing waits for a runner");
  assert_eq!(
    page.text("raise-limit-remove-meaning"),
    "Remove the limit continues the run with no token limit."
  );
  // An empty field names no new limit: Remove the limit is how the run goes on without one.
  page.set("raise-limit-input", "");
  page.until_says("raise-limit-input-note", "Enter a token budget.");
  assert!(page.has("raise-limit-tokens-by-step"));
  assert_eq!(page.text("raise-limit-step-research"), "40k tokens · Explorer");
}

#[test]
fn decision_daily_budget_allows_only_resume() {
  let mut page = Page::seeded("task-states");
  page.open("stopped by the daily budget", "task-notice");
  page
    .window
    .until("the run controls", |window| window.texts("task-resume-run").is_some());
  assert!(
    !page.has("raise-limit"),
    "raising the run's own limit does not lift the day's"
  );
  assert!(!page.has("raise-limit-continue"));
  assert!(page.has("task-steps"), "the usual Summary");
  assert!(page.locked("task-resume-run"), "Resume waits for a runner");
  assert_eq!(page.text("task-action-reasons"), "Runner not connected");
}

#[test]
fn decision_after_retries_allows_a_retry_returns_with_a_note_and_stops() {
  let mut page = Page::seeded("decisions");
  page.open("waits after Review's retries", "task-retry-decision");
  assert_eq!(page.text("task-retry-decision-title"), "Decide how the run continues");
  assert_eq!(page.text("task-retry-reported-head"), "What Review reported");
  assert!(page.has("task-retry-report-text"));
  assert!(page.has("task-retry-attempt"), "the latest attempt's steps");
  let options = page.options("task-return-step");
  assert!(options.contains(&"03 Implement · default".to_owned()), "{options:?}");
  let title = page.text("task-notice-title");
  assert!(
    title.starts_with("Review still fails after ") && (title.ends_with(" retries") || title.ends_with(" retry")),
    "{title}"
  );
  assert!(page.text("task-notice").contains("sent the work back to Implement"));
  // Each decision goes to the runner, which is not connected here: they wait, and say why.
  page.set("task-return-note", "Sample more than ten delays.");
  for button in ["task-allow-retry", "task-return-with-note", "task-stop-run"] {
    assert!(page.locked(button), "{button}");
  }
  assert_eq!(page.text("task-retry-offline"), OFFLINE);
}

#[test]
fn decision_ready_for_review_merges_into_main() {
  let mut page = Page::seeded("decisions");
  page.open("ready for review, merges cleanly", "task-merge");
  assert_eq!(page.text("task-notice-title"), "Ready for your review");
  assert!(page.locked("task-review-pull-request"));
  assert_eq!(
    page.text("task-review-pull-request-reason"),
    "Review PR needs a pull request, which no pipeline step opens yet."
  );
  assert!(page.locked("task-ask-agent"), "Ask agent waits for the composer");
  page.until_says("task-ready-changes-figure", "1 file");
  assert!(page.has("task-ready-check-review"), "Review verified the work");
  assert!(page.has("task-run-details"));
  page.click("task-merge");
  page.until_says("task-notice-title", "Merged into main");
  let body = page.text("task-notice");
  assert!(body.contains("main got merge commit"), "{body}");
  assert!(!page.has("task-merge"), "a merged run has nothing to merge");
}

#[test]
fn decision_merge_stops_at_a_conflict_and_says_why() {
  let mut page = Page::seeded("decisions");
  page.open("ready for review, conflicts with main", "task-merge");
  page.click("task-merge");
  page.until_says("task-notice-title", "Not merged into main");
  let body = page.text("task-notice");
  assert!(body.contains("conflict with your branch"), "{body}");
  assert_eq!(
    page.text("task-merge-conflicts"),
    "src/webhooks/sender.rs src/webhooks/settings.rs"
  );
  assert_eq!(page.text("task-merge-conflicts-head"), "Conflicting files 2");
  assert!(!page.locked("task-merge"), "it can be merged again");
}

#[test]
fn decision_merge_is_unavailable_without_a_branch_and_merged_says_where() {
  let mut page = Page::seeded("decisions");
  page.open("ready for review, no branch to merge into", "task-merge");
  assert!(page.locked("task-merge"));
  assert_eq!(
    page.text("task-merge-reason"),
    "Merge needs the branch your checkout had when the run started, which this run didn't record."
  );
  let mut page = Page::seeded("task-states");
  page.open("merged", "task-notice");
  assert_eq!(page.text("task-notice-title"), "Merged into main");
  assert!(page.has("task-stages"));
  assert!(!page.has("task-merge"));
  assert!(!page.has("task-review-pull-request"));
}

/// The key of the task seeded in `state`.
fn key_of(page: &Page, state: &str) -> String {
  page
    .tasks
    .iter()
    .find(|(_, seeded)| *seeded == state)
    .map(|(key, _)| key.clone())
    .unwrap_or_else(|| panic!("a task {state}"))
}
