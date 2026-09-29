//! What was found (`PNSEs` in `wulRC`): instruction files, then skill folders, each a row to
//! choose (`cuocA`) with what it becomes; an item that cannot be imported is faded and cannot be
//! chosen. Beside them, the preview of the open item (`TI5p1`): what it becomes and the file itself.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, Row, ScrollVertical, Spacer, Text, TextOverflow},
  layout::Alignment,
  node::{CursorIcon, Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  agent_files::import::{Candidate, ImportItem, ItemKind},
  resource::SKILL_FILE,
};

use super::{Chosen, Opened, RepositoryImportDialog, copy};
use crate::{
  skills::repository::RepositoryScan,
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, UNAVAILABLE_OPTION_OPACITY},
  ui::{
    icon::{Icon, IconSize, icon},
    pressable::{Ring, keyboard_stop, pressable, surface},
    property::{ValueStyle, styled_property},
    status_label::status_label,
  },
};

/// The list's width and a row's height (`PNSEs`, `cuocA`).
const LIST_WIDTH: f32 = 500.0;
const ROW_HEIGHT: f32 = 64.0;

impl RepositoryImportDialog {
  /// The list of what was found beside the open item's preview (`t09G7M`).
  pub(super) fn review(&self, ctx: &Ctx, scan: &RepositoryScan) -> Element {
    let Opened(opened) = self.opened.get();
    let open = opened
      .as_deref()
      .and_then(|source| scan.items.iter().find(|item| item.source == source));
    Row::new()
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .child(self.list(ctx, scan, opened.as_deref()))
      .with_children(open.map(|item| preview(ctx, item)))
      .into()
  }

  fn list(&self, ctx: &Ctx, scan: &RepositoryScan, opened: Option<&str>) -> Element {
    let Chosen(chosen) = self.chosen.get();
    let mut rows: Vec<Element> = Vec::new();
    for (kind, label) in [
      (ItemKind::Instruction, "skills.import.groups.instructions"),
      (ItemKind::Skill, "skills.import.groups.skills"),
    ] {
      let items: Vec<&ImportItem> = scan.items.iter().filter(|item| item.kind == kind).collect();
      if items.is_empty() {
        continue;
      }
      rows.push(
        Column::new()
          .width(Dimension::Pct(100.0))
          .padding_top(ExtraSpacing::Group.role())
          .padding_horizontal(SpacingSize::Md)
          .padding_bottom(ExtraSpacing::Tight.role())
          .child(Text::new(&ctx.t(label)).variant(ExtraTypography::Overline.role()))
          .into(),
      );
      rows.extend(items.into_iter().map(|item| {
        let state = (chosen.contains(&item.source), opened == Some(item.source.as_str()));
        self.row(ctx, item, state)
      }));
    }
    ScrollVertical::new(Column::new().width(Dimension::Pct(100.0)).with_children(rows))
      .id("repository-import-items")
      .width(Dimension::Px(LIST_WIDTH))
      .height(Dimension::Pct(100.0))
      .padding_vertical(SpacingSize::Sm)
      .border_right(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .into()
  }

  /// One found item (`cuocA`): its check, its path over what it becomes, and its kind. The check is
  /// the form's control; pressing the rest of the row opens its preview.
  fn row(&self, ctx: &Ctx, item: &ImportItem, (chosen, opened): (bool, bool)) -> Element {
    let importable = item.outcome.is_ok();
    let source: Arc<str> = item.source.as_str().into();
    let (glyph, color) = match (importable, chosen) {
      (true, true) => (Icon::SquareCheck, PaletteColor::TextPrimary),
      _ => (Icon::Square, PaletteColor::TextSecondary),
    };
    let check_id = format!("repository-import-check-{}", item.source);
    let mut check = keyboard_stop(Button::empty().button(), Ring::Outer)
      .id(check_id)
      .describe("selected", if chosen { "true" } else { "false" })
      .rounded(RadiusSize::Sm)
      .child(icon(glyph, IconSize::Notice, color));
    if importable {
      let (list, toggled) = (self.chosen.clone(), source.clone());
      check = check.cursor(CursorIcon::Pointer).on_click(move |_| {
        list.update(
          |Chosen(chosen)| match chosen.iter().position(|picked| **picked == *toggled) {
            Some(position) => {
              chosen.remove(position);
            }
            None => chosen.push(toggled.to_string()),
          },
        );
      });
    } else {
      check = check.focusable(false).cursor(CursorIcon::NotAllowed);
    }
    let (kind_icon, kind) = copy::kind(ctx, item.kind);
    let (open, shown) = (self.opened.clone(), source.clone());
    let identity = pressable(
      surface().id(format!("repository-import-item-{}", item.source)),
      move || {
        open.set(Opened(Some(shown.to_string())));
      },
    )
    .flex(1.0)
    .height(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .child(
      Column::new()
        .flex(1.0)
        .spacing(ExtraSpacing::Tight.role())
        .child(
          Text::new(&source)
            .variant(ExtraTypography::MonoTitle.role())
            .nowrap()
            .text_overflow(TextOverflow::Elipsis),
        )
        .child(
          Text::new(&copy::becomes(ctx, item))
            .variant(TypographyStyle::Caption)
            .nowrap()
            .text_overflow(TextOverflow::Elipsis),
        ),
    )
    .child(status_label(kind_icon, &kind));
    let mut row = Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(ROW_HEIGHT))
      .padding_horizontal(SpacingSize::Md)
      .spacing(ExtraSpacing::Group.role())
      .align_items(Alignment::Center)
      .background(if opened {
        PaletteColor::AccentMuted
      } else {
        ExtraColor::Surface.role()
      })
      .child(check)
      .child(identity);
    if !importable {
      row = row.opacity(UNAVAILABLE_OPTION_OPACITY);
    }
    row.into()
  }
}

/// What the open item becomes and the file itself (`TI5p1`).
fn preview(ctx: &Ctx, item: &ImportItem) -> Element {
  let file = match item.kind {
    ItemKind::Instruction => item.source.clone(),
    ItemKind::Skill => format!("{}/{SKILL_FILE}", item.source),
  };
  let header = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(Text::new(&ctx.t("skills.import.preview.title")).variant(TypographyStyle::Label))
    .child(Spacer::new().flex(1.0))
    .child(
      Text::new(&file)
        .variant(TypographyStyle::Mono)
        .color(PaletteColor::TextSecondary)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    );
  let mut column = Column::new()
    .id("repository-import-preview")
    .flex(1.0)
    .height(Dimension::Pct(100.0))
    .padding_vertical(ExtraSpacing::Step.role())
    .padding_horizontal(SpacingSize::Lg)
    .spacing(ExtraSpacing::Stack.role())
    .background(PaletteColor::SurfacePanel)
    .child(header)
    .with_children(properties(ctx, item));
  if let Some(text) = &item.text {
    let content = ScrollVertical::new(
      Column::new()
        .width(Dimension::Pct(100.0))
        .padding(SpacingSize::Md)
        .child(
          Text::new(text)
            .variant(ExtraTypography::Output.role())
            .color(PaletteColor::TextPrimary)
            .width(Dimension::Pct(100.0))
            .selectable(true),
        ),
    )
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .background(ExtraColor::Surface.role())
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(crate::theme::ExtraRadius::Notice.role());
    column = column.child(content);
  }
  column.into()
}

/// Becomes, Scope, and for a skill its description and reference files (`n9HSc` … `iRB3g`).
fn properties(ctx: &Ctx, item: &ImportItem) -> Vec<Element> {
  let property = |label: &str, value: &str| styled_property(&ctx.t(label), value, ValueStyle::Text);
  let scope = ctx.t("skills.import.preview.scopeValue");
  match &item.outcome {
    Ok(Candidate::Instruction { name, .. }) => vec![
      property(
        "skills.import.preview.becomes",
        &ctx.t_args("skills.import.preview.instruction", [("name", name.as_str())]),
      ),
      property("skills.import.preview.scope", &scope),
    ],
    Ok(Candidate::Skill {
      name,
      description,
      files,
      ..
    }) => {
      let none = ctx.t("skills.import.preview.none");
      let files: Vec<&str> = files.iter().map(|file| file.path.as_str()).collect();
      let files = if files.is_empty() {
        none.to_string()
      } else {
        files.join(&ctx.t("common.listSeparator"))
      };
      vec![
        property(
          "skills.import.preview.becomes",
          &ctx.t_args("skills.import.preview.skill", [("name", name.as_str())]),
        ),
        property("skills.import.preview.scope", &scope),
        property(
          "skills.import.preview.description",
          if description.is_empty() { &none } else { description },
        ),
        styled_property(&ctx.t("skills.import.preview.files"), &files, ValueStyle::Path),
      ]
    }
    Err(reason) => vec![property(
      "skills.import.preview.becomes",
      &copy::unimportable(ctx, reason),
    )],
  }
}
