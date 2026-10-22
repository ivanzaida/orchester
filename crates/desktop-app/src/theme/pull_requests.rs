//! The design values of the Pull requests page (`Pull requests · inbox · dark` `p9R7P` and its states) and of
//! a pull request's page (`Pull request · Files changed` `mVTVX`) that the UI kit's roles do not carry,
//! registered as lurq `Extra` roles.

use std::sync::Arc;

use lurq::{
  app::theme::{SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::{mono, style};

/// Gaps and paddings of the Pull requests page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PullRequestsSpacing {
  /// 40: inside the content panel at its sides (`Content · Pull requests` `X6VN7J`).
  Page,
  /// 64: over the unavailable panel (`Center` `hK3t5`).
  Unavailable,
  /// 72: over an empty tab's glyph (`Empty` `r1gusH`).
  Empty,
  /// 32: inside a pull request's page at its sides (`PR header` `nQXR1`, `Changes toolbar` `C4AKw`).
  Detail,
  /// 18: over and under an open feedback thread's panel (`Thread panel` `Z3JKS`), over the THREADS heading
  /// (`Threads head` `kVI2F`).
  FeedbackPanel,
  /// 30: a feedback post's words and a thread's events and notes past its author's avatar and their gap
  /// (`Body wrap` `ZWGEY`, `Events` `EJWbi`).
  FeedbackIndent,
}

impl PullRequestsSpacing {
  pub const ALL: [Self; 6] = [
    Self::Page,
    Self::Unavailable,
    Self::Empty,
    Self::Detail,
    Self::FeedbackPanel,
    Self::FeedbackIndent,
  ];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Page => "pull_requests_page",
      Self::Unavailable => "pull_requests_unavailable",
      Self::Empty => "pull_requests_empty",
      Self::Detail => "pull_requests_detail",
      Self::FeedbackPanel => "pull_requests_feedback_panel",
      Self::FeedbackIndent => "pull_requests_feedback_indent",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::Page => 40.0,
      Self::Unavailable => 64.0,
      Self::Empty => 72.0,
      Self::Detail => 32.0,
      Self::FeedbackPanel => 18.0,
      Self::FeedbackIndent => 30.0,
    }
  }
}

pub(super) fn install(theme: &Theme) {
  for spacing in PullRequestsSpacing::ALL {
    theme.set_spacing_value(spacing.role(), spacing.value());
  }
}

/// Text of a pull request's page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PullRequestsText {
  /// The number beside the title: Geist Mono 17 `text_muted` (`Number` `eFZie`).
  TitleNumber,
  /// The count beside FILES: Geist Mono 10.5 `text_muted` (`Count` `SDcBY`).
  FileCount,
  /// What a feedback thread asks, in its row: 12 `text_secondary` on 16 lines (`Message` `c2WVh` in `ZtcOK`).
  ThreadRequest,
  /// What happened to a feedback request: 12/500 `text_primary`, 1.5 (`Label` `ks2WH` in `y1KEgI`).
  EventLabel,
  /// When it happened: Geist Mono 11 `text_muted`, 1.6 (`Time` `PrLpE` in `y1KEgI`).
  EventTime,
}

impl PullRequestsText {
  pub const ALL: [Self; 5] = [
    Self::TitleNumber,
    Self::FileCount,
    Self::ThreadRequest,
    Self::EventLabel,
    Self::EventTime,
  ];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::TitleNumber => "pull_requests_title_number",
      Self::FileCount => "pull_requests_file_count",
      Self::ThreadRequest => "pull_requests_thread_request",
      Self::EventLabel => "pull_requests_event_label",
      Self::EventTime => "pull_requests_event_time",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::TitleNumber => mono(style(17.0, FontWeight::Normal, 1.3, 0.0, palette.text_muted)),
      Self::FileCount => mono(style(10.5, FontWeight::Normal, 1.35, 0.0, palette.text_muted)),
      Self::ThreadRequest => style(12.0, FontWeight::Normal, 16.0 / 12.0, 0.0, palette.text_secondary),
      Self::EventLabel => style(12.0, FontWeight::Medium, 1.5, 0.0, palette.text_primary),
      Self::EventTime => mono(style(11.0, FontWeight::Normal, 1.6, 0.0, palette.text_muted)),
    }
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  PullRequestsText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}
