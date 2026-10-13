//! gJurA form and the inline unavailable explanation abZYW.
use lurq::{
  app::{
    ctx::Ctx,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_workspace::{messages::Message, skills::editor::publish_reason};

use super::{super::words, DESCRIPTION_HEIGHT, Editor, Props, SCOPE_WIDTH};
use crate::{
  theme::ExtraSpacing,
  ui::{
    field::{FieldInput, field},
    message::message_text,
  },
};
impl Editor {
  pub(super) fn form(&self, ctx: &mut Ctx, p: &Props, error: Option<&Message>) -> Element {
    let locked = (self.baseline.get().0.is_none() && self.submitted.get()) || p.page.reloading;
    let name_label = ctx.t("skills.name");
    let mut name = FieldInput::new("skill-name", name_label.clone(), self.name.clone()).mono();
    if let Some(error) = error {
      name = name.error(message_text(ctx, error));
    }
    if locked {
      name = name.locked();
    }
    let name = name.render(ctx);
    let name_hint = error.is_none().then(|| ctx.t("skills.nameHint"));
    let scope_label = ctx.t("skills.scope");
    let scope = ctx.mount_keyed::<super::super::scope::ScopePicker>(
      "skill-scope-picker",
      super::super::scope::Props {
        page: p.page.clone(),
        value: self.scope.clone(),
        locked,
      },
    );
    let description_label = ctx.t("skills.descriptionLabel");
    let description = FieldInput::new("skill-description", description_label.clone(), self.description.clone())
      .text_area()
      .area_height(DESCRIPTION_HEIGHT)
      .placeholder(ctx.t("skills.descriptionPlaceholder"));
    let description = if locked { description.locked() } else { description };
    let description = description.render(ctx);
    let body = crate::ui::code_editor::CodeEditor::new("skill-body", ctx.t("skills.body"), self.body.clone())
      .locked(locked)
      .render(ctx);
    Column::new()
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .min_height(Dimension::Px(0.0))
      .padding(
        Padding::new()
          .left(crate::theme::skills::form_inset())
          .right(crate::theme::skills::form_inset())
          .top(ExtraSpacing::Tabs.role())
          .bottom(SpacingSize::Md),
      )
      .spacing(SpacingSize::Md)
      .child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .spacing(SpacingSize::Md)
          .child(field("skill-name-field", Some(&name_label), name, name_hint.as_deref()).flex(1.0))
          .child(field("skill-scope-field", Some(&scope_label), scope, None).width(Dimension::Px(SCOPE_WIDTH))),
      )
      .with_children(self.unavailable(ctx))
      .child(field(
        "skill-description-field",
        Some(&description_label),
        description,
        Some(&ctx.t("skills.descriptionHint")),
      ))
      .child(
        Column::new()
          .flex(1.0)
          .min_height(Dimension::Px(0.0))
          .width(Dimension::Pct(100.0))
          .spacing(ExtraSpacing::Snug.role())
          .child(
            Row::new()
              .width(Dimension::Pct(100.0))
              .child(Text::new(&ctx.t("skills.body")).variant(TypographyStyle::FieldLabel))
              .child(Spacer::new().flex(1.0))
              .child(Text::new(&ctx.t("skills.bodyFormat")).variant(TypographyStyle::Caption)),
          )
          .child(body),
      )
      .into()
  }
  fn unavailable(&self, ctx: &Ctx) -> Option<lurq::components::Row> {
    self
      .baseline
      .get()
      .0
      .as_ref()
      .filter(|e| e.state.has_draft())
      .and_then(|entry| {
        publish_reason(entry).map(|reason| {
          crate::ui::unavailable::reason_line(
            "skill-publish-unavailable",
            &ctx.t_args(
              "skills.publishUnavailable",
              [
                ("version", words::next(entry.state).to_string()),
                ("reason", message_text(ctx, &reason).to_string()),
              ],
            ),
          )
        })
      })
  }
}
