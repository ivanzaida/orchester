//! The composer's own theme roles resolve to `Composer` `QQ2ZM` and `Send button/split` `wPMDa` of
//! `data/orchester2.pen`.

use lurq::{app::App, node::color::Color};
use orchester_app::theme::{self, ComposerColor, ComposerRadius, Mode};

#[test]
fn composer_theme_roles_follow_the_composer_components() {
  let mut app = App::new();
  theme::install(&mut app, Mode::Dark);
  let theme = app.theme();
  assert_eq!(
    theme.palette().get(ComposerColor::SplitDivider.role()),
    Color::from_hex("#00000033")
  );
  assert_eq!(theme.radius_value(ComposerRadius::Context.role()), 8.0);
}
