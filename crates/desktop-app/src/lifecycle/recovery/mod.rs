//! The runs the previous session left interrupted, on their project's Overview (`Startup after a crash`
//! `JSRBO`, `AgD9q`): no startup screen or dialog. A compact notice says how many runs were interrupted;
//! *Review runs* opens their list in place, where the person continues all of them from their
//! checkpoints, or none. While they continue, the list stays; afterwards it goes, and a run that did not
//! continue is named with why. After *Don't continue* the notice goes and the runs stay interrupted:
//! each task page offers to continue its run. After "Wait for agents to save" nothing is shown. What was
//! read and decided is `orchester_workspace::recovery::Recovery`, the session's.

mod table;

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, StreamEmitter},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::ProjectId, project::TaskKey};
use orchester_ipc::SessionEnd;
use orchester_workspace::recovery::{Continuation, Decision, InterruptedRun, Reading, Recovery, RecoveryState};

use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{ProjectPage, Route, TaskTab, TaskView},
  services::Services,
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    message::message_text,
    notice::{Notice, NoticeTone},
    pressable::pressable,
  },
};

/// The element id of the notice.
pub const NOTICE_ID: &str = "recovery-notice";

/// The notice of `project`'s interrupted runs, if it has any to offer.
pub struct RecoveryNotice {
  /// The list is open (*Review runs*), or closed again (*Hide*).
  open: Signal<bool>,
}

impl Component for RecoveryNotice {
  type Props = ShownProject;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let shown = ctx.props::<ShownProject>().clone();
    let services = Services::of(ctx);
    let state = follow(ctx, &services.recovery);
    let project = shown.open.project.id();
    let runs = state.runs_of(project);
    match (&state.reading, state.decision(project)) {
      (Reading::Failed(reason), _) => unread(ctx, &message_text(ctx, reason)),
      (Reading::Read(_), Decision::Undecided) if !runs.is_empty() && !self.open.get() => {
        self.compact(ctx, project, &runs, &services)
      }
      (Reading::Read(_), decision @ (Decision::Undecided | Decision::Continuing)) if !runs.is_empty() => {
        let continuing = decision == Decision::Continuing;
        self.list(ctx, (&shown, &state), &runs, continuing)
      }
      (Reading::Read(_), Decision::Continued(continuation)) if !continuation.not_continued.is_empty() => {
        not_continued(ctx, project, &continuation)
      }
      _ => Element::new(),
    }
  }
}

impl RecoveryNotice {
  /// *2 runs were interrupted when Orchester closed*, with *Review runs* and *Continue all*.
  fn compact(&self, ctx: &Ctx, project: ProjectId, runs: &[InterruptedRun], services: &Services) -> Element {
    let open = self.open.clone();
    let count = runs.len();
    Notice::new(NOTICE_ID, NoticeTone::Warning, title(ctx, count))
      .body(ctx.t(plural(
        count,
        "lifecycle.recovery.summary.one",
        "lifecycle.recovery.summary.other",
      )))
      .action(
        ActionButton::new(
          "recovery-review",
          ButtonKind::Secondary,
          ctx.t("lifecycle.recovery.reviewRuns"),
        )
        .on_click(move || open.set(true)),
      )
      .action(
        ActionButton::new(
          "recovery-continue-all",
          ButtonKind::Primary,
          ctx.t("lifecycle.recovery.continueAll"),
        )
        .icon(Icon::Play)
        .on_click(continue_all(services, project)),
      )
      .into()
  }

  /// The list in place (`Interrupted runs` in `JSRBO`): why, the runs, what is kept, and the decision.
  fn list(
    &self,
    ctx: &mut Ctx,
    (shown, state): (&ShownProject, &RecoveryState),
    runs: &[InterruptedRun],
    continuing: bool,
  ) -> Element {
    let services = Services::of(ctx);
    let project = shown.open.project.id();
    let open = self.open.clone();
    let head = list_head(ctx, state.previous, runs.len()).child(hide(ctx, move || open.set(false)));
    let table = table::runs(ctx, project, runs);
    let note = Text::new(&ctx.t("lifecycle.recovery.note"))
      .variant(ExtraTypography::Prose.role())
      .width(Dimension::Pct(100.0));
    Column::new()
      .id(NOTICE_ID)
      .describe("state", if continuing { "continuing" } else { "open" })
      .width(Dimension::Pct(100.0))
      .padding(SpacingSize::Lg)
      .spacing(SpacingSize::Md)
      .background(PaletteColor::WarningMuted)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Warning))
      .rounded(RadiusSize::Lg)
      .child(head)
      .child(table)
      .child(note)
      .child(decision(ctx, (&services, shown), runs.len(), continuing))
      .into()
  }
}

/// The list's title and why the runs were interrupted.
fn list_head(ctx: &Ctx, previous: SessionEnd, count: usize) -> Row {
  let body = match previous {
    SessionEnd::QuitNow => "lifecycle.recovery.body.quitNow",
    _ => "lifecycle.recovery.body.crashed",
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Start)
    .child(icon(Icon::TriangleAlert, IconSize::Lg, PaletteColor::Warning))
    .child(
      Column::new()
        .flex(1.0)
        .spacing(ExtraSpacing::Tight.role())
        .child(
          Text::new(&title(ctx, count))
            .id(format!("{NOTICE_ID}-title"))
            .variant(ExtraTypography::Strong.role()),
        )
        .child(Text::new(&ctx.t(body)).variant(ExtraTypography::Prose.role())),
    )
}

/// *Open runner log* on the left, *Don't continue* and *Continue all N from checkpoints* on the right;
/// while the runs continue, the primary turns and nothing else can be decided.
fn decision(ctx: &Ctx, (services, shown): (&Services, &ShownProject), count: usize, continuing: bool) -> Row {
  let project = shown.open.project.id();
  let root = shown.open.repo_root.clone();
  let runners = services.runners.clone();
  let row = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      ActionButton::new(
        "recovery-open-log",
        ButtonKind::Secondary,
        ctx.t("lifecycle.recovery.openLog"),
      )
      .icon(Icon::FileText)
      .on_click(move || runners.open_log_of(&root)),
    )
    .child(Spacer::new().flex(1.0));
  let dont = ActionButton::new(
    "recovery-dont-continue",
    ButtonKind::Secondary,
    ctx.t("lifecycle.recovery.dontContinue"),
  );
  let words = |one, other| ctx.t_args(plural(count, one, other), [("count", count.to_string())]);
  if continuing {
    let label = words(
      "lifecycle.recovery.continuing.one",
      "lifecycle.recovery.continuing.other",
    );
    return row
      .child(dont.disabled())
      .child(ActionButton::new("recovery-continue", ButtonKind::Secondary, label).loading());
  }
  let recovery = services.recovery.clone();
  let label = words(
    "lifecycle.recovery.continueFromCheckpoints.one",
    "lifecycle.recovery.continueFromCheckpoints.other",
  );
  row.child(dont.on_click(move || recovery.leave(project))).child(
    ActionButton::new("recovery-continue", ButtonKind::Primary, label)
      .icon(Icon::Play)
      .on_click(continue_all(services, project)),
  )
}

/// *Hide* with its chevron: closes the list back to the compact notice.
fn hide(ctx: &Ctx, close: impl Fn() + Send + Sync + 'static) -> Element {
  pressable(close)
    .id("recovery-hide")
    .padding(Padding::horizontal(SpacingSize::Sm))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Md)
    .child(icon(Icon::ChevronUp, IconSize::Sm, PaletteColor::TextSecondary))
    .child(
      Text::new(&ctx.t("lifecycle.recovery.hide"))
        .variant(ExtraTypography::Prose.role())
        .nowrap(),
    )
    .into()
}

/// *Continue all*: the session's recovery continues the project's runs through its runner.
fn continue_all(services: &Services, project: ProjectId) -> impl Fn() + Send + Sync + 'static {
  let (recovery, runners, servers) = (
    services.recovery.clone(),
    services.runners.clone(),
    services.servers.clone(),
  );
  move || recovery.continue_all(project, &runners, &servers)
}

/// *1 of 2 runs continued*, naming the run that did not continue and why; it stays interrupted.
fn not_continued(ctx: &mut Ctx, project: ProjectId, continuation: &Continuation) -> Element {
  let left = &continuation.not_continued;
  let count = continuation.continued + left.len();
  let title = ctx.t_args(
    "lifecycle.recovery.partial.title",
    [
      ("continued", continuation.continued.to_string()),
      ("count", count.to_string()),
    ],
  );
  let body = match left.as_slice() {
    [one] => ctx.t_args(
      "lifecycle.recovery.partial.one",
      [
        ("task", one.run.task_key.to_string()),
        ("reason", message_text(ctx, &one.reason).to_string()),
      ],
    ),
    several => {
      let keys: Vec<&str> = several.iter().map(|left| &*left.run.task_key).collect();
      let separator = ctx.t("lifecycle.close.waitingNote.separator");
      ctx.t_args("lifecycle.recovery.partial.other", [("tasks", keys.join(&separator))])
    }
  };
  let mut notice = Notice::new(NOTICE_ID, NoticeTone::Warning, title).body(body);
  if let Some(first) = left.first()
    && let Ok(key) = first.run.task_key.parse::<TaskKey>()
  {
    let navigation = Navigation::of(ctx);
    let route = Route::Project(project, ProjectPage::Task(key, TaskView::of(TaskTab::Summary)));
    notice = notice.action(
      ActionButton::new(
        "recovery-open-task",
        ButtonKind::Secondary,
        ctx.t_args("lifecycle.recovery.partial.open", [("task", &*first.run.task_key)]),
      )
      .on_click(move || navigation.go(route.clone())),
    );
  }
  notice.into()
}

/// The interrupted runs could not be read: said, never a silent gap.
fn unread(ctx: &Ctx, reason: &str) -> Element {
  Notice::new(NOTICE_ID, NoticeTone::Warning, ctx.t("lifecycle.recovery.unread.title"))
    .body(ctx.t_args("lifecycle.recovery.unread.body", [("reason", reason)]))
    .into()
}

fn title(ctx: &Ctx, count: usize) -> std::sync::Arc<str> {
  ctx.t_args(
    plural(count, "lifecycle.recovery.title.one", "lifecycle.recovery.title.other"),
    [("count", count.to_string())],
  )
}

/// `one` for one, `other` otherwise.
const fn plural(count: usize, one: &'static str, other: &'static str) -> &'static str {
  if count == 1 { one } else { other }
}

/// The session's recovery; reading it in a render renders again when it changes.
fn follow(ctx: &mut Ctx, recovery: &Recovery) -> RecoveryState {
  let state = ctx.stream(
    recovery.clone(),
    |recovery, emitter: StreamEmitter<RecoveryState, ()>| async move {
      let mut updates = recovery.subscribe();
      loop {
        let state = updates.borrow_and_update().clone();
        if !emitter.emit(state) || updates.changed().await.is_err() {
          return;
        }
      }
    },
  );
  state.state().get().data.unwrap_or_else(|| recovery.state())
}
