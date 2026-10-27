//! A new instruction's scope, chosen on its page (`Scope` in `02e New instruction` `dMYPx`, new in ORC-107): the list
//! has no tab per scope any more, so the rule says where it applies — the project, an agent role, a pipeline or one
//! step — each with what it reaches and, for the last three, which one.
use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Row, SelectOption, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_workspace::instructions::{data::ProjectInstructions, form::ScopeChoice};

use super::fields::Fields;
use crate::{
  screens::hub::parts::{boxed, group, head},
  theme::{ExtraSpacing, ExtraTypography, HubSpacing, HubText},
  ui::{
    field::SelectField,
    toggle::{Toggle, ToggleKind},
  },
};

/// The scope selects' width (`Choose…` in 02e).
const SELECT_WIDTH: f32 = 240.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
  Project,
  Role,
  Pipeline,
  Step,
}

impl Kind {
  const ALL: [Self; 4] = [Self::Project, Self::Role, Self::Pipeline, Self::Step];

  const fn of(scope: &ScopeChoice) -> Self {
    match scope {
      ScopeChoice::Project => Self::Project,
      ScopeChoice::AgentRole(_) => Self::Role,
      ScopeChoice::Pipeline(_) => Self::Pipeline,
      ScopeChoice::Step(..) => Self::Step,
    }
  }

  const fn empty(self) -> ScopeChoice {
    match self {
      Self::Project => ScopeChoice::Project,
      Self::Role => ScopeChoice::AgentRole(None),
      Self::Pipeline => ScopeChoice::Pipeline(None),
      Self::Step => ScopeChoice::Step(None, None),
    }
  }

  const fn words(self) -> (&'static str, &'static str, &'static str) {
    match self {
      Self::Project => ("project", "instructions.scopeProject", "instructions.scopeProjectHint"),
      Self::Role => ("role", "instructions.scopeRole", "instructions.scopeRoleHint"),
      Self::Pipeline => (
        "pipeline",
        "instructions.scopePipeline",
        "instructions.scopePipelineHint",
      ),
      Self::Step => ("step", "instructions.scopeStep", "instructions.scopeStepHint"),
    }
  }
}

/// The scope choice; `locked` while the new rule is being created, so it is made where it was chosen.
pub fn render(ctx: &mut Ctx, fields: &Fields, data: &ProjectInstructions, locked: bool) -> Element {
  let chosen = Kind::of(&fields.scope.get());
  let rows: Vec<Element> = Kind::ALL
    .into_iter()
    .map(|kind| row(ctx, fields, data, (kind, kind == chosen, locked)))
    .collect();
  group(
    "instruction-scope",
    head("instruction-scope", &ctx.t("instructions.scopeTitle"), None, None),
    boxed("instruction-scope-rows", rows),
  )
  .into()
}

/// Which role, pipeline or step the scope means; the project needs no choice.
fn options(ctx: &Ctx, data: &ProjectInstructions, kind: Kind) -> Option<Vec<SelectOption<ScopeChoice>>> {
  match kind {
    Kind::Project => None,
    Kind::Role => Some(
      data
        .roles()
        .into_iter()
        .map(|role| SelectOption::new(ScopeChoice::AgentRole(Some(role.clone())), role.as_str()))
        .collect(),
    ),
    Kind::Pipeline => Some(
      data
        .pipelines
        .iter()
        .map(|p| SelectOption::new(ScopeChoice::Pipeline(Some(p.id)), p.name.clone()))
        .collect(),
    ),
    Kind::Step => Some(
      data
        .pipelines
        .iter()
        .flat_map(|p| p.steps.iter().map(move |s| (p, s)))
        .map(|(p, s)| {
          let step = ctx.t_args(
            "instructions.stepLabel",
            [("number", format!("{:02}", s.ordinal)), ("name", s.name.to_string())],
          );
          SelectOption::new(
            ScopeChoice::Step(Some(p.id), Some(s.key.clone())),
            ctx.t_args(
              "instructions.scopeStepOption",
              [("pipeline", p.name.to_string()), ("step", step.to_string())],
            ),
          )
        })
        .collect(),
    ),
  }
}

fn row(
  ctx: &mut Ctx,
  fields: &Fields,
  data: &ProjectInstructions,
  (kind, chosen, locked): (Kind, bool, bool),
) -> Element {
  let (name, title, hint) = kind.words();
  let scope = fields.scope.clone();
  let toggle = Toggle::new(
    format!("instruction-scope-{name}"),
    ToggleKind::Radio,
    chosen,
    ctx.t(title),
  )
  .hide_label();
  let toggle = if locked {
    toggle
  } else {
    toggle.on_change(move |_| {
      if Kind::of(&scope.get_untracked()) != kind {
        scope.set(kind.empty());
      }
    })
  };
  let select = options(ctx, data, kind).map(|options| {
    let field = SelectField::new(
      format!("instruction-scope-{name}-select"),
      ctx.t(title),
      fields.scope.clone(),
      options,
    )
    .placeholder(ctx.t("instructions.choose"));
    let field = if locked { field.locked() } else { field };
    Column::new()
      .width(Dimension::Px(SELECT_WIDTH))
      .child(field.render(ctx))
  });
  Row::new()
    .id(format!("instruction-scope-{name}-row"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(HubSpacing::Row.role(), HubSpacing::FieldY.role()))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(toggle)
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(Text::new(&ctx.t(title)).variant(ExtraTypography::Strong.role()))
        .child(Text::new(&ctx.t(hint)).variant(HubText::Hint.role())),
    )
    .with_children(select)
    .into()
}
