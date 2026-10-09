//! The activity slot (handover §3.3): one line for the most important of a runner problem (the runner
//! item says it, `runner`), the newest failure nobody opened or dismissed, the operations under way, or
//! the latest event (`orchester_workspace::activity::bar`). A failure stays on a raised fill with the
//! action that opens its object and a dismiss; a run that waits for the person's decision the same way,
//! with a hand and no dismiss. An operation shows how long it has run, never a percentage; several show
//! how many. The latest event shows its time and opens its object. Outside a project, an event names its
//! project first.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize},
  },
  components::{Row, Text},
  layout::{Alignment, layout_kind::ShrinkLimit},
  node::Element,
};
use orchester_core::project::TaskKey;
use orchester_workspace::{
  activity::{Event, Operation, Outcome, Place, Slot, Target},
  display,
};

use super::{
  facts::{BarAction, Dispatch, ProjectFacts},
  item::{action, dropping, give_way, glyph, item, pressed, trimmed},
};
use crate::{
  routes::{OverviewTab, ProjectPage, Route, TaskList, TaskView},
  shell::clock::{Clock, clock},
  theme::ExtraSpacing,
  ui::{icon::Icon, message::message_text},
};

/// The slot's items for `slot` of the activity log.
pub fn activity(ctx: &mut Ctx, slot: &Slot<'_>, project: Option<&ProjectFacts>, dispatch: &Dispatch) -> Vec<Element> {
  match slot {
    Slot::Empty => Vec::new(),
    Slot::Failure(event) => failure(ctx, event, project, dispatch),
    Slot::Running { newest, count } => vec![running(ctx, newest, *count, project)],
    Slot::Latest(event) => vec![latest(ctx, event, project, dispatch)],
  }
}

/// A failure, or a run that waits for the person's decision.
fn failure(ctx: &Ctx, event: &Event, project: Option<&ProjectFacts>, dispatch: &Dispatch) -> Vec<Element> {
  let decision = matches!(event.outcome(), Outcome::NeedsYou { .. });
  let (state, glyph_icon) = if decision {
    ("needs-you", Icon::Hand)
  } else {
    ("failure", Icon::CircleAlert)
  };
  let text = event_text(ctx, event, project);
  let row = item("status-activity")
    .describe("state", state)
    .flex_shrink(1.0)
    .shrink_order(give_way::ACTIVITY)
    .shrink_limit(ShrinkLimit::Content)
    .background(PaletteColor::SurfaceRaised)
    .rounded(RadiusSize::Md)
    .child(glyph(glyph_icon, PaletteColor::TextPrimary))
    .child(trimmed(&text, PaletteColor::TextPrimary).id("status-activity-text"))
    .child(time(event).shrink_order(give_way::TIMES));
  let mut items = vec![row.into()];
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

/// One operation under way and how long it has run, or how many and the newest.
fn running(ctx: &mut Ctx, newest: &Operation, count: usize, project: Option<&ProjectFacts>) -> Element {
  let title = message_text(ctx, newest.title());
  let text = if count > 1 {
    ctx.t_args(
      "shell.status.activity.several",
      [("count", count.to_string()), ("title", title.to_string())],
    )
  } else {
    ctx.t_args("shell.status.activity.running", [("title", title.to_string())])
  };
  let text = in_project(ctx, &text, newest.project(), project);
  let row = item("status-activity")
    .describe("state", if count > 1 { "several" } else { "running" })
    .flex_shrink(1.0)
    .shrink_order(give_way::ACTIVITY)
    .shrink_limit(ShrinkLimit::Content)
    .child(glyph(Icon::LoaderCircle, PaletteColor::TextMuted))
    .child(trimmed(&text, PaletteColor::TextMuted).id("status-activity-text"));
  if count > 1 {
    return row.into();
  }
  let since = clock(ctx, "status-activity-elapsed", Clock::Since(newest.started()));
  row.child(since).into()
}

/// The latest event and its time; it opens its object where it has one here.
fn latest(ctx: &Ctx, event: &Event, project: Option<&ProjectFacts>, dispatch: &Dispatch) -> Element {
  let glyph_icon = match event.outcome() {
    Outcome::Finished => Icon::Check,
    Outcome::NotDone | Outcome::Failed { .. } => Icon::CircleAlert,
    Outcome::NeedsYou { .. } => Icon::Hand,
  };
  let text = event_text(ctx, event, project);
  let content = Row::new()
    .flex_shrink(1.0)
    .shrink_limit(ShrinkLimit::Content)
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(glyph(glyph_icon, PaletteColor::TextMuted))
    .child(trimmed(&text, PaletteColor::TextMuted).id("status-activity-text"))
    .child(time(event).shrink_order(give_way::TIMES));
  let shrinking = |row: Row| {
    row
      .describe("state", "latest")
      .flex_shrink(1.0)
      .shrink_order(give_way::ACTIVITY)
      .shrink_limit(ShrinkLimit::Content)
  };
  match opening(event.target(), project) {
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
    None => shrinking(item("status-activity")).child(content).into(),
  }
}

/// When the event happened, *14:32*; it drops out whole when the bar gives way.
fn time(event: &Event) -> Text {
  dropping(&display::clock_time(event.at()), PaletteColor::TextMuted)
}

/// An event's words: its title, and a failure's reason.
fn event_text(ctx: &Ctx, event: &Event, project: Option<&ProjectFacts>) -> String {
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
fn in_project(ctx: &Ctx, text: &str, named: Option<&str>, shown: Option<&ProjectFacts>) -> String {
  match (named, shown) {
    (Some(name), None) => ctx
      .t_args("shell.status.activity.inProject", [("project", name), ("text", text)])
      .to_string(),
    _ => text.to_owned(),
  }
}

/// What opening `target` asks for, when it can be opened from here: the runner log of the shown project,
/// or a place of it.
fn opening(target: &Target, project: Option<&ProjectFacts>) -> Option<BarAction> {
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
  let key = |task| -> Option<TaskKey> {
    let tasks = project.tasks.as_ref()?;
    let task = tasks.tasks.iter().find(|candidate| candidate.id() == task)?;
    Some(task.key().clone())
  };
  Some(match place {
    Place::Overview => ProjectPage::Overview(OverviewTab::default()),
    Place::Tasks => ProjectPage::Tasks(TaskList::default()),
    Place::PullRequests => ProjectPage::PullRequests,
    Place::Pipeline(_) => ProjectPage::Pipelines,
    Place::Instruction(_) => ProjectPage::Instructions,
    Place::Skill(_) => ProjectPage::Skills,
    Place::Task(task) => ProjectPage::Task(key(task)?, TaskView::Summary),
    Place::TaskConversation(task) => ProjectPage::Task(key(task)?, TaskView::Conversation),
    Place::AgentProfile(_) => ProjectPage::Agents,
    Place::Tool(_) => ProjectPage::Tools,
    Place::Secrets => ProjectPage::Secrets,
    Place::Connections => ProjectPage::Connections,
    Place::Settings => ProjectPage::Settings,
  })
}
