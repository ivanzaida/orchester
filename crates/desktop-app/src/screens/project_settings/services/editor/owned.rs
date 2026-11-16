//! Owned paths (`Owned paths` `llF5n`): *Add a path pattern* with *Add* beside it (`Entry row` `W1WVzu`), then the
//! patterns as pills with their × (`Pills` `k207rV`). A run that changes a path one names starts this service for its
//! Test step. Adding one, by *Add* or Enter, and removing one saves; why a pattern cannot be owned is said under the
//! field.

use lurq::{
  app::ctx::Ctx,
  components::{Column, Row},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};

use super::{
  super::super::kit::{Group, on_enter},
  Commit, View,
  fields::field,
  sections::{Owned, Sections},
};
use crate::{
  theme::ExtraSpacing,
  ui::{
    button::{ActionButton, ButtonKind},
    chip::TagChip,
    field::FieldInput,
    icon::Icon,
    message::message_text,
  },
};

/// The Owned paths group.
pub(super) fn owned_group(ctx: &mut Ctx, sections: &Sections, view: &View, commit: &Commit) -> Element {
  let paths = sections.owned.get().0;
  let error = view.problems().and_then(|problems| problems.owned_paths.as_ref());
  let add = adder(sections, commit);
  let field_input = FieldInput::new(
    "settings-service-owned-add".to_owned(),
    ctx.t("project_settings.services.editor.owned.add"),
    sections.owned_adding.clone(),
  )
  .mono()
  .placeholder(ctx.t("project_settings.services.editor.owned.add"));
  let field_input = match error {
    Some(error) => field_input.error(message_text(ctx, error)),
    None => field_input,
  };
  let entry = Row::new()
    .id("settings-service-owned-entry")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Start)
    .on_key_down(on_enter({
      let add = add.clone();
      move || add()
    }))
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .child(field_input.render(ctx)),
    )
    .child(Element::from(
      ActionButton::new(
        "settings-service-owned-add-button",
        ButtonKind::Secondary,
        ctx.t("project_settings.services.editor.owned.addButton"),
      )
      .icon(Icon::Plus)
      .on_click(move || add()),
    ));
  let pills = (!paths.is_empty()).then(|| {
    Row::new()
      .id("settings-service-owned")
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Snug.role())
      .with_children(
        paths
          .iter()
          .enumerate()
          .map(|(at, path)| pill(ctx, sections, (at, path), commit)),
      )
  });
  let control = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .child(entry)
    .with_children(pills);
  let row = field(
    "settings-service-owned",
    ctx.t("project_settings.services.editor.owned.title"),
    ctx.t("project_settings.services.editor.owned.hint"),
  )
  .render(ctx, control);
  Group::new(
    "settings-service-owned-group",
    ctx.t("project_settings.services.editor.owned.title"),
  )
  .row(row)
  .into()
}

/// Adds the pattern typed after the others and saves; a blank one adds nothing.
fn adder(sections: &Sections, commit: &Commit) -> std::sync::Arc<dyn Fn() + Send + Sync> {
  let (owned, adding, commit) = (sections.owned.clone(), sections.owned_adding.clone(), commit.clone());
  std::sync::Arc::new(move || {
    let typed = adding.get_untracked().trim().to_owned();
    if typed.is_empty() {
      return;
    }
    adding.set(String::new());
    owned.update(|Owned(paths)| paths.push(typed));
    commit();
  })
}

/// An owned pattern's pill; its × removes it and saves.
fn pill(ctx: &Ctx, sections: &Sections, (at, path): (usize, &str), commit: &Commit) -> Element {
  let (owned, commit) = (sections.owned.clone(), commit.clone());
  TagChip::new(format!("settings-service-owned-{at}"), path.to_owned())
    .mono()
    .pill()
    .removable(
      ctx.t_args("project_settings.services.editor.owned.remove", [("path", path)]),
      move || {
        owned.update(|Owned(paths)| {
          if at < paths.len() {
            paths.remove(at);
          }
        });
        commit();
      },
    )
    .into()
}
