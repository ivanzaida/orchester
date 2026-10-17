//! A wait of an agent of the project in the Orchestrator's conversation (`docs/design/orchestrator-toolset.md`,
//! "Decisions" 5): in the action card's frame (`Action card` `nQIhx`), its head with what the agent waits for, the
//! task and the run, and at its end *Waits* and when it was noticed; what happens with it (the Orchestrator reads and
//! answers it by itself while *Let the Orchestrator answer agents* is on, else it waits for the person); and *Open*
//! its card on the task's Conversation. No Pencil frame draws it yet: it keeps to the action card's look, with the
//! waiting state of a proposal.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, TypographyStyle},
  },
  components::{Row, Text},
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::ProjectId,
  orchestrator::{AgentWait, WaitKind},
  project::TaskKey,
  time::Timestamp,
};
use orchester_workspace::display;

use super::card::{self, State};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route, TaskTab, TaskView},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
  },
};

/// The card of `wait`, kept at `at`, which started an Orchestrator turn by itself when `turned`; in project
/// `project`; `id` names it.
pub(super) fn card(ctx: &mut Ctx, id: &str, (wait, at, turned): (&AgentWait, Option<Timestamp>, bool), project: ProjectId) -> Element {
  let run = wait.number.to_string();
  let named = wait.agent.as_deref().or(wait.step.as_deref());
  let (glyph, title) = match (wait.kind, named) {
    (WaitKind::Question, Some(agent)) => (Icon::MessageCircleQuestion, ("orchestrator.wait.question", agent)),
    (WaitKind::Question, None) => (Icon::MessageCircleQuestion, ("orchestrator.wait.questionAnyone", "")),
    (WaitKind::Message, Some(agent)) => (Icon::MessageSquare, ("orchestrator.wait.message", agent)),
    (WaitKind::Message, None) => (Icon::MessageSquare, ("orchestrator.wait.messageAnyone", "")),
    (WaitKind::Plan, Some(agent)) => (Icon::ScrollText, ("orchestrator.wait.plan", agent)),
    (WaitKind::Plan, None) => (Icon::ScrollText, ("orchestrator.wait.planAnyone", "")),
  };
  let title = ctx.t_args(
    title.0,
    [("agent", title.1), ("task", wait.task.as_str()), ("run", run.as_str())],
  );
  let time = at.map(display::clock).unwrap_or_default();
  let state = State {
    glyph: Icon::Hourglass,
    color: PaletteColor::Warning,
    words: ctx.t_args("orchestrator.wait.noticed", [("time", time.as_str())]),
  };
  let effect = ctx.t(effect(wait, turned));
  card::frame(id, kind_name(wait.kind))
    .describe("automatic", wait.automatic.to_string())
    .child(card::head(id, (glyph, &title), None, state))
    .child(
      Text::new(&effect)
        .id(format!("{id}-effect"))
        .variant(TypographyStyle::Body)
        .width(Dimension::Pct(100.0)),
    )
    .with_children(open_card(ctx, id, project, &wait.task))
    .into()
}

/// What happens with `wait`: the Orchestrator answers it by itself (in the turn it `turned`, or after the turn under
/// way), its own answer raised it so it waits for the person, or the person's switch is off.
const fn effect(wait: &AgentWait, turned: bool) -> &'static str {
  match (wait.kind, wait.automatic, turned) {
    (WaitKind::Plan, true, _) => "orchestrator.wait.effectPlanRead",
    (WaitKind::Plan, false, _) => "orchestrator.wait.effectPlan",
    (_, true, true) => "orchestrator.wait.effectAnswering",
    (_, true, false) => "orchestrator.wait.effectQueued",
    (_, false, _) if wait.own => "orchestrator.wait.effectOwn",
    (_, false, _) => "orchestrator.wait.effectWaits",
  }
}

/// *Open* the task's Conversation, where the wait's card is.
fn open_card(ctx: &mut Ctx, id: &str, project: ProjectId, task: &str) -> Option<Element> {
  let key: TaskKey = task.parse().ok()?;
  let navigation = Navigation::of(ctx);
  let label = ctx.t_args("orchestrator.wait.open", [("task", task)]);
  let route = Route::Project(project, ProjectPage::Task(key, TaskView::of(TaskTab::Conversation)));
  Some(
    Row::new()
      .child(
        ActionButton::new(format!("{id}-open"), ButtonKind::Ghost, label)
          .icon(Icon::ArrowUpRight)
          .on_click(move || navigation.go(route.clone())),
      )
      .into(),
  )
}

/// The name tools read of a wait's kind.
const fn kind_name(kind: WaitKind) -> &'static str {
  match kind {
    WaitKind::Question => "question",
    WaitKind::Message => "message",
    WaitKind::Plan => "plan",
  }
}
