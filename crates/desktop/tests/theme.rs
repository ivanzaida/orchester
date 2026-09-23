use lurq::{
  app::{
    App,
    theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  layout::text_style::FontWeight,
  node::{color::Color, dimension::Dimension},
};
use orchester_desktop::theme::{self, ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography};

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

#[test]
fn extra_typography_follows_pencil() {
  let app = themed_app();
  let theme = app.theme();
  let overline = theme.typography_style(ExtraTypography::Overline.role());
  assert_eq!(overline.font_size, 9.0);
  assert_eq!(overline.weight, FontWeight::SemiBold);
  assert_eq!(
    theme.typography_style(ExtraTypography::SectionHeading.role()).font_size,
    15.0
  );
  assert_eq!(
    theme.typography_style(ExtraTypography::CardTitle.role()).font_size,
    14.0
  );
  assert_eq!(theme.typography_style(ExtraTypography::Metric.role()).font_size, 23.0);
}

#[test]
fn getting_started_roles_follow_pencil() {
  let app = themed_app();
  let theme = app.theme();
  let display = theme.typography_style(ExtraTypography::Display.role());
  assert_eq!(display.font_size, 44.0);
  assert_eq!(display.letter_spacing, -1.2);
  assert_eq!(
    theme.typography_style(ExtraTypography::Eyebrow.role()).letter_spacing,
    1.6
  );
  assert_eq!(
    &*theme.typography_style(ExtraTypography::IconNav.role()).font_family,
    "lucide"
  );
  assert_eq!(theme.spacing_value(ExtraSpacing::Columns.role()), Dimension::Px(112.0));
  assert_eq!(theme.spacing_value(SpacingSize::Section), Dimension::Px(32.0));
  assert_eq!(theme.radius_value(ExtraRadius::Notice.role()), 8.0);
}
