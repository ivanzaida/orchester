//! The Browser window's roles (`Browser · redesign` `lcNez` in the new `orchester.pen`): the switcher's initial
//! (`sdrrt`), the address (`dSm2I`), a dialog's message (`fCL4z`), the agent's name on its mark (`u7jZ2`), the file
//! dialog's hint (`LcNjE`, `LIvDb`, `FCera`), the switcher's, the activity line's and a dialog's spacing, the dots'
//! ring and the fade at a cut edge of the strip.
use orchester_app::theme::{BrowserBorder, BrowserColor, BrowserSpacing, BrowserText};

use super::*;

#[test]
fn browser_roles_follow_the_redesign() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  for (role, size, weight) in [
    (BrowserText::SwitcherInitial, 10.5, FontWeight::SemiBold),
    (BrowserText::Address, 12.0, FontWeight::Normal),
    (BrowserText::DialogMessage, 13.0, FontWeight::Normal),
    (BrowserText::AgentTag, 11.0, FontWeight::Medium),
    (BrowserText::FileHintTitle, 13.0, FontWeight::Medium),
    (BrowserText::FileHintLine, 12.0, FontWeight::Normal),
  ] {
    let style = theme.typography_style(role.role());
    assert_eq!((style.font_size, style.weight), (size, weight), "{role:?}");
  }
  for (spacing, value) in [
    (BrowserSpacing::Switcher, 7.0),
    (BrowserSpacing::Activity, 14.0),
    (BrowserSpacing::Dialog, 20.0),
    (BrowserSpacing::FileHintSides, 18.0),
  ] {
    assert_eq!(theme.spacing_value(spacing.role()), Dimension::Px(value), "{spacing:?}");
  }
  assert!((theme.border_size_value(BrowserBorder::Ring.role()) - 1.5).abs() < f32::EPSILON);
  assert_eq!(
    theme.palette_color(BrowserColor::FadeClear.role()),
    Color::from_hex("#11111300"),
    "the fade ends in the title bar's color, clear"
  );
  assert_eq!(
    theme.palette_color(BrowserColor::PageCover.role()),
    Color::from_hex("#00000000"),
    "no one sees what covers the page in an inspection capture"
  );
}
