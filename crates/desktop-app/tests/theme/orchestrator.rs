//! The Orchestrator's own type roles and spacing (`hujxu`, `zuvUi` in the new orchester.pen; `Xir64`), its replies'
//! links (`agv8r`), and the context meter and the slash menu in its composer.
use orchester_app::theme::{ContextText, SlashText};

use super::*;

#[test]
fn orchestrator_roles_follow_the_design() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  for (role, size, line, weight, family) in [
    // The header's title (`B9kNtt`), *What should we plan?* (`gOmoZ`).
    (OrchestratorText::HeaderTitle, 14.0, 1.3, FontWeight::SemiBold, "Geist"),
    (OrchestratorText::StartTitle, 24.0, 1.3, FontWeight::SemiBold, "Geist"),
    (OrchestratorText::Emphasis, 12.5, 1.35, FontWeight::Medium, "Geist"),
    // A day's label in History (`wpZj3`).
    (OrchestratorText::Eyebrow, 10.5, 1.3, FontWeight::Normal, "Geist Mono"),
    (OrchestratorText::Suggestion, 12.0, 1.4, FontWeight::Normal, "Geist"),
    // What an event line tells (`VGbU7`), what a decision card asks (`bxJZp`).
    (OrchestratorText::Line, 12.5, 1.3, FontWeight::Normal, "Geist"),
    (OrchestratorText::Question, 13.0, 1.3, FontWeight::SemiBold, "Geist"),
    // A plan's use in the runs-on picker (`qSIt9`).
    (OrchestratorText::PlanUse, 10.5, 1.3, FontWeight::Normal, "Geist Mono"),
    (OrchestratorText::FactKey, 12.0, 1.3, FontWeight::Normal, "Geist"),
    // A History row's meta (`r9TeH`) and a result's matching line (`srL91`), the composer's message (`lB97V`).
    (OrchestratorText::HistoryMeta, 11.0, 1.3, FontWeight::Normal, "Geist"),
    (OrchestratorText::HistoryLine, 11.5, 1.4, FontWeight::Normal, "Geist"),
    (OrchestratorText::Message, 13.0, 1.3, FontWeight::Normal, "Geist"),
    // *Plan only* on the composer's Mode line (`xptH1` in `y51fw9`).
    (OrchestratorText::Mode, 12.0, 1.3, FontWeight::Medium, "Geist"),
    // *Settings ↗* at the runs-on picker's foot (`rIT51`).
    (OrchestratorText::PickerLink, 12.0, 1.3, FontWeight::Normal, "Geist"),
    // What the card of a target another conversation leads says under its title (`kjs5n` in `f1J8z`).
    (OrchestratorText::CardNote, 11.5, 1.5, FontWeight::Normal, "Geist"),
    // A watch command's order (`oUZqo`), a line's aside and its end (`vYl6I`, `Hoj8n` in `avDZO`).
    (OrchestratorText::CommandOrder, 13.0, 1.55, FontWeight::Normal, "Geist"),
    (OrchestratorText::LineAside, 12.0, 1.3, FontWeight::Normal, "Geist"),
    (OrchestratorText::LineEnd, 12.0, 1.3, FontWeight::Normal, "Geist"),
    // A Leads chip's name (`oVMRX`) and its tooltip's lines (`f4DCkr`, `X48YP`, `j1E7E` in `X93D8T`).
    (OrchestratorText::LeadsChip, 11.5, 1.3, FontWeight::Normal, "Geist"),
    (
      OrchestratorText::LeadsTipTitle,
      11.0,
      1.3,
      FontWeight::SemiBold,
      "Geist",
    ),
    (OrchestratorText::LeadsTipOrder, 11.0, 1.45, FontWeight::Normal, "Geist"),
    (OrchestratorText::LeadsTipHint, 11.0, 1.3, FontWeight::Normal, "Geist"),
  ] {
    let style = theme.typography_style(role.role());
    assert_eq!(
      (style.font_size, style.line_height, style.weight, &*style.font_family),
      (size, line, weight, family),
      "{role:?}"
    );
  }
  assert_eq!(
    theme.typography_style(OrchestratorText::Eyebrow.role()).letter_spacing,
    0.6
  );
  for (spacing, value) in [
    (OrchestratorSpacing::Start, 20.0),
    (OrchestratorSpacing::PageSide, 40.0),
    (OrchestratorSpacing::Items, 18.0),
    (OrchestratorSpacing::Nested, 46.0),
    (OrchestratorSpacing::Report, 36.0),
    // A new conversation's message over its composer's bar (`QcfyZ`).
    (OrchestratorSpacing::StartComposer, 28.0),
    // What a stop left open, in from its line's start (`j6XFdt`).
    (OrchestratorSpacing::LeftOpen, 27.0),
  ] {
    assert_eq!(theme.spacing_value(spacing.role()), Dimension::Px(value), "{spacing:?}");
  }
}

#[test]
fn orchestrator_link_roles_follow_the_link_states() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  for (role, family) in [
    (OrchestratorText::LinkHost, "Geist"),
    (OrchestratorText::LinkAddress, "Geist Mono"),
  ] {
    let style = theme.typography_style(role.role());
    assert_eq!(
      (style.font_size, style.weight, &*style.font_family, style.color),
      (11.5, FontWeight::Normal, family, Color::from_hex("#8A8A92")),
      "{role:?} in `text_muted`"
    );
  }
  let link = theme
    .markdown()
    .link
    .apply(&theme.typography_style(TypographyStyle::Body));
  assert_eq!(
    (link.weight, link.color),
    (FontWeight::Medium, Color::from_hex("#EDEDF0")),
    "a link's words in `accent_text` 500"
  );
  let links = theme.markdown();
  assert!(
    links.link_focus_ring.is_none(),
    "a Markdown link takes no keyboard focus, so it draws no ring"
  );
  assert_eq!(
    (links.link_tooltip_box.background, links.link_tooltip_box.radius),
    (Some(Color::from_hex("#EDEDF0")), Some(4.0)),
    "a link's tooltip as the kit's, on `text_primary`"
  );
}

#[test]
fn orchestrator_chat_link_looks_follow_the_link_states() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  let looks = ChatLinkLooks::of(theme, &theme.typography_style(TypographyStyle::Body));
  let underline = looks.link.text.underline.expect("a reply's link is underlined");
  assert_eq!(
    (underline.color, underline.thickness),
    (Some(Color::from_hex("#8A8A92")), Some(1.0)),
    "a 1 `text_muted` underline"
  );
  let body = theme.typography_style(TypographyStyle::Body);
  let box_below_baseline = (body.line_height - 1.3) / 2.0 * body.font_size + 0.295 * body.font_size;
  assert!(
    underline
      .offset
      .is_some_and(|offset| (offset + 1.0 - box_below_baseline).abs() < 0.001),
    "the underline ends where the line box, and a hovered link's fill, ends: {underline:?}"
  );
  assert_eq!(
    (
      looks.hovered.color,
      looks.hovered.underline_color,
      looks.hovered.background,
      looks.hovered.trailing_color
    ),
    (
      Some(Color::from_hex("#FFFFFF")),
      Some(Color::from_hex("#FFFFFF")),
      Some(Color::from_hex("#FFFFFF0F")),
      Some(Color::from_hex("#B3B3BA"))
    ),
    "under the pointer `accent_hover` on `hover`, the host `text_secondary`"
  );
  assert_eq!(
    looks.radius.to_array(),
    [3.0, 3.0, 0.0, 0.0],
    "square at the underline's side"
  );
  for (gap, width) in [(&looks.host_gap, 6.0), (&looks.icon_gap, 3.0)] {
    let size = gap.text.font_size.expect("a gap's size");
    let spacing = gap.text.letter_spacing.expect("a gap's spacing");
    assert!(
      (size * 0.25 + spacing - width).abs() < 0.01,
      "a space widened to {width}"
    );
  }
}

#[test]
fn orchestrator_labelled_link_roles_follow_the_grid() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  let label = theme.typography_style(OrchestratorText::LinkLabel.role());
  assert_eq!((label.font_size, label.color), (12.0, Color::from_hex("#8A8A92")));
  let detail = theme.typography_style(OrchestratorText::LinkDetail.role());
  assert_eq!(
    (detail.font_size, detail.line_height, detail.color),
    (12.5, 1.5, Color::from_hex("#B3B3BA"))
  );
  assert_eq!(
    theme.spacing_value(OrchestratorSpacing::LinkColumns.role()),
    Dimension::Px(12.0)
  );
}

/// The context meter in the Orchestrator's composer and on a run's step at work (`Context meter · states` `Kp4IZ`,
/// its popovers `v6vdj` and `OZGiw`).
#[test]
fn context_meter_roles_follow_the_design() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  let mono = theme.typography_style(ExtraTypography::MonoNote.role()).font_family;
  for (role, size, weight, in_mono) in [
    // The share (`kYtZG`), *Full* (`kqwKb`), *Context* (`C0x35f`), the model (`hncjV`), the tokens (`DtYpM`), a note
    // (`YwJOp`).
    (ContextText::Share, 11.0, FontWeight::Normal, true),
    (ContextText::Full, 11.0, FontWeight::Normal, false),
    (ContextText::Title, 12.0, FontWeight::SemiBold, false),
    (ContextText::Model, 11.0, FontWeight::Normal, true),
    (ContextText::Tokens, 12.0, FontWeight::Normal, false),
    (ContextText::Note, 11.0, FontWeight::Normal, false),
  ] {
    let style = theme.typography_style(role.role());
    assert_eq!(
      (style.font_size, style.weight, style.font_family == mono),
      (size, weight, in_mono),
      "{role:?}"
    );
  }
  assert_eq!(
    theme.typography_style(ContextText::Full.role()).color,
    theme.palette_color(PaletteColor::Danger),
    "kqwKb"
  );
}

/// A composer's slash menu (`Slash menu` `IQOLN`, filtered `OKWFL`) and its skill chip (`o6R6w`): all 1.3, a
/// skill's name in Geist Mono.
#[test]
fn slash_menu_roles_follow_the_design() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  for (role, size, weight, color, family) in [
    // *Commands* (`WG5ZV`), a head's or the footer's words (`cvxZd`, `VzkPa`), a name (`mSV2x`), its matched letters
    // (`Match`), what it does (`Bu9RV`); a skill's name (`cLPAs`), its matched letters, and the chip's (`LTjMB`).
    (
      SlashText::Section,
      11.0,
      FontWeight::Medium,
      PaletteColor::TextMuted,
      "Geist",
    ),
    (
      SlashText::Note,
      11.0,
      FontWeight::Normal,
      PaletteColor::TextMuted,
      "Geist",
    ),
    (
      SlashText::Label,
      12.0,
      FontWeight::Normal,
      PaletteColor::TextPrimary,
      "Geist",
    ),
    (
      SlashText::Match,
      12.0,
      FontWeight::SemiBold,
      PaletteColor::TextPrimary,
      "Geist",
    ),
    (
      SlashText::Detail,
      12.0,
      FontWeight::Normal,
      PaletteColor::TextMuted,
      "Geist",
    ),
    (
      SlashText::SkillLabel,
      12.0,
      FontWeight::Normal,
      PaletteColor::TextPrimary,
      "Geist Mono",
    ),
    (
      SlashText::SkillMatch,
      12.0,
      FontWeight::SemiBold,
      PaletteColor::TextPrimary,
      "Geist Mono",
    ),
    (
      SlashText::Chip,
      11.0,
      FontWeight::Normal,
      PaletteColor::TextSecondary,
      "Geist Mono",
    ),
    // A command picker's own search (`slpmj` in `P1` `QXPDY`).
    (
      SlashText::Search,
      12.5,
      FontWeight::Normal,
      PaletteColor::TextPrimary,
      "Geist",
    ),
  ] {
    let style = theme.typography_style(role.role());
    assert_eq!(
      (
        style.font_size,
        style.line_height,
        style.weight,
        style.color,
        &*style.font_family
      ),
      (size, 1.3, weight, theme.palette_color(color), family),
      "{role:?}"
    );
  }
}
