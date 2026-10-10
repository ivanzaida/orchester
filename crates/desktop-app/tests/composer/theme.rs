//! The composer's own theme roles resolve to `Composer` `QQ2ZM` and `Send button/split` `wPMDa` of
//! `data/orchester2.pen`.

use lurq::{app::App, node::color::Color};
use orchester_app::theme::{self, ComposerColor, ComposerRadius, ComposerText, Mode};

#[test]
fn composer_theme_roles_follow_the_composer_components() {
  let mut app = App::new();
  theme::install(&mut app, Mode::Dark);
  let theme = app.theme();
  assert_eq!(
    theme.palette().get(ComposerColor::SplitDivider.role()),
    Color::from_hex("#00000033")
  );
  let lock = theme.typography_style(ComposerText::AttachLock.role());
  assert_eq!((lock.font_size, lock.color), (9.0, Color::from_hex("#8A8A92")));
  assert_eq!(theme.radius_value(ComposerRadius::Context.role()), 8.0);
}
