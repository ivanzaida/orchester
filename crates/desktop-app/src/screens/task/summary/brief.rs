//! What the task asks for (`Brief` `Gt4az`, `Acceptance criteria` `ufoA8` in `lUXff`): the brief as
//! Markdown, as Details shows it, and the criteria as a numbered list with their count, each read as
//! Markdown too. A part the task does not have yet is left out.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::task::TaskDetails;

use super::section_head;
use crate::{
  theme::{ExtraSpacing, TaskText},
  ui::{
    chat::links,
    markdown::{LinkHandler, MarkdownView},
  },
};

/// The width of a criterion's number (`Os6zg`).
const NUMBER_WIDTH: f32 = 14.0;

pub fn sections(ctx: &mut Ctx, details: &TaskDetails) -> Vec<Element> {
  let on_link = links(ctx);
  let brief = (!details.brief.trim().is_empty()).then(|| {
    let text = MarkdownView::new("task-brief-text", details.brief.as_str(), on_link.clone())
      .style(TaskText::Brief.role(), PaletteColor::TextPrimary)
      .blank_line_between_blocks()
      .render(ctx);
    Column::new()
      .id("task-brief")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(section_head(ctx, "task-brief-head", "summary.brief", None))
      .child(text)
      .into()
  });
  let criteria = &details.acceptance_criteria;
  let list = (!criteria.is_empty()).then(|| {
    let head = section_head(ctx, "task-criteria-head", "summary.criteria", Some(criteria.len()));
    let rows: Vec<Element> = (1..)
      .zip(criteria)
      .map(|(number, text)| criterion(ctx, number, text, &on_link))
      .collect();
    Column::new()
      .id("task-criteria")
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Snug.role())
      .child(head)
      .with_children(rows)
      .into()
  });
  brief.into_iter().chain(list).collect()
}

fn criterion(ctx: &mut Ctx, number: usize, text: &str, on_link: &LinkHandler) -> Element {
  let text = MarkdownView::new(format!("task-criterion-{number}-text"), text, on_link.clone()).render(ctx);
  Row::new()
    .id(format!("task-criterion-{number}"))
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Start)
    .child(
      Text::new(&number.to_string())
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .width(Dimension::Px(NUMBER_WIDTH)),
    )
    .child(Column::new().flex(1.0).min_width(Dimension::Px(0.0)).child(text))
    .into()
}
