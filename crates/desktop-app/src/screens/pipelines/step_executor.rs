//! The Step group's executor and model rows (`Executor` `RodWl`, `Model for this step` `NPfAh` in `IsU5u`): one
//! Executor dropdown listing the project's profiles, then its tools; what the profile runs on, or what decides a
//! tool step; and the step's model as the CLI takes it. The variants of 03b (`g6Ecy`): no profile chosen, a profile
//! with only a draft (*Open <profile>*), a tool that cannot run with why, by its kind.
use lurq::{
  app::ctx::Ctx,
  components::SelectOption,
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::resource::{StepKind, ToolKind};
use orchester_workspace::pipelines::{
  data::DraftProfileChoice,
  fields::{self, Executor, ModelChoice},
};

use super::{
  executor_menu::{Entry, ExecutorField, Tone},
  step_fields::{FIELD_WIDTH, Props, locked_options},
};
use crate::ui::{
  button::{ActionButton, ButtonKind},
  field::SelectField,
  icon::Icon,
  notice::{Notice, NoticeTone},
  row_group::{GroupRow, HintTone},
};

pub(super) fn draft_profile(props: &Props) -> Option<DraftProfileChoice> {
  let StepKind::Agent { profile: Some(id), .. } = props.step.kind else {
    return None;
  };
  props
    .state
    .get()
    .opened
    .assignables
    .draft_profiles
    .iter()
    .find(|p| p.id == id)
    .cloned()
}

/// The Executor row, and the notices that follow the group: a profile not published, a tool that cannot run.
pub fn executor(ctx: &mut Ctx, props: &Props, value: Signal<Executor>) -> (GroupRow, Vec<Element>) {
  let draft = draft_profile(props);
  let missing = value.get() == Executor::Unset;
  let row = GroupRow::new("step-executor-row", ctx.t("pipelines.executor"));
  let row = if props.viewing {
    row.value(&executor_name(ctx, props))
  } else {
    let (profiles, tools) = executor_entries(ctx, props, draft.as_ref());
    let tone = if missing {
      Tone::Invalid
    } else if draft.is_some() {
      Tone::Warning
    } else {
      Tone::Normal
    };
    let field = ExecutorField {
      id: "step-executor".into(),
      name: ctx.t("pipelines.executor"),
      value,
      unset: ctx.t("pipelines.chooseProfile"),
      profiles,
      tools,
      width: FIELD_WIDTH,
      tone,
      locked: props.locked,
    };
    row.end(field.render(ctx))
  };
  let row = match (missing, &draft) {
    (true, _) => row.toned_hint(ctx.t("pipelines.missingProfile"), HintTone::Warning),
    (false, Some(_)) => row.toned_hint(ctx.t("pipelines.profileUnpublished"), HintTone::Warning),
    (false, None) => match runs_on(ctx, props) {
      Some(hint) => row.hint(hint),
      None => row,
    },
  };
  (row, notices(ctx, props, draft.as_ref()))
}

/// The executor's name, as a viewed version shows it.
fn executor_name(ctx: &Ctx, props: &Props) -> String {
  let editing = props.state.get();
  let assignables = &editing.opened.assignables;
  match props.step.kind {
    StepKind::Agent { profile: Some(id), .. } => assignables.profile(id).map_or_else(
      || ctx.t("delivery.step.unknownProfile").to_string(),
      |choice| choice.name.to_string(),
    ),
    StepKind::Tool { tool: Some(id) } => assignables.tool(id).map_or_else(
      || ctx.t("pipelines.unassigned").to_string(),
      |choice| choice.name.to_string(),
    ),
    _ => ctx.t("pipelines.unassigned").to_string(),
  }
}

fn notices(ctx: &mut Ctx, props: &Props, draft: Option<&DraftProfileChoice>) -> Vec<Element> {
  let editing = props.state.get();
  let mut notices = Vec::new();
  if let Some(profile) = draft {
    notices.push(
      Notice::new(
        "step-profile-unpublished",
        NoticeTone::Warning,
        ctx.t("pipelines.profileUnpublished"),
      )
      .stacked()
      .body(ctx.t_args("pipelines.profileDraftHint", [("name", profile.name.as_ref())]))
      .action(profile_action(ctx, props, profile))
      .into(),
    );
  }
  if let StepKind::Tool { tool: Some(id) } = props.step.kind
    && let Some(tool) = editing.opened.assignables.tool(id).filter(|tool| !tool.runnable)
  {
    notices.push(
      Notice::new(
        "step-tool-unavailable",
        NoticeTone::Info,
        ctx.t("pipelines.toolUnavailable"),
      )
      .icon(Icon::Lock)
      .body(ctx.t(tool_reason(tool.kind)))
      .into(),
    );
  }
  notices
}

/// What the step's profile runs on (`Runs on Claude Code · account Work`), or what decides a tool step.
fn runs_on(ctx: &mut Ctx, props: &Props) -> Option<std::sync::Arc<str>> {
  let id = match props.step.kind {
    StepKind::Tool { .. } => return Some(ctx.t("pipelines.toolBadge")),
    StepKind::Agent { profile: Some(id), .. } => id,
    StepKind::Agent { profile: None, .. } => return None,
  };
  let accounts = crate::services::accounts::listed(ctx);
  let editing = props.state.get();
  let profile = editing
    .opened
    .assignables
    .profiles
    .iter()
    .find(|profile| profile.id == id)?;
  let connection = profile.agent.as_ref()?;
  let account = accounts.of_connection(connection.account(), connection.provider())?;
  let account = account.name().map_or_else(
    || ctx.t("pipelines.defaultAccount").to_string(),
    |name| name.as_str().to_owned(),
  );
  Some(ctx.t_args(
    "pipelines.runsOnHint",
    [("connection", connection.name().as_str()), ("account", &account)],
  ))
}

fn profile_action(ctx: &mut Ctx, props: &Props, profile: &DraftProfileChoice) -> Element {
  let button = ActionButton::new(
    "step-open-profile",
    ButtonKind::Secondary,
    ctx.t_args("pipelines.openProfile", [("name", profile.name.as_ref())]),
  );
  if props.navigation_locked {
    return crate::ui::tooltip::Tooltip::new("step-open-profile-reason", ctx.t("pipelines.profileWait"))
      .wrap(ctx, button.disabled());
  }
  let navigation = crate::navigation::Navigation::of(ctx);
  let project = navigation
    .route_now()
    .project()
    .expect("a pipeline page belongs to a project");
  button
    .opens(navigation.to(crate::routes::Route::Project(
      project,
      crate::routes::ProjectPage::Agent(profile.id, crate::routes::AgentTab::default()),
    )))
    .into()
}

/// The profiles, and the tools; a draft profile and a tool that cannot run are listed with why, not chosen.
fn executor_entries(ctx: &Ctx, props: &Props, draft: Option<&DraftProfileChoice>) -> (Vec<Entry>, Vec<Entry>) {
  let editing = props.state.get();
  let assignables = &editing.opened.assignables;
  let mut profiles: Vec<Entry> = assignables
    .profiles
    .iter()
    .map(|p| Entry {
      value: Executor::Profile(p.id),
      label: p.name.clone(),
      icon: Icon::Bot,
      unavailable: None,
    })
    .collect();
  if let Some(profile) = draft {
    profiles.push(Entry {
      value: Executor::Profile(profile.id),
      label: ctx.t_args("pipelines.profileDraft", [("name", profile.name.as_ref())]),
      icon: Icon::Bot,
      unavailable: Some(ctx.t("pipelines.profileUnpublished")),
    });
  }
  let tools = assignables
    .tools
    .iter()
    .map(|t| Entry {
      value: Executor::Tool(t.id),
      label: t.name.clone(),
      icon: tool_icon(t.kind),
      unavailable: (!t.runnable).then(|| ctx.t("pipelines.toolUnavailable")),
    })
    .collect();
  (profiles, tools)
}

/// A tool's kind, as the Tools screen draws it.
const fn tool_icon(kind: ToolKind) -> Icon {
  match kind {
    ToolKind::Command => Icon::Terminal,
    ToolKind::Http => Icon::Globe,
    ToolKind::Database => Icon::Database,
  }
}

/// Why a tool of `kind` cannot run as a step.
const fn tool_reason(kind: ToolKind) -> &'static str {
  match kind {
    ToolKind::Database => "pipelines.databaseUnavailable",
    ToolKind::Http => "pipelines.httpUnavailable",
    ToolKind::Command => "pipelines.toolAccessUnavailable",
  }
}

pub(super) fn unavailable_tool(props: &Props) -> Option<orchester_workspace::pipelines::data::ToolChoice> {
  let StepKind::Tool { tool: Some(id) } = props.step.kind else {
    return None;
  };
  props
    .state
    .get()
    .opened
    .assignables
    .tool(id)
    .filter(|tool| !tool.runnable)
    .cloned()
}

/// The Model row: an agent step's model, or *Not used by tools* for a tool that cannot run; none for a tool step.
pub fn model(ctx: &Ctx, props: &Props, value: &Signal<ModelChoice>) -> Option<GroupRow> {
  let row = GroupRow::new("step-model-row", ctx.t("pipelines.model"));
  if matches!(props.step.kind, StepKind::Tool { .. }) {
    unavailable_tool(props)?;
    let unused = Signal::new(false);
    let select = SelectField::new(
      "step-model",
      ctx.t("pipelines.model"),
      unused,
      vec![SelectOption::new(false, ctx.t("pipelines.toolModelUnused"))],
    )
    .width(Dimension::Px(FIELD_WIDTH))
    .locked();
    return Some(row.muted().end(select.render(ctx)));
  }
  let editing = props.state.get();
  let draft = draft_profile(props);
  let inherited = draft
    .as_ref()
    .map_or_else(|| ctx.t("pipelines.inherit"), |p| p.model.as_str().into());
  if props.viewing {
    return Some(match value.get().0 {
      Some(model) => row.mono_value(model.as_str()),
      None => row.value(&inherited),
    });
  }
  let models = fields::model_choices(
    editing.opened.assignables.profiles.iter().map(|p| &p.model),
    value.get().0.as_ref(),
  );
  let mut options = vec![SelectOption::new(ModelChoice(None), inherited)];
  options.extend(
    models
      .into_iter()
      .map(|m| SelectOption::new(ModelChoice(Some(m.clone())), m.as_str().to_owned())),
  );
  let unset = Executor::of(&props.step) == Executor::Unset;
  let select = SelectField::new(
    "step-model",
    ctx.t("pipelines.model"),
    value.clone(),
    locked_options(options, props.locked || unset),
  )
  .width(Dimension::Px(FIELD_WIDTH))
  .mono();
  let select = if unset { select.locked() } else { select }.render(ctx);
  let row = row.end(select);
  Some(match draft.filter(|_| value.get().0.is_none()) {
    Some(profile) => row.hint(ctx.t_args("pipelines.profileDraftModel", [("name", profile.name.as_ref())])),
    None => row,
  })
}
