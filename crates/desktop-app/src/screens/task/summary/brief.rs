//! What the task asks for, in the version the brief's chip shows (`Brief` `jP59S`, `Acceptance criteria` `oSvVA` in
//! `01 · Summary`): *Brief* with the version chip at the end of its heading and the brief as Markdown in 13
//! `text_secondary`, 10 under it; *Acceptance criteria* with their count, each criterion's number in Geist Mono 11
//! `text_muted` 12 before its words as inline Markdown in 13 `text_primary`, 9 over and under it over a hairline. A
//! part the task does not have yet says so.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::task::TaskDetails;

use super::section_head;
use crate::{
  theme::{ExtraSpacing, ExtraTypography, TaskSpacing, TaskText},
  ui::{
    chat::links,
    markdown::{LinkHandler, MarkdownView},
  },
};

/// *Brief* with `version`, the chip of the version shown, at its heading's end and what the version shown needs said
/// (`fallback`) under it.
pub fn brief(ctx: &mut Ctx, details: &TaskDetails, (version, fallback): (Element, Option<Element>)) -> Element {
  let text: Element = if details.brief.trim().is_empty() {
    muted("task-brief-text", &ctx.t("summary.noBrief"))
  } else {
    MarkdownView::new("task-brief-text", details.brief.as_str(), links(ctx))
      .style(TaskText::Brief.role(), PaletteColor::TextSecondary)
      .blank_line_between_blocks()
      .render(ctx)
  };
  let head = section_head(ctx, "task-brief-head", "summary.brief", None)
    .child(Spacer::new().flex(1.0))
    .child(version);
  Column::new()
    .id("task-brief")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .child(head)
    .with_children(fallback)
    .child(text)
    .into()
}

/// *Acceptance criteria* with their count and every criterion, numbered.
pub fn criteria(ctx: &mut Ctx, details: &TaskDetails) -> Element {
  let criteria = &details.acceptance_criteria;
  let head = section_head(ctx, "task-criteria-head", "summary.criteria", Some(criteria.len()));
  let list: Element = if criteria.is_empty() {
    muted("task-criteria-none", &ctx.t("summary.noCriteria"))
  } else {
    let on_link = links(ctx);
    let rows: Vec<Element> = (1..)
      .zip(criteria)
      .map(|(number, text)| criterion(ctx, number, text, &on_link))
      .collect();
    Column::new()
      .id("task-criteria-list")
      .width(Dimension::Pct(100.0))
      .with_children(rows)
      .into()
  };
  Column::new()
    .id("task-criteria")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .child(head)
    .child(list)
    .into()
}

/// "01" and the criterion's words, as inline Markdown (`Criterion 1` `JDHDx`).
fn criterion(ctx: &mut Ctx, number: usize, text: &str, on_link: &LinkHandler) -> Element {
  let text = MarkdownView::new(format!("task-criterion-{number}-text"), text, on_link.clone())
    .style(TypographyStyle::Body, PaletteColor::TextPrimary)
    .render(ctx);
  Row::new()
    .id(format!("task-criterion-{number}"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::vertical(TaskSpacing::CriterionRow.role()))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Start)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Text::new(&format!("{number:02}"))
        .id(format!("task-criterion-{number}-number"))
        .variant(ExtraTypography::MonoNote.role())
        .nowrap(),
    )
    .child(Column::new().flex(1.0).min_width(Dimension::Px(0.0)).child(text))
    .into()
}

fn muted(id: &str, text: &str) -> Element {
  Text::new(text)
    .id(id.to_owned())
    .variant(TypographyStyle::Body)
    .color(PaletteColor::TextMuted)
    .width(Dimension::Pct(100.0))
    .into()
}
