//! Values of a task's Services tab (`06 · Task · Services` `cjvRr` in `Agent testing · redesign` `eWydp`): a service
//! row's name, port and uptime, and the selected service's live log (`Log panel` `iw8mT`).
use std::sync::Arc;

use lurq::{
  app::theme::{ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::{mono, style};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunServicesText {
  /// Geist Mono 12.5/500 `text_primary`: a service's name in its row (`an59e`).
  Name,
  /// Geist Mono 12 `text_secondary`: a service's port, `:41870` (`bBPsV`).
  Port,
  /// Geist Mono 12 `text_muted`: how long a service has run, `18m 02s`, or `—` (`pPX2f`).
  Uptime,
  /// 12/500 `text_secondary`: the live log's title, `team-server · live log` (`ftDRR`).
  LogTitle,
  /// Geist Mono 11.5 `text_muted`: when a log line came (`TPtll`); also Orchester's own lines (`YNX3I`).
  LogTime,
  /// Geist Mono 11.5 `text_secondary`: what a log line says (`FCQe1`); standard error's in `danger` (`eJvWh`).
  LogLine,
}

impl RunServicesText {
  pub const ALL: [Self; 6] = [
    Self::Name,
    Self::Port,
    Self::Uptime,
    Self::LogTitle,
    Self::LogTime,
    Self::LogLine,
  ];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Name => "run_services_name",
      Self::Port => "run_services_port",
      Self::Uptime => "run_services_uptime",
      Self::LogTitle => "run_services_log_title",
      Self::LogTime => "run_services_log_time",
      Self::LogLine => "run_services_log_line",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::Name => mono(style(12.5, FontWeight::Medium, 1.3, 0.0, palette.text_primary)),
      Self::Port => mono(style(12.0, FontWeight::Normal, 1.3, 0.0, palette.text_secondary)),
      Self::Uptime => mono(style(12.0, FontWeight::Normal, 1.3, 0.0, palette.text_muted)),
      Self::LogTitle => style(12.0, FontWeight::Medium, 1.3, 0.0, palette.text_secondary),
      Self::LogTime => mono(style(11.5, FontWeight::Normal, 1.3, 0.0, palette.text_muted)),
      Self::LogLine => mono(style(11.5, FontWeight::Normal, 1.3, 0.0, palette.text_secondary)),
    }
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  RunServicesText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}
