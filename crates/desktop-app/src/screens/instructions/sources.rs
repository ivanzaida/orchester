//! Prompt sources BYHVZ and excluded rules S9rsK, read in core assembly order.
use super::words;
use crate::theme::ExtraSpacing;
use lurq::{
  app::{
    ctx::Ctx,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  node::{Element, dimension::Dimension},
};
use orchester_core::run::{Composition, PromptSource};

pub fn list(ctx: &Ctx, composition: &Composition) -> Element {
  let mut list = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(
      Text::new(&ctx.t_args(
        "instructions.sources",
        [("count", composition.sections().len().to_string())],
      ))
      .variant(crate::theme::instructions::overline()),
    );
  for (index, section) in composition.sections().iter().enumerate() {
    let version = match section.source {
      PromptSource::AgentProfile { version, .. } | PromptSource::Step { version, .. } => {
        ctx.t_args("instructions.version", [("version", version.get().to_string())])
      }
      PromptSource::Instruction { version, .. } => words::preview_version(ctx, version),
      PromptSource::Task { .. } => ctx.t("instructions.taskContext"),
    };
    list = list.child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Snug.role())
        .child(Text::new(&(index + 1).to_string()).variant(TypographyStyle::Caption))
        .child(
          Column::new()
            .flex(1.0)
            .child(Text::new(&words::part(ctx, section.source.part())).variant(TypographyStyle::Caption))
            .child(Text::new(&section.name).variant(TypographyStyle::Description)),
        )
        .child(Text::new(&version).variant(TypographyStyle::Caption)),
    );
  }
  for excluded in composition.excluded() {
    list = list.child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .child(
          Text::new(&ctx.t_args("instructions.excluded", [("name", excluded.name.as_str())]))
            .variant(TypographyStyle::Description),
        )
        .child(Text::new(&words::excluded(ctx, excluded.reason)).variant(TypographyStyle::Caption)),
    );
  }
  list.into()
}
