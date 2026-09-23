use lurq::{
  app::{component::Component, ctx::Ctx, theme::PaletteColor},
  components::{Column, Router, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension},
  router::{RouterHandle, Routes},
};

pub struct Root {
  router: RouterHandle,
}

impl Component for Root {
  type Props = ();

  fn create(ctx: &mut Ctx) -> Self {
    let router = ctx.router(Routes::new().route("/", placeholder));
    router.replace("/");
    Self { router }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    Column::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .background(PaletteColor::SurfaceBase)
      .child(Router::mount(ctx, self.router.clone()))
  }
}

fn placeholder(ctx: &mut Ctx) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(Text::new(&ctx.t("app.name")).variant(lurq::app::theme::TypographyStyle::Heading))
    .into()
}
