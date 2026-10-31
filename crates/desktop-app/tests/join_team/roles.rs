//! The step's text roles follow *Join a team*'s frames (`B914L`, `drCQd`): size, line height, weight, tracking and
//! family.

use lurq::{
  app::{App, theme::TypographyStyle},
  layout::text_style::FontWeight,
};
use orchester_app::theme::{self, Mode, StartupText};

#[test]
fn join_team_roles_follow_the_design() {
  let mut app = App::new();
  theme::install(&mut app, Mode::Dark);
  let theme = app.theme();
  let check = |role: TypographyStyle, (size, line): (f32, f32), weight: FontWeight, (tracking, family): (f32, &str)| {
    let style = theme.typography_style(role);
    assert_eq!(
      (
        style.font_size,
        // The line height is a ratio of the size: the design's pixels, to a hundredth.
        (style.font_size * style.line_height * 100.0).round() / 100.0,
        style.weight,
        style.letter_spacing,
        &*style.font_family,
      ),
      (size, line, weight, tracking, family),
      "{role:?}"
    );
  };
  let sans = (0.0, "Geist");
  check(StartupText::Crumb.role(), (12.5, 16.0), FontWeight::Normal, sans);
  check(StartupText::CrumbCurrent.role(), (12.5, 16.0), FontWeight::Medium, sans);
  check(StartupText::StepNumber.role(), (10.5, 13.0), FontWeight::Medium, sans);
  check(StartupText::StepLabel.role(), (12.0, 16.0), FontWeight::Normal, sans);
  check(
    StartupText::StepLabelCurrent.role(),
    (12.0, 16.0),
    FontWeight::Medium,
    sans,
  );
  check(
    StartupText::Title.role(),
    (22.0, 28.0),
    FontWeight::SemiBold,
    (-0.4, "Geist"),
  );
  check(StartupText::Lead.role(), (13.5, 21.0), FontWeight::Normal, sans);
  check(StartupText::WayTitle.role(), (15.0, 20.0), FontWeight::SemiBold, sans);
  check(StartupText::Sub.role(), (12.0, 17.0), FontWeight::Normal, sans);
  check(StartupText::Aside.role(), (12.5, 17.0), FontWeight::Normal, sans);
  check(StartupText::Link.role(), (12.5, 17.0), FontWeight::Medium, sans);
  check(
    StartupText::Section.role(),
    (11.0, 14.0),
    FontWeight::Medium,
    (0.4, "Geist"),
  );
  check(StartupText::RowTitle.role(), (13.5, 18.0), FontWeight::Medium, sans);
  check(
    StartupText::RowAddress.role(),
    (12.0, 17.0),
    FontWeight::Normal,
    (0.0, "Geist Mono"),
  );
  check(
    StartupText::FieldMono.role(),
    (13.0, 17.0),
    FontWeight::Normal,
    (0.0, "Geist Mono"),
  );
  check(
    StartupText::ArrivalKey.role(),
    (11.5, 15.0),
    FontWeight::Normal,
    (0.0, "Geist Mono"),
  );
  check(StartupText::ArrivalTitle.role(), (12.5, 17.0), FontWeight::Normal, sans);
  check(
    StartupText::ServerAddress.role(),
    (12.5, 17.0),
    FontWeight::Normal,
    (0.0, "Geist Mono"),
  );
  check(StartupText::ProjectName.role(), (13.0, 18.0), FontWeight::Medium, sans);
  check(
    StartupText::ProjectAddress.role(),
    (12.0, 16.0),
    FontWeight::Normal,
    (0.0, "Geist Mono"),
  );
  check(StartupText::CheckLine.role(), (12.0, 17.0), FontWeight::Normal, sans);
}
