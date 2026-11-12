//! What the Agent picker lists for a step (`Agent picker` `Zn8BM` in K4 `CwLfL`): the project's published profiles,
//! then its tools, each with what it runs on and, when it is not ready, why in the words readiness uses. The agent the
//! step has now is always listed, so that nothing stored is hidden: a profile with only a draft, and one that was
//! archived or removed (the picker names it *no longer available*). A step that overrides the model as it was stored
//! says so after the profile's line (*· model: <id>*), which a long line cuts before the CLI and the connection.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::{
  connection::AgentProvider,
  resource::{ResourceKind, Step},
};
use orchester_workspace::{
  pipelines::{
    data::{Assignables, ProfileChoice, ToolChoice},
    fields::{Executor, ModelChoice},
  },
  readiness::{ReadinessStanding, Undecided},
};

use crate::{
  screens::{
    local_hub::words::{missing, undecided},
    team_hub::words::cli_key,
  },
  ui::{
    agent_mark::AgentMark,
    agent_picker::{Group, Item, Mark},
    icon::Icon,
  },
};

/// What the picker lists for a step, and how its field is outlined.
pub(super) struct Choices {
  pub items: Vec<Item>,
  /// What each item runs the step as, in the order of `items`.
  pub executors: Vec<Executor>,
  /// The CLI each item runs on, in the order of `items`, when it is known: a published profile's.
  pub clis: Vec<Option<AgentProvider>>,
  /// The CLI the step's own agent runs on, when it is known.
  pub cli: Option<AgentProvider>,
  /// What stands out about the step's agent.
  pub flag: Option<Flag>,
}

impl Choices {
  /// The item of the step's own agent.
  pub fn current(&self, executor: Executor) -> Option<&Item> {
    self
      .executors
      .iter()
      .position(|candidate| *candidate == executor)
      .map(|at| &self.items[at])
  }
}

/// What is wrong with the step's agent, which outlines the field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Flag {
  /// No agent profile or tool is chosen.
  Unset,
  /// The agent cannot run as it is: it has no published version, is gone, or lacks something here.
  NotReady,
}

/// The profiles and tools of `assignables` for `step`.
pub(super) fn choices(ctx: &Ctx, assignables: &Assignables, step: &Step) -> Choices {
  let current = Executor::of(step);
  let overridden = ModelChoice::of(step).0;
  let model = overridden.as_ref().map(|model| model.as_str());
  let mut items = Vec::new();
  let mut executors = Vec::new();
  let mut clis = Vec::new();
  for profile in &assignables.profiles {
    let executor = Executor::Profile(profile.id);
    let own = if current == executor { model } else { None };
    items.push(profile_item(ctx, assignables, profile, own));
    executors.push(executor);
    clis.push(profile.provider);
  }
  let unlisted = unlisted(ctx, assignables, current, model);
  if let Some(item) = unlisted.clone().filter(|item| item.group == Group::Profiles) {
    items.push(item);
    executors.push(current);
    clis.push(None);
  }
  for tool in &assignables.tools {
    items.push(tool_item(ctx, assignables, tool));
    executors.push(Executor::Tool(tool.id));
    clis.push(None);
  }
  if let Some(item) = unlisted.filter(|item| item.group == Group::Tools) {
    items.push(item);
    executors.push(current);
    clis.push(None);
  }
  let cli = match current {
    Executor::Profile(id) => assignables.profile(id).and_then(|profile| profile.provider),
    Executor::Tool(_) | Executor::Unset => None,
  };
  let mut chosen = Choices {
    items,
    executors,
    clis,
    cli,
    flag: None,
  };
  chosen.flag = if current == Executor::Unset {
    Some(Flag::Unset)
  } else {
    chosen
      .current(current)
      .is_some_and(|item| item.warning.is_some())
      .then_some(Flag::NotReady)
  };
  chosen
}

/// The step's own agent when the project does not offer it (`current` with its model override `model`): a profile or
/// tool with only a draft, under its name, or one that is gone (archived or removed), named as the flow's card names
/// it; each says what to do in `warning`, outlined as not ready. `None` when it is offered, or there is none.
fn unlisted(ctx: &Ctx, assignables: &Assignables, current: Executor, model: Option<&str>) -> Option<Item> {
  let (id, group) = match current {
    Executor::Profile(id) if assignables.profile(id).is_none() => (id, Group::Profiles),
    Executor::Tool(id) if assignables.tool(id).is_none() => (id, Group::Tools),
    Executor::Profile(_) | Executor::Tool(_) | Executor::Unset => return None,
  };
  let draft_profile = assignables.draft_profiles.iter().find(|profile| profile.id == id);
  let draft_tool = assignables.draft_tools.iter().find(|tool| tool.id == id);
  let (name, runs_on, warning) = match (group, draft_profile, draft_tool) {
    (Group::Profiles, Some(draft), _) => (
      draft.name.clone(),
      Arc::from(draft.model.as_str()),
      "pipeline_hub.agent.unpublished",
    ),
    (Group::Tools, _, Some(draft)) => (
      draft.name.clone(),
      Arc::from(""),
      "pipeline_hub.stepFlow.toolUnpublished",
    ),
    (Group::Profiles, None, _) => (
      ctx.t("pipeline_hub.stepFlow.profileGone"),
      Arc::from(""),
      "pipeline_hub.stepFlow.chooseAnotherAgent",
    ),
    (Group::Tools, _, None) => (
      ctx.t("pipeline_hub.stepFlow.toolGone"),
      Arc::from(""),
      "pipeline_hub.stepFlow.chooseAnotherAgent",
    ),
  };
  Some(Item {
    key: id.to_string().into(),
    group,
    mark: Mark::Icon(match group {
      Group::Profiles => Icon::Bot,
      Group::Tools => Icon::Terminal,
    }),
    name,
    runs_on,
    model: override_words(ctx, model).filter(|_| group == Group::Profiles),
    suffix: None,
    warning: Some(ctx.t(warning)),
  })
}

/// A published profile: its CLI's mark, and *Claude Code · Work · claude-opus-5-5* (the step's own model after it,
/// `overridden`, when the step overrides the model); why it cannot run here in `warning`, as readiness says it.
pub(super) fn profile_item(
  ctx: &Ctx,
  assignables: &Assignables,
  profile: &ProfileChoice,
  overridden: Option<&str>,
) -> Item {
  let model = profile.model.as_str();
  let runs_on: Arc<str> = match (profile.provider, &profile.connection) {
    (Some(cli), Some(connection)) => ctx.t_args(
      "pipeline_hub.agent.runsOn",
      [
        ("cli", &*ctx.t(cli_key(cli))),
        ("connection", &**connection),
        ("model", model),
      ],
    ),
    (None, Some(connection)) => ctx.t_args(
      "pipeline_hub.agent.runsOnConnection",
      [("connection", &**connection), ("model", model)],
    ),
    (_, None) => model.into(),
  };
  let mark = profile
    .agent
    .as_ref()
    .and_then(AgentMark::of_connection)
    .map_or(Mark::Icon(Icon::Bot), Mark::Agent);
  let warning = match assignables.standing(profile.id) {
    Some(standing) => standing_words(ctx, ResourceKind::AgentProfile, standing),
    None => profile
      .connection
      .is_none()
      .then(|| ctx.t("pipeline_hub.agent.connectionGone")),
  };
  Item {
    key: profile.id.to_string().into(),
    group: Group::Profiles,
    mark,
    name: profile.name.clone(),
    runs_on,
    model: override_words(ctx, overridden),
    suffix: None,
    warning,
  }
}

/// A published tool: its command, then that its exit code decides the step; why it cannot run in `warning`.
fn tool_item(ctx: &Ctx, assignables: &Assignables, tool: &ToolChoice) -> Item {
  let (runs_on, suffix) = match &tool.command {
    Some(command) => (command.clone(), ctx.t("pipeline_hub.agent.toolDecidesAfter")),
    None => (Arc::from(""), ctx.t("pipeline_hub.agent.toolDecides")),
  };
  let warning = if tool.runnable {
    assignables
      .standing(tool.id)
      .and_then(|standing| standing_words(ctx, ResourceKind::Tool, standing))
  } else {
    Some(ctx.t("pipeline_hub.agent.toolUnavailable"))
  };
  Item {
    key: tool.id.to_string().into(),
    group: Group::Tools,
    mark: Mark::Icon(Icon::Terminal),
    name: tool.name.clone(),
    runs_on,
    model: None,
    suffix: Some(suffix),
    warning,
  }
}

/// What readiness says of a profile or tool of `kind` that cannot run here, in the words the Agents hub uses (*Choose a
/// Claude Code connection*, *Set the program path of figma*); `None` when it is ready.
fn standing_words(ctx: &Ctx, kind: ResourceKind, standing: &ReadinessStanding) -> Option<Arc<str>> {
  match standing {
    ReadinessStanding::Ready => None,
    ReadinessStanding::Missing(readiness) => Some(match readiness.missing() {
      [only] => missing(ctx, kind, only),
      many => ctx.t_args("local_hub.line.many", [("count", many.len().to_string())]),
    }),
    ReadinessStanding::Undecided(Undecided::ConnectionGone(_)) => Some(ctx.t("pipeline_hub.agent.connectionGone")),
    ReadinessStanding::Undecided(why) => Some(undecided(ctx, why)),
  }
}

/// *· model: <id>*, what stands after a profile's line when the step overrides the model; it is cut before that line,
/// so a long override never hides the CLI and the connection the step runs on.
fn override_words(ctx: &Ctx, overridden: Option<&str>) -> Option<Arc<str>> {
  overridden.map(|model| ctx.t_args("pipeline_hub.agent.modelOverride", [("model", model)]))
}
