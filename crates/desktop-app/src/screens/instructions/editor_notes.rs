//! Injection explanations and publication recovery in ilieH / J7EgbM.
use std::collections::BTreeSet;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  node::{Element, dimension::Dimension},
};
use orchester_core::resource::Variable;
use orchester_workspace::instructions::{
  data::ProjectInstructions,
  form::{Events, ScopeChoice},
  publication::Unavailable,
};

use super::{availability::Availability, fields::Fields, words};
use crate::{
  theme::IconSize,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    notice::{Notice, NoticeTone},
  },
};

pub fn scope(ctx: &Ctx, fields: &Fields, data: &ProjectInstructions) -> Option<Element> {
  let note = match fields.scope.get() {
    ScopeChoice::AgentRole(_) => ctx.t("instructions.roleNote"),
    ScopeChoice::Step(Some(pipeline), Some(key)) => {
      let step = data.pipeline(pipeline)?.steps.iter().find(|step| step.key == key)?;
      ctx.t_args(
        "instructions.stepNote",
        [
          (
            "step",
            ctx
              .t_args(
                "instructions.stepLabel",
                [
                  ("number", format!("{:02}", step.ordinal)),
                  ("name", step.name.to_string()),
                ],
              )
              .to_string(),
          ),
          (
            "role",
            step
              .role
              .as_ref()
              .map_or_else(|| ctx.t("instructions.all").to_string(), ToString::to_string),
          ),
        ],
      )
    }
    _ => return None,
  };
  Some(
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(icon(Icon::Info, IconSize::Xs, PaletteColor::TextMuted))
      .child(Text::new(&note).variant(TypographyStyle::Caption).flex(1.0))
      .into(),
  )
}
pub fn warning(ctx: &Ctx, fields: &Fields, availability: &Availability) -> Option<Element> {
  let Unavailable { variable, event } = fields.blocked().first().copied()?;
  let expression = format!("{{{{{}}}}}", variable.name());
  let title = ctx.t_args(
    "instructions.unavailableTitle",
    [
      ("expression", expression),
      (
        "event",
        words::event(ctx, Events::of(&BTreeSet::from([event]))).to_string(),
      ),
    ],
  );
  // Without a task to preview there is no role or step to name.
  let detail = match &availability.context {
    Some(context) => ctx.t_args(
      "instructions.unavailableDetail",
      [("role", context.role.as_str()), ("step", context.step.as_str())],
    ),
    None => ctx.t("instructions.unavailableDetailAnyAgent"),
  };
  let note = Notice::new("instruction-publication-warning", NoticeTone::Warning, title)
    .stacked()
    .body(detail);
  if variable != Variable::RunRevision {
    return Some(note.into());
  }
  // The revision can be replaced by the base revision; what the rule does without it is chosen under the notice
  // (`TYInP`, `hPvK0` in 02d).
  let text = fields.text.clone();
  let frozen = fields.frozen.clone();
  let base = ActionButton::new(
    "instruction-warning-base",
    ButtonKind::Secondary,
    ctx.t_args(
      "instructions.useBaseRevision",
      [("expression", "{{run.base_revision}}")],
    ),
  )
  .on_click(move || {
    if !frozen.get_untracked() {
      text.update(|raw| {
        *raw = orchester_workspace::instructions::tokens::replace_variable(
          raw,
          Variable::RunRevision,
          Variable::RunBaseRevision,
        )
      });
    }
  });
  let base = if fields.frozen.get() || availability.unavailable.contains(&Variable::RunBaseRevision) {
    base.disabled()
  } else {
    base
  };
  Some(
    Column::new()
      .id("instruction-unavailable")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(note.action(base))
      .child(super::variable_detail::policy(
        ctx,
        (&fields.events, &fields.frozen),
        variable,
      ))
      .into(),
  )
}
