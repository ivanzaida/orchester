//! Flaky tests under Project settings › Services (`09 · Flaky tests` `Tv9VC`; `docs/architecture/agent-testing.md`,
//! "Flaky tests and quarantine"): the crumb back to Services and the head, then the quarantined tests and the others a
//! Test step saw flaky, each with how often it was flaky in the window and when it was last seen, with Unquarantine or
//! Quarantine now ([`rows`]), and the rule they are watched by.
//!
//! The list is read off the UI thread when the page opens, when the project's tasks are saved and after each change
//! (`orchester_workspace::agent_testing::flaky`). Quarantining lets a test's failures pass the Test step, so lurq's
//! synthetic input (an MCP client driving the window) quarantines only under *Let automation change agent
//! permissions*, as `quarantine_test` does; otherwise nothing changes and the row says so. Unquarantine is always
//! taken, as `release_test` is.

mod rows;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
  },
  components::Column,
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  test_step::flaky::{FlakyTest, TestKey, WATCH_AFTER, WATCH_WINDOW_MILLIS},
  time::Timestamp,
};
use orchester_workspace::{
  agent_testing::flaky::{self, QuarantineError},
  background,
  messages::{self, Message},
};

use super::super::{
  TabProps,
  kit::{back, page_head},
  page,
};
use crate::{
  mcp::input_gate::{Guarded, InputGate, input_may_decide},
  navigation::Navigation,
  routes::{ProjectPage, ProjectSettingsTab, Route},
  theme::ExtraSpacing,
  ui::{
    message::message_text,
    state::{load_error, loading},
  },
};

/// A day: the watch window in days, and where yesterday starts.
const DAY_MILLIS: i64 = 24 * 60 * 60 * 1000;

/// The days a test's flaky runs are counted in.
const fn window_days() -> i64 {
  WATCH_WINDOW_MILLIS / DAY_MILLIS
}

/// The page Flaky tests.
pub(super) fn flaky_tests(ctx: &mut Ctx, props: &TabProps) -> Element {
  ctx.mount_keyed::<FlakyTests>("settings-flaky-tests", props.clone())
}

/// The project's flaky and quarantined tests as they were read, with when.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Read {
  tests: Arc<[FlakyTest]>,
  at: Timestamp,
}

impl DevtoolsInspectable for Read {}

/// A test moved to quarantine or back.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Move {
  Quarantine(TestKey),
  Release(TestKey),
}

impl DevtoolsInspectable for Move {}

impl Move {
  const fn test(&self) -> &TestKey {
    match self {
      Self::Quarantine(test) | Self::Release(test) => test,
    }
  }
}

/// The test a row's move or refusal is about, as the page's signals hold it.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Row(TestKey);

impl DevtoolsInspectable for Row {}

/// A move that was made.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Moved;

impl DevtoolsInspectable for Moved {}

type MoveAsk = (PathBuf, Move);
type MoveAction = FutureAction<MoveAsk, Moved, Message>;

/// What the rows' actions need: the move under way or last made, and who may quarantine.
#[derive(Clone)]
struct Moves {
  root: PathBuf,
  /// Whether the rows are the latest read's; while the list is read again they show the read before and act on none.
  current: bool,
  action: MoveAction,
  /// The test of the move under way or last made, whose row shows it loading or why it failed.
  moving: Signal<Option<Row>>,
  /// The test an MCP client's *Quarantine now* was refused for: nothing was quarantined.
  refused: Signal<Option<Row>>,
  gate: Option<InputGate>,
}

impl Moves {
  /// `change` pressed: made unless a move is under way or the list is being read again. Quarantining is a person's,
  /// or automation's under *Let automation change agent permissions*; refused, nothing changes.
  fn press(&self, change: Move) {
    if !self.current || self.action.state().get_untracked().is_pending() {
      return;
    }
    let row = Row(change.test().clone());
    if matches!(change, Move::Quarantine(_)) && !input_may_decide(self.gate.as_ref(), Guarded::PermissionWidening) {
      tracing::warn!("synthetic input asked to quarantine a test; nothing quarantined");
      self.refused.set(Some(row));
      return;
    }
    self.refused.set(None);
    self.moving.set(Some(row));
    self.action.run((self.root.clone(), change));
  }

  /// Whether `test`'s row holds the move under way or last made.
  fn moving(&self, test: &TestKey) -> bool {
    self.moving.get().is_some_and(|Row(moving)| moving == *test)
  }

  /// Whether `test`'s row holds a refused *Quarantine now*.
  fn refused(&self, test: &TestKey) -> bool {
    self.refused.get().is_some_and(|Row(refused)| refused == *test)
  }
}

struct FlakyTests {
  /// Counts reads again: each *Try again* and each move made.
  reread: Signal<u64>,
  action: MoveAction,
  moving: Signal<Option<Row>>,
  refused: Signal<Option<Row>>,
  gate: Option<InputGate>,
  /// What the rule's locked fields show: its runs, its days and what a quarantined test does.
  rule: rows::RuleValues,
}

impl Component for FlakyTests {
  type Props = TabProps;

  fn create(ctx: &mut Ctx) -> Self {
    let outcome = ctx
      .t("project_settings.services.flakyTests.rule.outcomeValue")
      .to_string();
    let page = Self {
      reread: ctx.signal(0),
      action: ctx
        .future_action(|(root, change): MoveAsk| async move { background::run(move || apply(&root, &change)).await? }),
      moving: ctx.signal(None),
      refused: ctx.signal(None),
      gate: ctx.use_context::<InputGate>(),
      rule: rows::RuleValues {
        runs: ctx.signal(WATCH_AFTER.to_string()),
        days: ctx.signal(window_days().to_string()),
        outcome: ctx.signal(outcome),
      },
    };
    // Each move made reads the list again, so the test shows where it stands now.
    let reread = page.reread.clone();
    ctx.watch(&page.action.state(), move |state| {
      if state.is_fulfilled() && state.data.is_some() {
        reread.update(|count| *count += 1);
      }
    });
    page
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<TabProps>().clone();
    let root = props.project.open.repo_root.clone();
    let deps = (root.clone(), props.project.feed.revision.get(), self.reread.get());
    let read = ctx
      .future(deps, |(root, ..)| async move {
        background::run(move || {
          let at = Timestamp::now();
          flaky::read(&root, at)
            .map(|tests| Read {
              tests: tests.into(),
              at,
            })
            .map_err(|error| {
              tracing::error!(%error, "cannot read the project's flaky tests");
              messages::store_error(&error)
            })
        })
        .await?
      })
      .state()
      .get();
    let mut shown = Column::new()
      .id("settings-flaky")
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Roomy.role())
      .child(crumb(ctx, &props))
      .child(page_head(
        "settings-flaky-head",
        &ctx.t("project_settings.services.flaky"),
        Some(&ctx.t_args(
          "project_settings.services.flakyTests.lead",
          [("runs", WATCH_AFTER.to_string()), ("days", window_days().to_string())],
        )),
      ));
    shown = match (&read.data, &read.error) {
      // A read that failed says why instead of the list read before it: its tests may have moved since, so none of
      // its rows is offered to act on.
      (_, Some(error)) if read.is_rejected() => {
        let reread = self.reread.clone();
        shown.child(load_error(
          ctx,
          "settings-flaky-load-error",
          (
            &ctx.t("project_settings.services.flakyTests.loadFailed"),
            &message_text(ctx, error),
          ),
          move || reread.update(|count| *count += 1),
        ))
      }
      // What was read stays while it is read again; its rows act only once the new read is in.
      (Some(listed), _) => {
        let moves = Moves {
          root,
          current: read.is_fulfilled(),
          action: self.action.clone(),
          moving: self.moving.clone(),
          refused: self.refused.clone(),
          gate: self.gate.clone(),
        };
        shown
          .with_children(rows::groups(ctx, &moves, listed))
          .child(rows::rule(ctx, &self.rule))
      }
      _ => shown.child(loading(
        "settings-flaky-loading",
        &ctx.t("project_settings.services.flakyTests.reading"),
      )),
    };
    page::scroll(shown)
  }
}

/// Makes `change` in the project at `root`.
fn apply(root: &std::path::Path, change: &Move) -> Result<Moved, Message> {
  let made = match change {
    Move::Quarantine(test) => flaky::quarantine(root, test, Timestamp::now()).map_err(|error| match error {
      QuarantineError::NotFlaky => Message::new("appMcp.flakyTests.notFlaky")
        .with("test", test.test().to_owned())
        .with("suite", test.suite().as_str().to_owned()),
      QuarantineError::Store(error) => messages::store_error(&error),
    }),
    Move::Release(test) => flaky::release(root, test).map_err(|error| messages::store_error(&error)),
  };
  made.map(|_| Moved).inspect_err(|error| {
    tracing::error!(?error, "cannot move a test to quarantine or back");
  })
}

/// The crumb back to Services with *Flaky tests* after it (`Crumb` `r4dw4X`).
fn crumb(ctx: &mut Ctx, props: &TabProps) -> Element {
  back(
    "settings-flaky-crumb",
    (
      &ctx.t("project_settings.services.tab"),
      Some(&ctx.t("project_settings.services.flaky")),
    ),
    Navigation::of(ctx).to(Route::Project(
      props.project.open.project.id(),
      ProjectPage::Settings(ProjectSettingsTab::Services),
    )),
  )
}
