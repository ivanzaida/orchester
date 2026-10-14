//! Repository source metadata and read-only preview in U69ob.
use lurq::{
  app::{
    ctx::Ctx,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Text},
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::agent_files::import::{Candidate, ImportItem, ItemKind};

use crate::{
  theme::{ExtraSpacing, instructions},
  ui::{
    detail_row::{DetailValue, detail_row},
    source_box::source_box,
  },
};

pub fn preview(ctx: &Ctx, item: Option<&ImportItem>) -> Element {
  let Some(item) = item else {
    return Text::new(&ctx.t("instructions.importEmpty"))
      .variant(TypographyStyle::Description)
      .into();
  };
  let mut preview = Column::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .child(
      Text::new(&ctx.t_args("instructions.importSource", [("source", item.source.as_str())]))
        .variant(instructions::overline()),
    );
  if let Ok(candidate) = &item.outcome {
    let (name, files, kind) = candidate_metadata(ctx, candidate);
    let metadata = Column::new()
      .width(Dimension::Pct(100.0))
      .child(
        detail_row(
          "instruction-import-becomes",
          &ctx.t("instructions.becomes"),
          &ctx.t_args(kind, [("name", name)]),
          DetailValue::Text,
        )
        .padding(Padding::vertical(SpacingSize::Xs)),
      )
      .child(
        detail_row(
          "instruction-import-scope",
          &ctx.t("instructions.scope"),
          &ctx.t("instructions.importScope"),
          DetailValue::Text,
        )
        .padding(Padding::vertical(SpacingSize::Xs)),
      )
      .with_children((item.kind == ItemKind::Instruction).then(|| {
        detail_row(
          "instruction-import-applies",
          &ctx.t("instructions.applies"),
          &ctx.t("instructions.importApplies"),
          DetailValue::Text,
        )
        .padding(Padding::vertical(SpacingSize::Xs))
      }))
      .child(
        detail_row(
          "instruction-import-files",
          &ctx.t("instructions.referenceFiles"),
          &files,
          DetailValue::Text,
        )
        .padding(Padding::vertical(SpacingSize::Xs)),
      );
    preview = preview.child(metadata);
  } else {
    preview = preview.child(Text::new(&super::import::item_detail(ctx, item)).variant(TypographyStyle::Description));
  }
  let text = item.text.as_deref().unwrap_or_default();
  let filename = if item.kind == ItemKind::Skill {
    format!("{}/SKILL.md", item.source)
  } else {
    item.source.clone()
  };
  preview
    .child(source_box(
      "instruction-import-source",
      &filename,
      &ctx.t_args(
        if text.lines().count() == 1 {
          "instructions.importSourceLineOne"
        } else {
          "instructions.importSourceLines"
        },
        [("count", text.lines().count().to_string())],
      ),
      text,
    ))
    .into()
}
fn candidate_metadata<'a>(ctx: &Ctx, candidate: &'a Candidate) -> (&'a str, std::sync::Arc<str>, &'static str) {
  match candidate {
    Candidate::Instruction { name, .. } => (
      name.as_str(),
      ctx.t("instructions.none"),
      "instructions.importBecomesInstruction",
    ),
    Candidate::Skill { name, files, .. } => (
      name.as_str(),
      if files.is_empty() {
        ctx.t("instructions.none")
      } else {
        files
          .iter()
          .map(|file| file.path.as_str())
          .collect::<Vec<_>>()
          .join(", ")
          .into()
      },
      "instructions.importBecomesSkill",
    ),
  }
}
