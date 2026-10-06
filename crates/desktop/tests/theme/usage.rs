//! The Usage dashboard's chart roles ("Chart colors"): the series colors follow the CLI, the grid,
//! rules, axis text, threshold and single-series marks come from the chrome's roles, and the
//! hatching before a CLI's first report is `#FFFFFF0F`.

use lurq::{
  app::{
    App, Tree,
    component::Component,
    ctx::Ctx,
    theme::{PaletteColor, TypographyStyle},
  },
  node::{Element, color::Color},
};
use orchester_desktop::theme::{ExtraColor, chart::ChartStyle};
use parking_lot::Mutex;

use super::themed_app;

#[test]
fn usage_chart_colors_follow_pencil() {
  let app = themed_app();
  let theme = app.theme();
  for (role, hex) in [
    (ExtraColor::ChartSeries1, "#3987E5"),
    (ExtraColor::ChartSeries2, "#D95926"),
    (ExtraColor::ChartSeries3, "#199E70"),
    (ExtraColor::ChartHatch, "#FFFFFF0F"),
  ] {
    assert_eq!(theme.palette_color(role.role()), Color::from_hex(hex), "{hex}");
  }
}

static STYLE: Mutex<Option<ChartStyle>> = Mutex::new(None);

/// Resolves the chart style as a component does, from the theme it renders with.
struct Probe;

impl Component for Probe {
  type Props = ();

  fn create(_: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    *STYLE.lock() = Some(ChartStyle::of(ctx));
    Element::new()
  }
}

#[test]
fn usage_chart_style_draws_with_the_chrome_roles() {
  let mut app: App = themed_app();
  let mut tree = Tree::new();
  tree.mount_root::<Probe>(&mut app, ());
  tree.pass_headless(&mut app);
  let style = STYLE.lock().clone().expect("the probe rendered");
  let theme = app.theme();
  let color = |role: PaletteColor| theme.palette_color(role);
  assert_eq!(style.series[0], Color::from_hex("#3987E5"));
  assert_eq!(style.series[1], Color::from_hex("#D95926"));
  assert_eq!(style.grid, color(ExtraColor::Hairline.role()));
  assert_eq!(style.rule, color(PaletteColor::Border));
  assert_eq!(style.axis, color(PaletteColor::TextMuted));
  assert_eq!(style.threshold, color(PaletteColor::TextSecondary));
  let axis = theme.typography_style(TypographyStyle::Mono);
  assert_eq!((&*style.font.family, style.font.size), (&*axis.font_family, 10.0));
}
