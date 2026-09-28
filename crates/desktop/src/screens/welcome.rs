//! 01.01 First launch (`ZBoBm`): shown while the workspace has no projects.

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};

use crate::{
  navigation::{Navigation, route::Route},
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    brand::brand,
    button::ActionButton,
    icon::{Icon, IconSize, icon},
  },
};

/// Layout from `ZBoBm`: topbar and footer heights, intro column width, step row height and the
/// size of the create button (`e2vBjj`).
const TOPBAR_HEIGHT: f32 = 88.0;
const FOOTER_HEIGHT: f32 = 76.0;
const INTRO_WIDTH: f32 = 528.0;
const STEP_HEIGHT: f32 = 100.0;
const CREATE_WIDTH: f32 = 260.0;
const CREATE_HEIGHT: f32 = 48.0;

pub struct Welcome;

impl Component for Welcome {
  type Props = ();

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let navigation = ctx
      .use_context::<Navigation>()
      .expect("the root provides Navigation before mounting screens");
    Column::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(topbar(ctx))
      .child(
        Column::new()
          .width(Dimension::Pct(100.0))
          .flex(1.0)
          .padding_top(ExtraSpacing::Columns.role())
          .padding_horizontal(ExtraSpacing::Margin.role())
          .padding_bottom(ExtraSpacing::Footer.role())
          .spacing(ExtraSpacing::Hero.role())
          .child(
            Row::new()
              .width(Dimension::Pct(100.0))
              .flex(1.0)
              .spacing(ExtraSpacing::Columns.role())
              .align_items(Alignment::Center)
              .child(introduction(ctx, &navigation))
              .child(steps(ctx)),
          )
          .child(footer(ctx)),
      )
  }
}

/// Brand and the workspace (`DY5WC`), also above All projects (`N6ZX71` in `Z7IZZ`).
pub(crate) fn topbar(ctx: &Ctx) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(TOPBAR_HEIGHT))
    .padding_horizontal(ExtraSpacing::Page.role())
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(brand(ctx))
    .child(Spacer::new().flex(1.0))
    .child(Text::new(&ctx.t("workspace.personal")).variant(TypographyStyle::Description))
    .into()
}

fn introduction(ctx: &Ctx, navigation: &Navigation) -> Element {
  let create = ActionButton::new(ctx.t("setup.welcome.createFirstProject"))
    .primary()
    .icon(Icon::Plus)
    .id("create-first-project")
    .size(Dimension::Px(CREATE_WIDTH), Dimension::Px(CREATE_HEIGHT))
    .link(navigation.link(Route::Setup))
    .render(ctx);
  Column::new()
    .width(Dimension::Px(INTRO_WIDTH))
    .spacing(SpacingSize::Lg)
    .child(Text::new(&ctx.t("setup.welcome.eyebrow")).variant(ExtraTypography::Eyebrow.role()))
    .child(Text::new(&ctx.t("setup.welcome.title")).variant(ExtraTypography::Display.role()))
    .child(Text::new(&ctx.t("setup.welcome.description")).variant(ExtraTypography::Lead.role()))
    .child(
      Column::new()
        .padding_top(ExtraSpacing::Stack.role())
        .spacing(ExtraSpacing::Group.role())
        .child(create)
        .child(Text::new(&ctx.t("setup.welcome.createNext")).variant(TypographyStyle::Description)),
    )
    .into()
}

fn steps(ctx: &Ctx) -> Element {
  let heading = Text::new(&ctx.t("setup.welcome.stepsHeading")).variant(ExtraTypography::Eyebrow.role());
  Column::new()
    .flex(1.0)
    .padding_vertical(SpacingSize::Sm)
    .padding_left(ExtraSpacing::Indent.role())
    .spacing(ExtraSpacing::Step.role())
    .border_left(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(heading)
    .child(step(
      "01",
      &ctx.t("setup.welcome.stepRepository"),
      &ctx.t("setup.welcome.stepRepositorySummary"),
      None,
    ))
    .child(step(
      "02",
      &ctx.t("setup.welcome.stepAgents"),
      &ctx.t("setup.welcome.stepAgentsSummary"),
      Some(&ctx.t("common.notAvailableYet")),
    ))
    .child(step(
      "03",
      &ctx.t("setup.welcome.stepTask"),
      &ctx.t("setup.welcome.stepTaskSummary"),
      Some(&ctx.t("common.notAvailableYet")),
    ))
    .into()
}

/// One getting started step; `unavailable` explains a step this version cannot do yet.
fn step(number: &str, title: &str, summary: &str, unavailable: Option<&str>) -> Element {
  let mut text = Column::new()
    .flex(1.0)
    .spacing(ExtraSpacing::Inline.role())
    .child(Text::new(title).variant(ExtraTypography::Subheading.role()))
    .child(
      Text::new(summary)
        .variant(TypographyStyle::Body)
        .color(PaletteColor::TextSecondary),
    );
  if let Some(unavailable) = unavailable {
    text = text.child(
      Row::new()
        .spacing(SpacingSize::Xs)
        .align_items(Alignment::Center)
        .child(icon(Icon::Lock, IconSize::Small, PaletteColor::TextMuted))
        .child(
          Text::new(unavailable)
            .variant(TypographyStyle::Caption)
            .color(PaletteColor::TextMuted),
        ),
    );
  }
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(STEP_HEIGHT))
    .spacing(ExtraSpacing::Step.role())
    .align_items(Alignment::Center)
    .child(Text::new(number).variant(ExtraTypography::StepNumber.role()))
    .child(text)
    .into()
}

fn footer(ctx: &Ctx) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(FOOTER_HEIGHT))
    .spacing(ExtraSpacing::Group.role())
    .align_items(Alignment::Center)
    .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(icon(Icon::FolderLock, IconSize::Feature, PaletteColor::TextSecondary))
    .child(Text::new(&ctx.t("setup.welcome.projectScope")).variant(TypographyStyle::Description))
    .child(Spacer::new().flex(1.0))
    .child(
      Text::new(&ctx.t("setup.welcome.noProjects"))
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::TextMuted),
    )
    .into()
}
