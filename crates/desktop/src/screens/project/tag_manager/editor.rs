//! Named tag editors and impact confirmations (YBfjV, ZWSYj, nOOi1).
use lurq::layout::layout_kind::Justify;

use super::*;
use crate::{
  tasks::organization::tag_chip,
  ui::field::{FieldNote, field},
};

// Approved focused editor and merge/delete panel widths.
const EDITOR_WIDTH: f32 = 568.0;
const IMPACT_WIDTH: f32 = 620.0;

impl TagManager {
  pub(super) fn dialog(&self, ctx: &mut Ctx, tags: &[(Tag, usize)], action: &Action) -> Element {
    let Some(mode) = self.editing.get() else {
      return Modal::new(Column::new()).open_when(false).into();
    };
    let pending = action.state().get().is_pending();
    let source = source(tags, mode);
    let target = self
      .target
      .get()
      .0
      .and_then(|id| tags.iter().find(|(tag, _)| tag.id() == id));
    let title = match mode {
      EditingTag::Create => ctx.t("tasks.tags.new"),
      EditingTag::Rename(_) => ctx.t_args("tasks.tags.renameNamed", [("tag", source.0)]),
      EditingTag::Delete(_) => ctx.t_args("tasks.tags.deleteNamed", [("tag", source.0)]),
      EditingTag::Merge(_) => target.map_or_else(
        || ctx.t_args("tasks.tags.mergeSource", [("source", source.0)]),
        |(tag, _)| {
          ctx.t_args(
            "tasks.tags.mergeNamed",
            [("source", source.0), ("target", tag.name().as_str())],
          )
        },
      ),
    };
    // The name editors are cards (`TK65i`, `mMFb1`: padding 24, 16 apart); merge and delete are
    // dialogs (padding 32, 24 apart).
    let named = matches!(mode, EditingTag::Create | EditingTag::Rename(_));
    let mut content = Column::new()
      .width(Dimension::Px(if named { EDITOR_WIDTH } else { IMPACT_WIDTH }))
      .padding(if named { SpacingSize::Lg } else { SpacingSize::Section })
      .spacing(if named { SpacingSize::Md } else { SpacingSize::Lg })
      .background(ExtraColor::Surface.role())
      .rounded(RadiusSize::Lg)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .child(Text::new(&title).variant(ExtraTypography::DialogTitle.role()))
      .child(self.editor(ctx, tags, mode, action));
    if let Some(error) = action.state().get().error {
      content = content.child(field_error(&message_text(ctx, &error)));
    }
    if pending {
      content = content.child(Text::new(&ctx.t("tasks.tags.saving")).variant(TypographyStyle::Caption));
    }
    self.present_editor(ctx, mode, pending, content)
  }

  fn present_editor(&self, ctx: &mut Ctx, mode: EditingTag, pending: bool, content: Column) -> Element {
    if !matches!(mode, EditingTag::Delete(_)) {
      let editing = self.editing.clone();
      return form(
        None,
        Column::new()
          .child(content)
          .on_key_down(move |event: lurq::app::events::KeyboardEvent| {
            if event.key == "Escape" && !pending {
              event.prevent_default();
              editing.set(None);
            }
          }),
      );
    }
    Modal::new(backdrop(ctx, form(None, content)))
      .target(ModalTarget::Root)
      .open(self.open.clone())
      .dismiss_on_escape(!pending)
      .into()
  }

  fn name_editor(&self, ctx: &Ctx, tags: &[(Tag, usize)], mode: EditingTag) -> (Column, Option<Mutation>) {
    let name = self.name.get();
    let input = self.name.clone();
    let duplicate = tags.iter().map(|(tag, _)| tag).find(|tag| {
      tag.name().as_str().to_lowercase() == name.trim().to_lowercase() && mode != EditingTag::Rename(tag.id())
    });
    let valid = Name::new(&name).is_ok() && duplicate.is_none();
    let control = text_input(ctx, "project-tag-name", input.clone(), !valid, TypographyStyle::Body)
      .on_input(move |event: lurq::app::events::TextInputEvent| input.set(event.new_value));
    let note = if name.trim().is_empty() {
      FieldNote::Error(ctx.t("tasks.tags.nameRequired"))
    } else if let Some(existing) = duplicate {
      FieldNote::Error(ctx.t_args(
        "tasks.tags.alreadyExists",
        [
          ("tag", existing.name().as_str()),
          ("project", ctx.props::<OpenProject>().project.name().as_str()),
        ],
      ))
    } else {
      FieldNote::None
    };
    let mut column =
      Column::new()
        .spacing(SpacingSize::Md)
        .child(field(&ctx.t("tasks.tags.nameLabel"), control.into(), note));
    // Renaming onto an existing name offers to merge into that tag instead (`YBfjV`).
    if let (EditingTag::Rename(id), Some(existing)) = (mode, duplicate) {
      let (editing, target, into) = (self.editing.clone(), self.target.clone(), existing.id());
      column = column.child(
        ActionButton::new(ctx.t_args("tasks.tags.mergeInto", [("target", existing.name().as_str())]))
          .icon(Icon::GitMerge)
          .id("tag-editor-merge-duplicate")
          .in_form()
          .on_click(move || {
            target.set(Target(Some(into)));
            editing.set(Some(EditingTag::Merge(id)));
          })
          .render(ctx),
      );
    }
    if matches!(mode, EditingTag::Rename(_)) {
      column = column.child(
        Text::new(&ctx.t_args(
          "tasks.tags.renameImpact",
          [("count", source(tags, mode).1.to_string().as_str())],
        ))
        .variant(TypographyStyle::Description),
      );
    }
    column = column.child(
      Row::new()
        .spacing(crate::theme::ExtraSpacing::Stack.role())
        .align_items(lurq::layout::Alignment::Center)
        .child(Text::new(&ctx.t("tasks.tags.preview")).variant(TypographyStyle::Description))
        .with_children((!name.trim().is_empty()).then(|| tag_chip(name.trim()))),
    );
    let request = valid.then_some(match mode {
      EditingTag::Rename(id) => Mutation::Rename(id, name),
      _ => Mutation::Create(name),
    });
    (column, request)
  }

  fn merge_editor(&self, ctx: &Ctx, tags: &[(Tag, usize)], id: TagId) -> (Column, Option<Mutation>) {
    let (source, count) = source(tags, EditingTag::Merge(id));
    let target = self
      .target
      .get()
      .0
      .and_then(|id| tags.iter().find(|(tag, _)| tag.id() == id));
    let options = std::iter::once((Target(None), ctx.t("tasks.tags.chooseTarget"))).chain(
      tags.iter().filter(|(tag, _)| tag.id() != id).map(|(tag, count)| {
        (
          Target(Some(tag.id())),
          ctx.t_args(
            if *count == 1 {
              "tasks.tags.targetCountOne"
            } else {
              "tasks.tags.targetCount"
            },
            [("tag", tag.name().as_str()), ("count", count.to_string().as_str())],
          ),
        )
      }),
    );
    let mut column = Column::new().spacing(SpacingSize::Lg);
    if let Some((tag, _)) = target {
      column = column.child(
        Text::new(&ctx.t_args(
          "tasks.tags.mergeImpact",
          [
            ("source", source),
            ("count", count.to_string().as_str()),
            ("target", tag.name().as_str()),
          ],
        ))
        .variant(ExtraTypography::Explanation.role()),
      );
    }
    column = column
      .child(field(
        &ctx.t("tasks.tags.mergeLabel"),
        Element::from(
          Select::new(self.target.clone())
            .options(options)
            .style(select_style(ctx, crate::tasks::organization::CONTROL_HEIGHT)),
        )
        .id("tag-merge-target"),
        FieldNote::None,
      ))
      .child(
        Row::new()
          .spacing(SpacingSize::Sm)
          .align_items(lurq::layout::Alignment::Center)
          .child(tag_chip(source))
          .with_children(target.map(|(tag, _)| {
            Row::new()
              .spacing(SpacingSize::Sm)
              .align_items(lurq::layout::Alignment::Center)
              .child(crate::ui::icon::icon(
                Icon::ArrowRight,
                crate::ui::icon::IconSize::Small,
                PaletteColor::TextSecondary,
              ))
              .child(tag_chip(tag.name().as_str()))
          })),
      )
      .child(impact_panel(
        &ctx.t_args("tasks.tags.affected", [("count", count.to_string().as_str())]),
        &ctx.t("tasks.tags.otherTagsStay"),
      ));
    (column, target.map(|(tag, _)| Mutation::Merge(id, tag.id())))
  }

  fn delete_editor(&self, ctx: &Ctx, tags: &[(Tag, usize)], mode: EditingTag) -> Column {
    let (name, count) = source(tags, mode);
    Column::new()
      .spacing(SpacingSize::Lg)
      .child(
        Text::new(&ctx.t_args(
          "tasks.tags.deleteImpact",
          [
            ("tag", name),
            ("count", count.to_string().as_str()),
            ("project", ctx.props::<OpenProject>().project.name().as_str()),
          ],
        ))
        .variant(ExtraTypography::Explanation.role()),
      )
      .child(impact_panel(
        &ctx.t("tasks.tags.cannotUndo"),
        &ctx.t_args("tasks.tags.deleteWarning", [("tag", name)]),
      ))
  }
  fn editor(&self, ctx: &Ctx, tags: &[(Tag, usize)], mode: EditingTag, action: &Action) -> Element {
    let (column, request) = match mode {
      EditingTag::Create | EditingTag::Rename(_) => self.name_editor(ctx, tags, mode),
      EditingTag::Merge(id) => self.merge_editor(ctx, tags, id),
      EditingTag::Delete(id) => (self.delete_editor(ctx, tags, mode), Some(Mutation::Delete(id))),
    };
    let pending = action.state().get().is_pending();
    let can_confirm = request.is_some() && !pending;
    let confirm = match mode {
      EditingTag::Create => ctx.t("tasks.tags.createTag"),
      EditingTag::Rename(_) => ctx.t("tasks.tags.saveName"),
      EditingTag::Delete(_) => ctx.t("tasks.tags.deleteTag"),
      EditingTag::Merge(_) => self
        .target
        .get()
        .0
        .and_then(|id| tags.iter().find(|(tag, _)| tag.id() == id))
        .map_or_else(
          || ctx.t("tasks.tags.merge"),
          |(tag, _)| ctx.t_args("tasks.tags.mergeInto", [("target", tag.name().as_str())]),
        ),
    };
    let (cancel, apply) = (self.editing.clone(), action.clone());
    column
      .child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .spacing(SpacingSize::Sm)
          .justify(if matches!(mode, EditingTag::Create | EditingTag::Rename(_)) {
            Justify::Start
          } else {
            Justify::End
          })
          .child(
            ActionButton::new(ctx.t(if matches!(mode, EditingTag::Delete(_)) {
              "tasks.tags.keepTag"
            } else {
              "common.cancel"
            }))
            .icon(Icon::X)
            .id("tag-editor-cancel")
            .in_form()
            .on_click(move || cancel.set(None))
            .enabled(!pending)
            .render(ctx),
          )
          .child(
            ActionButton::new(confirm)
              .icon(match mode {
                EditingTag::Create => Icon::Plus,
                EditingTag::Delete(_) => Icon::Trash,
                EditingTag::Merge(_) => Icon::GitMerge,
                _ => Icon::Check,
              })
              .id("tag-editor-confirm")
              .in_form()
              .on_click(move || {
                if let Some(request) = &request {
                  apply.run(request.clone());
                }
              })
              .enabled(can_confirm)
              .render(ctx),
          ),
      )
      .into()
  }
}

fn source(tags: &[(Tag, usize)], mode: EditingTag) -> (&str, usize) {
  let id = match mode {
    EditingTag::Create => return ("", 0),
    EditingTag::Rename(id) | EditingTag::Merge(id) | EditingTag::Delete(id) => id,
  };
  tags
    .iter()
    .find(|(tag, _)| tag.id() == id)
    .map_or(("", 0), |(tag, count)| (tag.name().as_str(), *count))
}
fn impact_panel(title: &str, explanation: &str) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Md)
    .spacing(SpacingSize::Sm)
    .background(PaletteColor::SurfaceInput)
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .rounded(RadiusSize::Md)
    .child(Text::new(title).variant(ExtraTypography::NoticeHeading.role()))
    .child(Text::new(explanation).variant(ExtraTypography::Explanation.role()))
    .into()
}
