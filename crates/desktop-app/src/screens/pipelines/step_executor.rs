//! Executor readback and unavailable branches in Clxs2 (adK4e / A0a2p).
use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, SelectOption},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::resource::{StepKind, ToolKind};
use orchester_workspace::pipelines::{
  data::DraftProfileChoice,
  fields::{self, Executor, ModelChoice},
};

use super::{
  parts,
  step_fields::{Props, locked_options},
};
use crate::ui::{
  button::{ActionButton, ButtonKind},
  field::SelectField,
  icon::Icon,
  inspector::control,
  notice::{Notice, NoticeTone},
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

pub fn executor(ctx: &mut Ctx, props: &Props, value: Signal<Executor>) -> Element {
  let editing = props.state.get();
  let draft = draft_profile(props);
  let options = executor_options(ctx, props, draft.as_ref());
  let missing = value.get() == Executor::Unset;
  let tool = matches!(props.step.kind, StepKind::Tool { .. });
  let input = SelectField::new(
    "step-executor",
    ctx.t("pipelines.executor"),
    value,
    locked_options(options, props.locked),
  )
  .leading(if tool { Icon::Wrench } else { Icon::Bot });
  let input = if missing {
    input.invalid()
  } else if draft.is_some() {
    input.warning()
  } else {
    input
  };
  let mut field = Column::new().width(Dimension::Pct(100.0)).child(input.render(ctx));
  if missing {
    field = field.child(
      parts::note(&ctx.t("pipelines.missingProfile"))
        .id("step-profile-required")
        .color(PaletteColor::Danger),
    );
  }
  let mut body = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xs)
    .child(control(
      "step-executor-row",
      &ctx.t(if tool { "pipelines.tool" } else { "pipelines.profile" }),
      field,
    ));
  if let Some(profile) = draft {
    body = body.child(
      Notice::new(
        "step-profile-unpublished",
        NoticeTone::Warning,
        ctx.t("pipelines.profileUnpublished"),
      )
      .inspector()
      .body(ctx.t_args("pipelines.profileDraftHint", [("name", profile.name.as_ref())]))
      .action(profile_action(ctx, props, &profile)),
    );
  }
  if let StepKind::Tool { tool: Some(id) } = props.step.kind
    && let Some(tool) = editing.opened.assignables.tool(id).filter(|tool| !tool.runnable)
  {
    let reason = match tool.kind {
      ToolKind::Database => "pipelines.databaseUnavailable",
      ToolKind::Http => "pipelines.httpUnavailable",
      ToolKind::Command => "pipelines.toolAccessUnavailable",
    };
    body = body.child(
      Notice::new(
        "step-tool-unavailable",
        NoticeTone::Info,
        ctx.t("pipelines.toolUnavailable"),
      )
      .inspector()
      .icon(Icon::Lock)
      .body(ctx.t(reason)),
    );
  }
  body.into()
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
    .expect("a pipeline editor belongs to a project");
  let id = profile.id;
  button
    .on_click(move || {
      navigation.go(crate::routes::Route::Project(
        project,
        crate::routes::ProjectPage::Agent(id),
      ))
    })
    .into()
}

fn executor_options(ctx: &Ctx, props: &Props, draft: Option<&DraftProfileChoice>) -> Vec<SelectOption<Executor>> {
  let editing = props.state.get();
  let mut options = vec![SelectOption::new(Executor::Unset, ctx.t("pipelines.chooseProfile"))];
  options.extend(
    editing
      .opened
      .assignables
      .profiles
      .iter()
      .map(|p| SelectOption::new(Executor::Profile(p.id), p.name.clone())),
  );
  if let Some(profile) = &draft {
    options.push(
      SelectOption::new(
        Executor::Profile(profile.id),
        ctx.t_args("pipelines.profileDraft", [("name", profile.name.as_ref())]),
      )
      .disabled(true),
    );
  }
  options.extend(editing.opened.assignables.tools.iter().map(|t| {
    let option = SelectOption::new(Executor::Tool(t.id), t.name.clone());
    if t.runnable {
      option
    } else {
      option.detail(ctx.t("pipelines.toolUnavailable")).disabled(true)
    }
  }));
  options
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

pub fn model(ctx: &Ctx, props: &Props, value: &Signal<ModelChoice>) -> Element {
  if matches!(props.step.kind, StepKind::Tool { .. }) {
    if unavailable_tool(props).is_none() {
      return Column::new().into();
    }
    let unused = Signal::new(false);
    return control(
      "step-model-row",
      &ctx.t("pipelines.model"),
      SelectField::new(
        "step-model",
        ctx.t("pipelines.model"),
        unused,
        vec![SelectOption::new(false, ctx.t("pipelines.toolModelUnused"))],
      )
      .locked()
      .render(ctx),
    )
    .into();
  }
  let editing = props.state.get();
  let draft = draft_profile(props);
  let inherited = draft
    .as_ref()
    .map_or_else(|| ctx.t("pipelines.inherit"), |p| p.model.as_str().into());
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
  .leading(Icon::Cpu);
  let select = if unset { select.locked() } else { select }.render(ctx);
  let mut body =
    Column::new()
      .width(Dimension::Pct(100.0))
      .child(control("step-model-row", &ctx.t("pipelines.model"), select));
  if let Some(profile) = draft.filter(|_| value.get().0.is_none()) {
    body = body.child(control(
      "step-model-source",
      "",
      parts::note(&ctx.t_args("pipelines.profileDraftModel", [("name", profile.name.as_ref())])),
    ));
  }
  body.into()
}
