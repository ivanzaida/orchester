//! Beside Import from repository's groups, the opened item's preview (`Preview · …` `q8fCag` in 02c): its head with
//! the file it reads and that it is read only, then what it becomes, its scope, where it applies or what it says it is
//! for, and its reference files as the hub's rows, and the file's text in a read-only box that scrolls. `id` names the
//! page: `<id>-source` holds the preview, `<id>-source-text` the file's text.
use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize},
  },
  components::{Column, ScrollVertical, Text},
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::agent_files::import::{Candidate, ImportItem, ItemKind};

use super::{
  import_rows::Detail,
  parts::{self, Entry},
};
use crate::theme::{HubSpacing, HubText, instructions};

/// The text's box (`Area` `E8duxz`).
const SOURCE_HEIGHT: f32 = 200.0;

pub(crate) fn preview(ctx: &Ctx, id: &str, item: Option<&ImportItem>, detail: Detail) -> Element {
  let Some(item) = item else {
    return Text::new(&ctx.t("instructions.importEmpty"))
      .variant(HubText::Hint.role())
      .into();
  };
  let text = item.text.as_deref().unwrap_or_default();
  let filename = if item.kind == ItemKind::Skill {
    format!("{}/SKILL.md", item.source)
  } else {
    item.source.clone()
  };
  let lines = ctx.t_args(
    if text.lines().count() == 1 {
      "instructions.importSourceLineOne"
    } else {
      "instructions.importSourceLines"
    },
    [("count", text.lines().count().to_string())],
  );
  let source = format!("{id}-source");
  let title = ctx.t_args("instructions.importSource", [("source", filename.as_str())]);
  let head = parts::head(&source, &title, None, Some(parts::quiet(&lines).into()));
  let facts: Element = match &item.outcome {
    Ok(candidate) => parts::boxed(&format!("{id}-facts"), facts(ctx, id, candidate)).into(),
    Err(_) => Text::new(&detail(ctx, item))
      .id(format!("{id}-unimportable"))
      .variant(HubText::Hint.role())
      .width(Dimension::Pct(100.0))
      .into(),
  };
  Column::new()
    .id(source.clone())
    .width(Dimension::Pct(100.0))
    .spacing(lurq::app::theme::SpacingSize::Sm)
    .child(head)
    .child(facts)
    .child(text_box(&source, text))
    .into()
}

/// *Becomes*, *Scope*, *Applies* on an instruction or *Description* on a skill, and *Reference files* (`BvEJV`,
/// `C2OJP`, `Z3WmL3`).
fn facts(ctx: &Ctx, id: &str, candidate: &Candidate) -> Vec<Element> {
  let (becomes, files) = match candidate {
    Candidate::Instruction { name, .. } => (
      ctx.t_args("instructions.importBecomesInstruction", [("name", name.as_str())]),
      ctx.t("instructions.none"),
    ),
    Candidate::Skill { name, files, .. } => (
      ctx.t_args("instructions.importBecomesSkill", [("name", name.as_str())]),
      if files.is_empty() {
        ctx.t("instructions.none")
      } else {
        files
          .iter()
          .map(|file| file.path.as_str())
          .collect::<Vec<_>>()
          .join(" · ")
          .into()
      },
    ),
  };
  let mut rows = vec![
    Entry::new(format!("{id}-becomes"), ctx.t("instructions.becomes")).hint(becomes),
    Entry::new(format!("{id}-scope"), ctx.t("instructions.scope")).hint(ctx.t("instructions.importScope")),
  ];
  match candidate {
    Candidate::Instruction { .. } => rows.push(
      Entry::new(format!("{id}-applies"), ctx.t("instructions.applies")).hint(ctx.t("instructions.importApplies")),
    ),
    Candidate::Skill { description, .. } if !description.is_empty() => rows.push(
      Entry::new(format!("{id}-description"), ctx.t("skills.previewDescription")).hint(description.to_string()),
    ),
    _ => {}
  }
  rows.push(Entry::new(format!("{id}-files"), ctx.t("instructions.referenceFiles")).hint(files));
  rows.into_iter().map(Element::from).collect()
}

/// The file's text, selectable and read only, scrolling in its box.
fn text_box(id: &str, text: &str) -> Element {
  let body = Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(HubSpacing::Row.role(), HubSpacing::FieldY.role()))
    .child(
      Text::new(text)
        .id(format!("{id}-text"))
        .variant(instructions::source_text())
        .selectable(true)
        .width(Dimension::Pct(100.0)),
    );
  Column::new()
    .id(format!("{id}-box"))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(SOURCE_HEIGHT))
    .background(PaletteColor::SurfaceInput)
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .rounded(RadiusSize::Lg)
    .clip()
    .child(
      ScrollVertical::new(body)
        .width(Dimension::Pct(100.0))
        .height(Dimension::Pct(100.0)),
    )
    .into()
}
