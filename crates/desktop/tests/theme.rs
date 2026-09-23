use lurq::{
  app::{
    App,
    theme::{PaletteColor, RadiusSize, TypographyStyle},
  },
  node::color::Color,
};
use orchester_desktop::theme::{self, ExtraColor};

fn themed_app() -> App {
  let mut app = App::new();
  theme::install(&mut app);
  app
}

#[test]
fn palette_roles_resolve_to_pencil_values() {
  let app = themed_app();
  let theme = app.theme();
  assert_eq!(
    theme.palette_color(PaletteColor::SurfaceBase),
    Color::from_hex("#101010")
  );
  assert_eq!(
    theme.palette_color(PaletteColor::TextPrimary),
    Color::from_hex("#EEEEEE")
  );
  assert_eq!(
    theme.palette_color(ExtraColor::Surface.role()),
    Color::from_hex("#1A1A1A")
  );
  assert_eq!(
    theme.palette_color(ExtraColor::Hairline.role()),
    Color::from_hex("#2B2B2B")
  );
}

#[test]
fn typography_and_radii_follow_pencil() {
  let app = themed_app();
  let theme = app.theme();
  let heading = theme.typography_style(TypographyStyle::Heading);
  assert_eq!(heading.font_size, 30.0);
  assert_eq!(&*heading.font_family, "Inter");
  assert_eq!(theme.radius_value(RadiusSize::Md), 7.0);
  assert_eq!(theme.radius_value(RadiusSize::Lg), 12.0);
}
