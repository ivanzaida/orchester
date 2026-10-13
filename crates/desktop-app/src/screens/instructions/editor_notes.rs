//! Injection explanations and publication recovery in ilieH / J7EgbM.
use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, Text},
  node::{Element, dimension::Dimension},
};
use orchester_core::resource::Variable;
use orchester_workspace::instructions::{
  data::ProjectInstructions,
  form::{Events, ScopeChoice},
};

use super::{fields::Fields, words};
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
pub fn warning(ctx: &Ctx, fields: &Fields) -> Option<Element> {
  let variable = fields.blocked.get().0.first().copied()?;
  let context = fields.preview_context.get();
  let expression = format!("{{{{{}}}}}", variable.name());
  let title = ctx.t_args(
    "instructions.unavailableTitle",
    [
      ("expression", expression),
      ("event", words::event(ctx, fields.events.get()).to_string()),
    ],
  );
  let mut note = Notice::new("instruction-publication-warning", NoticeTone::Warning, title)
    .inspector()
    .body(ctx.t_args(
      "instructions.unavailableDetail",
      [("role", context.role), ("step", context.step)],
    ));
  if variable == Variable::RunRevision {
    let events = fields.events.clone();
    let frozen = fields.frozen.clone();
    let resume = ActionButton::new(
      "instruction-warning-resume",
      ButtonKind::Secondary,
      ctx.t("instructions.injectResume"),
    )
    .icon(Icon::Clock)
    .on_click(move || {
      if !frozen.get_untracked() {
        events.set(Events::Resume);
      }
    });
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
    .icon(Icon::GitBranch)
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
    let base = if fields.frozen.get() || fields.unavailable.get().0.contains(&Variable::RunBaseRevision) {
      base.disabled()
    } else {
      base
    };
    note = note
      .action(if fields.frozen.get() { resume.disabled() } else { resume })
      .action(base);
  }
  Some(note.into())
}
