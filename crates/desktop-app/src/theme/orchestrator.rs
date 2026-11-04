//! Values of the Orchestrator chat (`Orchestrator · redesign` `hujxu`, its cards `zuvUi` in the new orchester.pen) and
//! its settings (`Xir64`) that no other screen uses; its replies' links (`Orchestrator · links · states` `agv8r`).

use std::sync::Arc;

use lurq::{
  app::theme::{ShadowStyle, SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::{
    scrollbar::{ScrollBarPlacement, ScrollBarStyle},
    text_style::{FontWeight, TextStyle},
  },
  node::box_shadow::BoxShadow,
};

use super::{
  palette::ExtraColor,
  typography::{mono, style},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrchestratorText {
  /// 14/600 `text_primary`: the conversation's title in the header (`B9kNtt` in `s9sEw6`).
  HeaderTitle,
  /// 24/600 `text_primary`: *What should we plan?* over a new conversation (`gOmoZ` in `Y7fviC`).
  StartTitle,
  /// 12.5/500 `text_primary`: a conversation's title in History (`dNV62` in `eFI39`), a digest's count (`tQ4q9`),
  /// what a loading conversation or a failed send says.
  Emphasis,
  /// Geist Mono 10.5 `text_muted`, tracking 0.6: a day's label in History (`wpZj3` in `eFI39`).
  Eyebrow,
  /// 12 `text_secondary`, 1.4: a suggestion (`yzhnB` in `DZw5h`).
  Suggestion,
  /// 12.5 `text_primary`, 1.3: what an event line tells (`What` in `1 · Event lines` `VGbU7`).
  Line,
  /// 13/600 `text_primary`, 1.3: what a decision card asks (`Question` in `3 · Decisions` `bxJZp`).
  Question,
  /// Geist Mono 10.5 `text_muted`, 1.3: a plan's use in the runs-on picker (`sP8cS` in `v6mWp8`), an API provider's
  /// tokens and *no plan limit* (`AJwSk`, `xzvGo` in `AZp3I`), the model over its efforts (`NPHfB`).
  PlanUse,
  /// 12 `text_muted`, 1.3: what a fact of a decision card is (`Key` in `bxJZp`).
  FactKey,
  /// 12 `text_secondary`, 1.3: a fact's value (`Value` in `bxJZp`).
  FactValue,
  /// 12/500 `accent_text`, 1.3: a fact that opens its place, over a 1 `text_muted` line.
  FactLink,
  /// 12 `text_muted`, 1.3: what a fact adds after its links (*2 runs under way keep v3*).
  FactAside,
  /// Geist Mono 12 `text_secondary`, 1.3: a branch, revision or command as a fact (`p3lslr` in `S451Mc`).
  FactMono,
  /// 11 `text_muted`, 1.3: *What changes* over a draft's diff (`BlMyH`).
  DiffLabel,
  /// Geist Mono 11 `text_muted`: a line's numbers in a draft's diff (`ey7iM` in `kzMBc`).
  DiffNumber,
  /// Geist Mono 12 `text_primary`: a line of a draft's diff, and its sign (`VX2mn`, `OtSVu` in `kzMBc`).
  DiffCode,
  /// 11.5 `text_muted`: the host after a link to the web (`zKxtr` in `agv8r`).
  LinkHost,
  /// Geist Mono 11.5 `text_muted`: the address after the words of a link the chat does not follow (`Suuac` in
  /// `agv8r`).
  LinkAddress,
  /// 12 `text_muted`: what a link in a reply's labelled links is (`p0bb6` in `w262B`).
  LinkLabel,
  /// 12.5 `text_secondary`, 1.5: what a labelled link adds after it (`XosQ1` in `w262B`).
  LinkDetail,
  /// 12/600 `text_secondary`: a section's heading in an open daily report (`XCYjc` in `VZKyw`).
  ReportHeading,
  /// 11.5 `text_muted`: what an item of a daily report adds at its end (`HlavS` in `VZKyw`).
  ReportMeta,
  /// 11 `text_muted`, 1.3: when a conversation in History last changed and how many messages it has (`r9TeH` in
  /// `eFI39`).
  HistoryMeta,
  /// 11.5 `text_secondary`, 1.4: the line under a History result's title where its messages say what was searched for
  /// (`Matching line` `srL91` in 13 `SZ6QY`); who said it in `text_muted`.
  HistoryLine,
  /// 13 `text_primary`, 1.3: the message in the composer (`lB97V` in `QcfyZ`, a 17 line).
  Message,
  /// 12/500 `text_primary`, 1.3: *Plan only* on the composer's *Mode* line (`xptH1` in `y51fw9`).
  Mode,
  /// 12 `text_secondary`, 1.3: *Settings ↗* at the runs-on picker's foot, one 16 line in its 32 (`rIT51` in `SwrjI`).
  PickerLink,
}

impl OrchestratorText {
  pub const ALL: [Self; 27] = [
    Self::HeaderTitle,
    Self::StartTitle,
    Self::Emphasis,
    Self::Eyebrow,
    Self::Suggestion,
    Self::Line,
    Self::Question,
    Self::PlanUse,
    Self::FactKey,
    Self::FactValue,
    Self::FactLink,
    Self::FactAside,
    Self::FactMono,
    Self::DiffLabel,
    Self::DiffNumber,
    Self::DiffCode,
    Self::LinkHost,
    Self::LinkAddress,
    Self::LinkLabel,
    Self::LinkDetail,
    Self::ReportHeading,
    Self::ReportMeta,
    Self::HistoryMeta,
    Self::HistoryLine,
    Self::Message,
    Self::Mode,
    Self::PickerLink,
  ];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::HeaderTitle => "orchestrator_header_title",
      Self::StartTitle => "orchestrator_start_title",
      Self::Emphasis => "orchestrator_emphasis",
      Self::Eyebrow => "orchestrator_eyebrow",
      Self::Suggestion => "orchestrator_suggestion",
      Self::Line => "orchestrator_line",
      Self::Question => "orchestrator_question",
      Self::PlanUse => "orchestrator_plan_use",
      Self::FactKey => "orchestrator_fact_key",
      Self::FactValue => "orchestrator_fact_value",
      Self::FactLink => "orchestrator_fact_link",
      Self::FactAside => "orchestrator_fact_aside",
      Self::FactMono => "orchestrator_fact_mono",
      Self::DiffLabel => "orchestrator_diff_label",
      Self::DiffNumber => "orchestrator_diff_number",
      Self::DiffCode => "orchestrator_diff_code",
      Self::LinkHost => "orchestrator_link_host",
      Self::LinkAddress => "orchestrator_link_address",
      Self::LinkLabel => "orchestrator_link_label",
      Self::LinkDetail => "orchestrator_link_detail",
      Self::ReportHeading => "orchestrator_report_heading",
      Self::ReportMeta => "orchestrator_report_meta",
      Self::HistoryMeta => "orchestrator_history_meta",
      Self::HistoryLine => "orchestrator_history_line",
      Self::Message => "orchestrator_message",
      Self::Mode => "orchestrator_mode",
      Self::PickerLink => "orchestrator_picker_link",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::HeaderTitle => style(14.0, FontWeight::SemiBold, 1.3, 0.0, palette.text_primary),
      Self::StartTitle => style(24.0, FontWeight::SemiBold, 1.3, 0.0, palette.text_primary),
      Self::Emphasis => style(12.5, FontWeight::Medium, 1.35, 0.0, palette.text_primary),
      Self::Eyebrow => mono(style(10.5, FontWeight::Normal, 1.3, 0.6, palette.text_muted)),
      Self::Suggestion => style(12.0, FontWeight::Normal, 1.4, 0.0, palette.text_secondary),
      Self::Line => style(12.5, FontWeight::Normal, 1.3, 0.0, palette.text_primary),
      Self::Question => style(13.0, FontWeight::SemiBold, 1.3, 0.0, palette.text_primary),
      Self::PlanUse => mono(style(10.5, FontWeight::Normal, 1.3, 0.0, palette.text_muted)),
      Self::FactKey => style(12.0, FontWeight::Normal, 1.3, 0.0, palette.text_muted),
      Self::FactValue => style(12.0, FontWeight::Normal, 1.3, 0.0, palette.text_secondary),
      Self::FactLink => style(
        12.0,
        FontWeight::Medium,
        1.3,
        0.0,
        palette.get(ExtraColor::AccentText.role()),
      ),
      Self::FactAside => style(12.0, FontWeight::Normal, 1.3, 0.0, palette.text_muted),
      Self::FactMono => mono(style(12.0, FontWeight::Normal, 1.3, 0.0, palette.text_secondary)),
      Self::DiffLabel => style(11.0, FontWeight::Normal, 1.3, 0.0, palette.text_muted),
      Self::DiffNumber => mono(style(11.0, FontWeight::Normal, 1.3, 0.0, palette.text_muted)),
      Self::DiffCode => mono(style(12.0, FontWeight::Normal, 1.3, 0.0, palette.text_primary)),
      Self::LinkHost => style(11.5, FontWeight::Normal, 1.3, 0.0, palette.text_muted),
      Self::LinkAddress => mono(style(11.5, FontWeight::Normal, 1.3, 0.0, palette.text_muted)),
      Self::LinkLabel => style(12.0, FontWeight::Normal, 1.3, 0.0, palette.text_muted),
      Self::LinkDetail => style(12.5, FontWeight::Normal, 1.5, 0.0, palette.text_secondary),
      Self::ReportHeading => style(12.0, FontWeight::SemiBold, 1.3, 0.0, palette.text_secondary),
      Self::ReportMeta => style(11.5, FontWeight::Normal, 1.3, 0.0, palette.text_muted),
      Self::HistoryMeta => style(11.0, FontWeight::Normal, 1.3, 0.0, palette.text_muted),
      Self::HistoryLine => style(11.5, FontWeight::Normal, 1.4, 0.0, palette.text_secondary),
      Self::Message => style(13.0, FontWeight::Normal, 1.3, 0.0, palette.text_primary),
      Self::Mode => style(12.0, FontWeight::Medium, 1.3, 0.0, palette.text_primary),
      Self::PickerLink => style(12.0, FontWeight::Normal, 1.3, 0.0, palette.text_secondary),
    }
  }
}

/// Gaps and paddings of the Orchestrator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrchestratorSpacing {
  /// 20: between a new conversation's glyph, title, lead, composer and suggestions (`Start` `Y7fviC`); over the
  /// chat's first item (`ECQg8`).
  Start,
  /// 40: the sides of both Orchestrator pages, of the chat and of its composer's area (`ECQg8`, `C7Eppg`), as
  /// Settings' (`Head` `b8gGj`).
  PageSide,
  /// 18: between the chat's items (`Column` `Hoare`).
  Items,
  /// 46: what an agent's wait nests under its line, in from the line's start (`Nested` in `GIsJh`).
  Nested,
  /// 36: an open daily report's sections, in from its frame's start past its icon (`Sections` `ayZ7J` in `VZKyw`).
  Report,
  /// 12: a labelled link's label, link and detail apart (`Link row · Task` `z7af5` in `w262B`).
  LinkColumns,
  /// 28: a new conversation's message over its composer's bar (`Composer` `QcfyZ` in `c99rep`).
  StartComposer,
}

impl OrchestratorSpacing {
  pub const ALL: [Self; 7] = [
    Self::Start,
    Self::PageSide,
    Self::Items,
    Self::Nested,
    Self::Report,
    Self::LinkColumns,
    Self::StartComposer,
  ];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Start => "orchestrator_start",
      Self::PageSide => "orchestrator_page_side",
      Self::Items => "orchestrator_items",
      Self::Nested => "orchestrator_nested",
      Self::Report => "orchestrator_report",
      Self::LinkColumns => "orchestrator_link_columns",
      Self::StartComposer => "orchestrator_start_composer",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::Start => 20.0,
      Self::PageSide => 40.0,
      Self::Items => 18.0,
      Self::Nested => 46.0,
      Self::Report => 36.0,
      Self::LinkColumns => 12.0,
      Self::StartComposer => 28.0,
    }
  }
}

/// `base`, the theme's scrollbar, for the chat's turns: in a lane of its own, so the thumb never covers a message's
/// end.
pub fn chat_scrollbar(base: &ScrollBarStyle) -> ScrollBarStyle {
  ScrollBarStyle {
    placement: ScrollBarPlacement::Reserved,
    ..base.clone()
  }
}

/// The chat's side (`ECQg8`) less the lane `scrollbar` keeps at the turns' right, so their column stays centred on
/// the chat, as the composer's is, wherever the lane takes room.
pub fn beside_chat_lane(scrollbar: &ScrollBarStyle) -> f32 {
  OrchestratorSpacing::PageSide.value() - (scrollbar.width + scrollbar.resolved_edge_inset() * 2.0)
}

/// The runs-on picker's models' scrollbar (`Scrollbar` `Px0Pz` in `GaA7N`): 3 wide, radius 2, at the list's end, 2
/// from its top and bottom.
const PICKER_SCROLLBAR_WIDTH: f32 = 3.0;
const PICKER_SCROLLBAR_RADIUS: f32 = 2.0;
const PICKER_SCROLLBAR_END: f32 = 2.0;

/// `base`, the theme's scrollbar (its `border_strong` thumb), as the runs-on picker's models' list draws it.
pub fn picker_scrollbar(base: &ScrollBarStyle) -> ScrollBarStyle {
  ScrollBarStyle {
    width: PICKER_SCROLLBAR_WIDTH,
    thumb_radius: PICKER_SCROLLBAR_RADIUS,
    track_radius: PICKER_SCROLLBAR_RADIUS,
    edge_inset: Some(0.0),
    end_inset: Some(PICKER_SCROLLBAR_END),
    ..base.clone()
  }
}

/// Shadows of the Orchestrator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrchestratorShadow {
  /// -8 0 24 of `shadow`: History over the chat's right edge, cast to its left (`eFI39`).
  History,
}

impl OrchestratorShadow {
  pub fn role(self) -> ShadowStyle {
    ShadowStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::History => "orchestrator_history",
    }
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  OrchestratorText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}

pub(super) fn shadows() -> impl Iterator<Item = (Arc<str>, Arc<[BoxShadow]>)> {
  let history = BoxShadow::new(-8.0, 0.0, 24.0, ExtraColor::Shadow.role());
  std::iter::once((Arc::from(OrchestratorShadow::History.name()), Arc::from([history])))
}

pub(super) fn install(theme: &Theme) {
  for spacing in OrchestratorSpacing::ALL {
    theme.set_spacing_value(spacing.role(), spacing.value());
  }
}
