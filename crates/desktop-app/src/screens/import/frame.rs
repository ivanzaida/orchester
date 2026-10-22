//! The frame every step of Import resources shares (`Content · Import resources` in `DsqQJ`): the header with the
//! breadcrumb, the title, what an import does and the stepper (`d0fLpX`); the footer with what the step will do and
//! its actions (`s4V5W0`); and the centered state a step shows instead of its list (`Empty state` `MXKb5`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use super::{State, Step};
use crate::{
  routes::{OverviewTab, ProjectPage, Route},
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography, IconSize, ImportText},
  ui::{
    breadcrumb::{Crumb, CrumbSeparator, breadcrumb},
    icon::{Icon, icon},
    icon_button::IconButton,
  },
};

/// The footer's height (`s4V5W0`).
const FOOTER_HEIGHT: f32 = 56.0;
/// A step's dot and the line between two steps (`Dot` in `oFevv`, `Connector` `G4PIA2`).
const STEP_DOT: f32 = 18.0;
const CONNECTOR_WIDTH: f32 = 20.0;
/// The centered state's glyph and the widest its words run (`ehYtq`, `fG1nT` in `MXKb5`).
const STATE_GLYPH: f32 = 36.0;
const STATE_TEXT_WIDTH: f32 = 460.0;

/// The page, as tall as the content panel: the header, then `body` taking the room the footer leaves.
pub(super) fn page(header: Element, body: Element, footer: Element) -> Element {
  Column::new()
    .id("screen-import_resources")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .child(header)
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .min_height(Dimension::Px(0.0))
        .child(body),
    )
    .child(footer)
    .into()
}

/// The header: back and the breadcrumb over the title, what an import does and where the import stands.
pub(super) fn header(ctx: &mut Ctx, state: &State, name: &str, step: Step) -> Element {
  let navigation = state.navigation.clone();
  let back = IconButton::new("import-back", Icon::ArrowLeft, ctx.t("import.back"))
    .inspector()
    .on_click(move || navigation.back())
    .render(ctx);
  let overview = state.navigation.to(Route::Project(
    state.project,
    ProjectPage::Overview(OverviewTab::default()),
  ));
  let crumbs = breadcrumb(
    ctx,
    "import-crumbs",
    CrumbSeparator::Chevron,
    vec![Crumb::to(name, overview), Crumb::here(ctx.t("import.title"))],
  );
  let title = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(SpacingSize::Xs)
    .child(
      Text::new(&ctx.t("import.title"))
        .id("import-title")
        .variant(TypographyStyle::Heading),
    )
    .child(
      Text::new(&ctx.t_args("import.explanation", [("project", name)]))
        .id("import-explanation")
        .width(Dimension::Pct(100.0))
        .variant(TypographyStyle::Description),
    );
  Column::new()
    .id("import-header")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(ExtraSpacing::Inset.role())
        .bottom(SpacingSize::Lg)
        .left(SpacingSize::Xl)
        .right(SpacingSize::Xl),
    )
    .spacing(ExtraSpacing::Loose.role())
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Row::new()
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(back)
        .child(crumbs),
    )
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Xl)
        .align_items(Alignment::End)
        .child(title)
        .child(
          Row::new()
            .padding_bottom(ExtraSpacing::Tight.role())
            .child(stepper(ctx, step)),
        ),
    )
    .into()
}

/// Select resources, Resolve dependencies and Create drafts, the ones before `step` done (`Stepper` `s4SpV4`).
fn stepper(ctx: &Ctx, step: Step) -> Row {
  let steps = [
    (Step::Select, "select", "import.steps.select"),
    (Step::Resolve, "resolve", "import.steps.resolve"),
    (Step::Create, "create", "import.steps.create"),
  ];
  let mut row = Row::new()
    .id("import-stepper")
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center);
  for (index, (shown, id, key)) in steps.into_iter().enumerate() {
    if index > 0 {
      row = row.child(
        Row::new()
          .width(Dimension::Px(CONNECTOR_WIDTH))
          .height(Dimension::Px(ctx.theme().border_size_value(BorderSize::Sm)))
          .background(PaletteColor::BorderStrong),
      );
    }
    row = row.child(step_mark((shown.cmp(&step), index + 1), id, &ctx.t(key)));
  }
  row
}

/// One step of the stepper: done (`MkJiU`), current (`oFevv`) or next (`MhTGv`).
fn step_mark((place, number): (std::cmp::Ordering, usize), id: &str, label: &str) -> Row {
  use std::cmp::Ordering;
  let dot = Row::new()
    .width(Dimension::Px(STEP_DOT))
    .height(Dimension::Px(STEP_DOT))
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .rounded(ExtraRadius::Full.role());
  let number = number.to_string();
  let (dot, label, state) = match place {
    Ordering::Less => (
      dot
        .background(PaletteColor::SuccessMuted)
        .child(icon(Icon::Check, IconSize::Xs, PaletteColor::Success)),
      Text::new(label)
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextSecondary),
      "done",
    ),
    Ordering::Equal => (
      dot
        .background(PaletteColor::TextPrimary)
        .child(Text::new(&number).variant(ImportText::StepCurrent.role())),
      Text::new(label).variant(TypographyStyle::Label),
      "current",
    ),
    Ordering::Greater => (
      dot
        .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
        .child(Text::new(&number).variant(ImportText::StepNext.role())),
      Text::new(label)
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted),
      "next",
    ),
  };
  Row::new()
    .id(format!("import-step-{id}"))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .describe("state", state)
    .child(dot)
    .child(label.nowrap())
}

/// The footer: `text` at its start, then `end` (a reason and the actions) at its end.
pub(super) fn footer(text: &str, end: Vec<Element>) -> Element {
  Row::new()
    .id("import-footer")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(FOOTER_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Xl))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Text::new(text)
        .id("import-footer-text")
        .variant(TypographyStyle::Description)
        .nowrap(),
    )
    .child(Spacer::new().flex(1.0))
    .with_children(end)
    .into()
}

/// Why the footer's action cannot be used: a lock and the words (`Blocked` `NNW4g`).
pub(super) fn blocked(text: &str) -> Element {
  Row::new()
    .id("import-blocked")
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .describe("role", "unavailable-reason")
    .child(icon(Icon::Lock, IconSize::Sm, PaletteColor::TextMuted))
    .child(
      Text::new(text)
        .id("import-blocked-text")
        .variant(TypographyStyle::Description)
        .nowrap(),
    )
    .into()
}

/// What the footer's action creates (`Readiness` `pOXam`).
pub(super) fn ready_line(text: &str) -> Element {
  Row::new()
    .id("import-readiness")
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::CircleCheck, IconSize::Base, PaletteColor::Success))
    .child(
      Text::new(text)
        .id("import-readiness-text")
        .variant(TypographyStyle::Description)
        .nowrap(),
    )
    .into()
}

/// A step's centered state (`Empty state` `MXKb5`): a round glyph, a title, what it means and its action.
pub(super) fn centered(
  id: &str,
  (glyph, tint): (Icon, PaletteColor),
  (title, text): (&str, &str),
  action: Option<Element>,
) -> Element {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::vertical(ExtraSpacing::Roomy.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .describe("state", "empty")
    .child(
      Row::new()
        .width(Dimension::Px(STATE_GLYPH))
        .height(Dimension::Px(STATE_GLYPH))
        .align_items(Alignment::Center)
        .justify(Justify::Center)
        .rounded(ExtraRadius::Full.role())
        .background(PaletteColor::SurfaceRaised)
        .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .child(icon(glyph, IconSize::Lg, tint)),
    )
    .child(
      Text::new(title)
        .id(format!("{id}-title"))
        .variant(ExtraTypography::FormSection.role()),
    )
    .child(
      Text::new(text)
        .id(format!("{id}-text"))
        .max_width(Dimension::Px(STATE_TEXT_WIDTH))
        .text_align(lurq::layout::text_style::TextAlign::Center)
        .variant(TypographyStyle::Description),
    )
    .with_children(action)
    .into()
}

/// A group's eyebrow in Geist Mono (`KRlEH`, `JsK2W`).
pub(super) fn eyebrow(id: &str, text: &Arc<str>) -> Text {
  Text::new(text)
    .id(id.to_owned())
    .variant(ImportText::Eyebrow.role())
    .nowrap()
}
