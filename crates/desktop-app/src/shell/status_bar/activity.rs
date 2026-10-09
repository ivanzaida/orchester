//! The activity slot (handover §3.3): one line for the most important of a runner problem (the runner
//! item says it, `runner`), the newest failure nobody opened or dismissed, the operations under way, or
//! the latest event (`orchester_workspace::activity::bar`). The project's tasks that could not be read
//! are its own unresolved failure, with *Try again*. A failure stays on a raised fill with the action
//! that opens its object and a dismiss; a run that waits for the person's decision the same way, with a
//! hand and no dismiss. One operation shows how long it has run, never a percentage; several show how many
//! and open the *Background operations* popover (`operations`). The latest event shows its time and opens
//! its object. Outside a project, an event names its project first. Words the bar shortens keep their
//! full value in a tooltip.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize},
  },
  components::Row,
  layout::{Alignment, layout_kind::ShrinkLimit},
  node::Element,
};
use orchester_core::project::TaskKey;
use orchester_workspace::{
  activity::{ActivityLog, Event, Operation, Outcome, Place, Slot, Target},
  display,
};

use super::{
  facts::{BarAction, Dispatch, ProjectFacts, ReadFailure},
  item::{action, cut, give_way, glyph, item, label, pressed, time_part},
  operations::{Operations, OperationsProps},
};
use crate::{
  routes::{OverviewTab, ProjectPage, Route, TaskList, TaskView},
  shell::clock::{Clock, clock},
  theme::ExtraSpacing,
  ui::{icon::Icon, message::message_text},
};

/// The slot's items for `slot` of `log`.
pub fn activity(
  ctx: &mut Ctx,
  (slot, log): (&Slot<'_>, &ActivityLog),
  project: Option<&ProjectFacts>,
  dispatch: &Dispatch,
) -> Vec<Element> {
  match slot {
    Slot::Empty => Vec::new(),
    Slot::Failure(event) => failure(ctx, event, project, dispatch),
    Slot::Running { newest, count: 1 } => running(ctx, newest, project),
    Slot::Running { .. } => {
      let props = OperationsProps {
        log: log.clone(),
        project: project.cloned(),
        dispatch: dispatch.clone(),
      };
      vec![ctx.mount_keyed::<Operations>("status-operations", props)]
    }
    Slot::Latest(event) => latest(ctx, event, project, dispatch),
  }
}

/// A failure, or a run that waits for the person's decision.
fn failure(ctx: &mut Ctx, event: &Event, project: Option<&ProjectFacts>, dispatch: &Dispatch) -> Vec<Element> {
  let decision = matches!(event.outcome(), Outcome::NeedsYou { .. });
  let (state, glyph_icon) = if decision {
    ("needs-you", Icon::Hand)
  } else {
    ("failure", Icon::CircleAlert)
  };
  let text = event_text(ctx, event, project);
  let row = raised("status-activity", state)
    .child(glyph(glyph_icon, PaletteColor::TextPrimary))
    .child(cut(ctx, "status-activity-text", &text, PaletteColor::TextPrimary));
  let mut items = vec![row.into(), time(event)];
  let id = event.id();
  if let Some(open) = opening(event.target(), project) {
    let key = match (decision, event.target()) {
      (true, _) => "shell.status.open.decision",
      (
        false,
        Target::Project {
          place: Place::Task(_) | Place::TaskConversation(_),
          ..
        },
      ) => "shell.status.open.task",
      (false, Target::RunnerLog) => "shell.status.runner.openLog",
      (false, _) => "shell.status.open.other",
    };
    let dispatch = dispatch.clone();
    items.push(action("status-activity-open", &ctx.t(key), move || {
      dispatch(BarAction::Resolve(id));
      dispatch(open.clone());
    }));
  }
  // A waiting run's item goes once its task is opened from it or it no longer waits: it has no dismiss.
  if !decision {
    let dispatch = dispatch.clone();
    items.push(
      pressed("status-activity-dismiss", move || dispatch(BarAction::Resolve(id)))
        .describe("a11y_name", ctx.t("shell.status.dismiss"))
        .child(glyph(Icon::X, PaletteColor::TextSecondary))
        .into(),
    );
  }
  items
}

/// The project's tasks could not be read, or not read again (the ones shown are the ones last read),
/// and *Try again*.
pub fn read_failure(ctx: &mut Ctx, failure: &ReadFailure, dispatch: &Dispatch) -> Vec<Element> {
  let (key, reason, state) = match failure {
    ReadFailure::First(reason) => ("shell.status.unread.first", reason, "unread"),
    ReadFailure::Again(reason) => ("shell.status.unread.again", reason, "stale"),
  };
  let text = ctx.t_args(key, [("reason", message_text(ctx, reason).to_string())]);
  let row = raised("status-activity", state)
    .child(glyph(Icon::CircleAlert, PaletteColor::TextPrimary))
    .child(cut(ctx, "status-activity-text", &text, PaletteColor::TextPrimary));
  let dispatch = dispatch.clone();
  vec![
    row.into(),
    action("status-read-again", &ctx.t("shell.status.unread.tryAgain"), move || {
      dispatch(BarAction::ReadAgain);
    }),
  ]
}

/// An activity item on a raised fill, for what the person should look at.
fn raised(id: &str, state: &'static str) -> Row {
  item(id)
    .describe("state", state)
    .flex_shrink(1.0)
    .shrink_order(give_way::ACTIVITY)
    .shrink_limit(ShrinkLimit::Content)
    .background(PaletteColor::SurfaceRaised)
    .rounded(RadiusSize::Md)
}

/// One operation under way, and how long it has run.
fn running(ctx: &mut Ctx, newest: &Operation, project: Option<&ProjectFacts>) -> Vec<Element> {
  let title = message_text(ctx, newest.title());
  let text = ctx.t_args("shell.status.activity.running", [("title", title.to_string())]);
  let text = in_project(ctx, &text, newest.project(), project);
  let row = item("status-activity")
    .describe("state", "running")
    .flex_shrink(1.0)
    .shrink_order(give_way::ACTIVITY)
    .shrink_limit(ShrinkLimit::Content)
    .child(glyph(Icon::LoaderCircle, PaletteColor::TextMuted))
    .child(cut(ctx, "status-activity-text", &text, PaletteColor::TextMuted));
  let since = clock(ctx, "status-activity-elapsed-text", Clock::Since(newest.started()));
  vec![row.into(), time_part("status-activity-elapsed", since)]
}

/// The latest event and its time; it opens its object where it has one here.
fn latest(ctx: &mut Ctx, event: &Event, project: Option<&ProjectFacts>, dispatch: &Dispatch) -> Vec<Element> {
  let glyph_icon = match event.outcome() {
    Outcome::Finished => Icon::Check,
    Outcome::NotDone | Outcome::Failed { .. } => Icon::CircleAlert,
    Outcome::NeedsYou { .. } => Icon::Hand,
  };
  let text = event_text(ctx, event, project);
  let content = Row::new()
    .flex_shrink(1.0)
    .min_width(lurq::node::dimension::Dimension::Px(0.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(glyph(glyph_icon, PaletteColor::TextMuted))
    .child(cut(ctx, "status-activity-text", &text, PaletteColor::TextMuted));
  let row: Element = match opening(event.target(), project) {
    Some(open) => {
      let dispatch = dispatch.clone();
      pressed("status-activity", move || dispatch(open.clone()))
        .describe("state", "latest")
        .flex_shrink(1.0)
        .shrink_order(give_way::ACTIVITY)
        .shrink_limit(ShrinkLimit::Content)
        .child(content)
        .into()
    }
    None => item("status-activity")
      .describe("state", "latest")
      .flex_shrink(1.0)
      .shrink_order(give_way::ACTIVITY)
      .shrink_limit(ShrinkLimit::Content)
      .child(content)
      .into(),
  };
  vec![row, time(event)]
}

/// When the event happened, *· 14:32*; it drops out with the bar's times.
fn time(event: &Event) -> Element {
  time_part(
    "status-activity-time",
    label(
      &format!("· {}", display::clock_time(event.at())),
      PaletteColor::TextMuted,
    ),
  )
}

/// An event's words: its title, and a failure's reason.
pub(super) fn event_text(ctx: &Ctx, event: &Event, project: Option<&ProjectFacts>) -> String {
  let title = message_text(ctx, event.title());
  let text = match event.outcome() {
    Outcome::Failed { reason, .. } => ctx.t_args(
      "shell.status.activity.failure",
      [
        ("title", title.to_string()),
        ("reason", message_text(ctx, reason).to_string()),
      ],
    ),
    Outcome::Finished | Outcome::NotDone | Outcome::NeedsYou { .. } => title,
  };
  in_project(ctx, &text, event.project(), project)
}

/// `text` after its project's name when no project is shown (*Kontur · Run #128 ready for review*).
pub(super) fn in_project(ctx: &Ctx, text: &str, named: Option<&str>, shown: Option<&ProjectFacts>) -> String {
  match (named, shown) {
    (Some(name), None) => ctx
      .t_args("shell.status.activity.inProject", [("project", name), ("text", text)])
      .to_string(),
    _ => text.to_owned(),
  }
}

/// What opening `target` asks for, when it can be opened from here: the runner log of the shown project,
/// or a place of it.
pub(super) fn opening(target: &Target, project: Option<&ProjectFacts>) -> Option<BarAction> {
  let project = project?;
  match target {
    Target::Nothing => None,
    Target::RunnerLog => Some(BarAction::OpenRunnerLog),
    Target::Project { repo_root, place } if *repo_root == project.repo_root => {
      Some(BarAction::Go(Route::Project(project.id, page(*place, project)?)))
    }
    Target::Project { .. } => None,
  }
}

/// The page of the project that shows `place`; a task the tasks read do not have has none.
fn page(place: Place, project: &ProjectFacts) -> Option<ProjectPage> {
  Some(match place {
    Place::Overview => ProjectPage::Overview(OverviewTab::default()),
    Place::Tasks => ProjectPage::Tasks(TaskList::default()),
    Place::PullRequests => ProjectPage::PullRequests,
    Place::Pipeline(_) => ProjectPage::Pipelines,
    Place::Instruction(_) => ProjectPage::Instructions,
    Place::Skill(_) => ProjectPage::Skills,
    Place::Task(task) => ProjectPage::Task(task_key(project, task)?, TaskView::Summary),
    Place::TaskConversation(task) => ProjectPage::Task(task_key(project, task)?, TaskView::Conversation),
    Place::AgentProfile(_) => ProjectPage::Agents,
    Place::Tool(_) => ProjectPage::Tools,
    Place::Secrets => ProjectPage::Secrets,
    Place::Connections => ProjectPage::Connections,
    Place::Settings => ProjectPage::Settings,
  })
}

/// The key of `task`, when the project's tasks read have it.
pub(super) fn task_key(project: &ProjectFacts, task: orchester_core::id::TaskId) -> Option<TaskKey> {
  let tasks = project.tasks.as_ref()?;
  let task = tasks.tasks.iter().find(|candidate| candidate.id() == task)?;
  Some(task.key().clone())
}
