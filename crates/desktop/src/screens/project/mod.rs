//! Project shell: the sidebar (`zo9Py`) beside the project overview (`Twtqc`). Tasks, runs and
//! agents do not exist yet, so the overview shows what the project is and says so.

mod sidebar;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension},
};

use self::sidebar::sidebar;
use crate::{
  theme::{ExtraColor, ExtraSpacing},
  ui::{
    button::ActionButton,
    icon::{Icon, IconSize, icon},
    panel::RecoveryPanel,
  },
  workspace::OpenProject,
};

/// Project bar height (`CsB3h`).
const PROJECT_BAR_HEIGHT: f32 = 64.0;

pub struct ProjectShell;

impl Component for ProjectShell {
  type Props = OpenProject;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let open = ctx.props::<OpenProject>().clone();
    Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(sidebar(ctx, &open))
      .child(
        Column::new()
          .flex(1.0)
          .height(Dimension::Pct(100.0))
          .child(project_bar(ctx, &open))
          .child(overview(ctx, &open)),
      )
  }
}

fn project_bar(ctx: &Ctx, open: &OpenProject) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(PROJECT_BAR_HEIGHT))
    .padding_horizontal(SpacingSize::Section)
    .spacing(ExtraSpacing::Inline.role())
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(Text::new(open.project.name().as_str()).variant(TypographyStyle::Description))
    .child(icon(Icon::ChevronRight, IconSize::Small, PaletteColor::TextMuted))
    .child(
      Text::new(&ctx.t("nav.overview"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextPrimary),
    )
    .into()
}

fn overview(ctx: &Ctx, open: &OpenProject) -> Element {
  let heading = Row::new()
    .width(Dimension::Pct(100.0))
    .justify(Justify::SpaceBetween)
    .align_items(Alignment::Center)
    .child(
      Column::new()
        .spacing(SpacingSize::Sm)
        .child(Text::new(open.project.name().as_str()).variant(TypographyStyle::Heading))
        .child(
          Text::new(&ctx.t("overview.description"))
            .variant(TypographyStyle::Body)
            .color(PaletteColor::TextSecondary),
        ),
    )
    .child(
      ActionButton::new(ctx.t("overview.newTask"))
        .primary()
        .icon(Icon::Plus)
        .id("new-task")
        .render(ctx),
    );
  let empty = RecoveryPanel::new(
    Icon::FolderGit,
    &ctx.t("overview.emptyTitle"),
    &ctx.t("overview.emptyDetail"),
  )
  .detail(property(&ctx.t("overview.projectKey"), open.project.key().as_str()))
  .detail(location(
    &ctx.t("overview.repository"),
    &open.repo_root.display().to_string(),
  ));
  Column::new()
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .padding(SpacingSize::Section)
    .spacing(ExtraSpacing::Rail.role())
    .child(heading)
    .child(empty)
    .into()
}

/// Key/value property (`tfGRt`).
fn property(label: &str, value: &str) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .justify(Justify::SpaceBetween)
    .child(Text::new(label).variant(TypographyStyle::Caption).nowrap())
    .child(
      Text::new(value)
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::TextPrimary)
        .selectable(true),
    )
    .into()
}

/// A long value such as a path, under its label so it can use the full width.
fn location(label: &str, value: &str) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xs)
    .child(Text::new(label).variant(TypographyStyle::Caption))
    .child(
      Text::new(value)
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::TextPrimary)
        .selectable(true),
    )
    .into()
}
