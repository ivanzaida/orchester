//! 01.03 Project & repository (`W6uXiL`), limited to what works locally: a name, a key and a
//! local git repository. GitHub, runner and agent steps are shown as unavailable.

mod form;
mod page;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, Spacer, Text},
  node::{Element, border::Border, dimension::Dimension},
};

use self::form::ProjectForm;
use crate::{
  navigation::Navigation,
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{button::ActionButton, icon::Icon, nav_item::NavItem},
};

/// Setup rail width (`Hw5eX`).
const RAIL_WIDTH: f32 = 280.0;

pub struct ProjectSetup;

impl Component for ProjectSetup {
  type Props = ();

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let navigation = ctx
      .use_context::<Navigation>()
      .expect("the root provides Navigation before mounting screens");
    Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(rail(ctx, navigation))
      .child(ctx.mount_keyed::<ProjectForm>("project-form", ()))
  }
}

fn rail(ctx: &Ctx, navigation: Navigation) -> Element {
  let back = ActionButton::new(ctx.t("common.back"))
    .icon(Icon::ArrowLeft)
    .id("setup-back")
    .fill_width()
    .on_click(move || navigation.back_to_start())
    .render(ctx);
  Column::new()
    .width(Dimension::Px(RAIL_WIDTH))
    .height(Dimension::Pct(100.0))
    .padding(ExtraSpacing::Rail.role())
    .spacing(SpacingSize::Lg)
    .background(PaletteColor::SurfacePanel)
    .border_right(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(Text::new(&ctx.t("app.wordmark")).variant(ExtraTypography::SetupBrand.role()))
    .child(Text::new(&ctx.t("setup.rail.heading")).variant(ExtraTypography::Eyebrow.role()))
    .child(
      NavItem::new("setup-step-project", Icon::CircleDot, ctx.t("setup.rail.project"))
        .step()
        .selected(true),
    )
    .child(NavItem::new("setup-step-agents", Icon::Circle, ctx.t("setup.rail.agents")).step())
    .child(NavItem::new("setup-step-ready", Icon::Circle, ctx.t("setup.rail.ready")).step())
    .child(Spacer::new().flex(1.0))
    .child(back)
    .child(Text::new(&ctx.t("setup.rail.backHint")).variant(ExtraTypography::Detail.role()))
    .into()
}
