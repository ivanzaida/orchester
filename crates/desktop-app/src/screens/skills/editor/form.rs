//! A skill's fields (`Skill`, `Body` and `Reference files` in 03b): why its draft cannot be published, then the
//! *Skill* group's rows — *Name* and *Scope* in 280 fields beside what they mean, *Description* under its label — the
//! body under its head, then its reference files.
use lurq::{
  app::ctx::Ctx,
  components::{Column, Row},
  node::{Element, dimension::Dimension},
};
use orchester_core::resource::InstructionScope;
use orchester_workspace::{messages::Message, skills::editor::publish_reason};

use super::{
  super::words, BODY_HEIGHT, DESCRIPTION_HEIGHT, Editor, FIELD_WIDTH, Props,
  files::{self, ReferenceFiles},
};
use crate::{
  screens::hub::parts,
  ui::{field::FieldInput, message::message_text},
};
impl Editor {
  pub(super) fn form(&self, ctx: &mut Ctx, p: &Props, error: Option<&Message>, busy: bool) -> Vec<Element> {
    let locked = (self.baseline.get().0.is_none() && self.submitted.get()) || p.page.reloading;
    // Why Publish is unavailable, under the bar that holds it.
    let mut parts: Vec<Element> = self.unavailable(ctx).map(Element::from).into_iter().collect();
    parts.push(self.skill_rows(ctx, p, error, locked));
    parts.push(self.body_group(ctx, locked));
    parts.push(ctx.mount_keyed::<ReferenceFiles>(
      "skill-reference-files",
      files::Props {
        root: p.page.project.open.repo_root.clone(),
        files: self.files.clone(),
        choosing: self.choosing_file.clone(),
        busy,
      },
    ));
    parts
  }

  /// The *Skill* group (`yhSpF`): Name, Scope and Description.
  fn skill_rows(&self, ctx: &mut Ctx, p: &Props, error: Option<&Message>, locked: bool) -> Element {
    let name_label = ctx.t("skills.name");
    let mut name = FieldInput::new("skill-name", name_label.clone(), self.name.clone()).mono();
    if let Some(error) = error {
      name = name.error(message_text(ctx, error));
    }
    if locked {
      name = name.locked();
    }
    let name = name.render(ctx);
    let name_hint = ctx.t("skills.nameHint");
    let scope_label = ctx.t("skills.scope");
    let scope = ctx.mount_keyed::<super::super::scope::ScopePicker>(
      "skill-scope-picker",
      super::super::scope::Props {
        page: p.page.clone(),
        value: self.scope.clone(),
        locked,
      },
    );
    let scope_hint = ctx.t(scope_hint(&self.scope.get().0));
    let description_label = ctx.t("skills.descriptionLabel");
    let description = FieldInput::new("skill-description", description_label.clone(), self.description.clone())
      .text_area()
      .area_height(DESCRIPTION_HEIGHT)
      .placeholder(ctx.t("skills.descriptionPlaceholder"));
    let description = if locked { description.locked() } else { description };
    let description = description.render(ctx);
    let rows: Vec<Element> = vec![
      parts::field("skill-name-field", &name_label, Some(&name_hint), sized(name)).into(),
      parts::field("skill-scope-field", &scope_label, Some(&scope_hint), sized(scope)).into(),
      parts::stacked_field(
        "skill-description-field",
        (&description_label, Some(&ctx.t("skills.descriptionHint"))),
        description,
        None,
      )
      .into(),
    ];
    let id = "skill-form";
    parts::group(
      id,
      parts::head(id, &ctx.t("skills.skillGroup"), None, None),
      parts::boxed("skill-form-rows", rows),
    )
    .into()
  }

  /// *Body · Markdown · SKILL.md* over the body's editor (`FZ83A`).
  fn body_group(&self, ctx: &mut Ctx, locked: bool) -> Element {
    let body = crate::ui::code_editor::CodeEditor::new("skill-body", ctx.t("skills.body"), self.body.clone())
      .locked(locked)
      .render(ctx);
    let id = "skill-body-group";
    parts::group(
      id,
      parts::head(id, &ctx.t("skills.body"), Some(&ctx.t("skills.bodyFormat")), None),
      Column::new()
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(BODY_HEIGHT))
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

/// What the scope means for who gets the skill (`Agents of this profile` in `Xg99n`).
const fn scope_hint(scope: &InstructionScope) -> &'static str {
  match scope {
    InstructionScope::Project => "skills.scopeHint.project",
    InstructionScope::AgentRole { .. } => "skills.scopeHint.role",
    InstructionScope::Pipeline { .. } => "skills.scopeHint.pipeline",
    InstructionScope::Step { .. } => "skills.scopeHint.step",
  }
}

/// A control in a *Skill* row's 280 field (`zalHn`).
fn sized(control: impl Into<Element>) -> Row {
  Row::new()
    .width(Dimension::Px(FIELD_WIDTH))
    .flex_shrink(0.0)
    .child(control)
}
