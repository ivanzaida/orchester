//! Values of a composer's slash menu (ORC-321, ORC-325; `Slash menu` `IQOLN` in `07 · Orchestrator · slash menu`
//! `jAiyu`, `v7Xwo` in `06 · Conversation · slash menu` `yKWQx`, the rows and the skill chip in `06d · Slash menu ·
//! parts and rules` `lXQi1`).

use std::sync::Arc;

use lurq::{
  app::theme::{ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::{mono, style};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlashText {
  /// 11/500 `text_muted`, 1.3: a section's title, *Commands* (`WG5ZV`), *Skills* (`qLLLv`).
  Section,
  /// 11 `text_muted`, 1.3: what a section's head says at its end (`cvxZd`), the keys' words and the note in the
  /// footer (`rkDuL`, `VzkPa`).
  Note,
  /// 12 `text_primary`, 1.3: a command's name (`mSV2x`), the no-match title.
  Label,
  /// 12/600 `text_primary`, 1.3: the letters of a name that match what was typed (`Match` in `OKWFL`); the rest of
  /// a filtered name is [`SlashText::Label`] in `text_secondary`.
  Match,
  /// 12 `text_muted`, 1.3: what a command does (`Bu9RV`); why one cannot run is in `text_secondary`.
  Detail,
  /// Geist Mono 12 `text_primary`, 1.3: a skill's name (`cLPAs`); the rest of a filtered one in `text_secondary`.
  SkillLabel,
  /// Geist Mono 12/600 `text_primary`, 1.3: the letters of a skill's name that match what was typed (`zRg3n`); in the
  /// @ menu, those of a file's path (`Match` `O0RYH` in `nUjD7`).
  SkillMatch,
  /// Geist Mono 11 `text_secondary`, 1.3: a skill chip's name (`LTjMB` in `o6R6w`).
  Chip,
  /// 12/500 `text_secondary`, 1.2: a command badge's verb, *Watch* (`v4W6sK` in `VvRWU`).
  BadgeVerb,
  /// 12 `text_primary`, 1.2: what a command badge holds, *Orchestrator watch* (`ejUxQ`); its `·` in `text_muted`.
  BadgeTarget,
  /// 11 `text_secondary`, 1.4: a command badge's *+N* (`NB842` in `BaYA3`).
  BadgeMore,
}

impl SlashText {
  pub const ALL: [Self; 11] = [
    Self::Section,
    Self::Note,
    Self::Label,
    Self::Match,
    Self::Detail,
    Self::SkillLabel,
    Self::SkillMatch,
    Self::Chip,
    Self::BadgeVerb,
    Self::BadgeTarget,
    Self::BadgeMore,
  ];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Section => "slash_section",
      Self::Note => "slash_note",
      Self::Label => "slash_label",
      Self::Match => "slash_match",
      Self::Detail => "slash_detail",
      Self::SkillLabel => "slash_skill_label",
      Self::SkillMatch => "slash_skill_match",
      Self::Chip => "slash_chip",
      Self::BadgeVerb => "slash_badge_verb",
      Self::BadgeTarget => "slash_badge_target",
      Self::BadgeMore => "slash_badge_more",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::Section => style(11.0, FontWeight::Medium, 1.3, 0.0, palette.text_muted),
      Self::Note => style(11.0, FontWeight::Normal, 1.3, 0.0, palette.text_muted),
      Self::Label => style(12.0, FontWeight::Normal, 1.3, 0.0, palette.text_primary),
      Self::Match => style(12.0, FontWeight::SemiBold, 1.3, 0.0, palette.text_primary),
      Self::Detail => style(12.0, FontWeight::Normal, 1.3, 0.0, palette.text_muted),
      Self::SkillLabel => mono(style(12.0, FontWeight::Normal, 1.3, 0.0, palette.text_primary)),
      Self::SkillMatch => mono(style(12.0, FontWeight::SemiBold, 1.3, 0.0, palette.text_primary)),
      Self::Chip => mono(style(11.0, FontWeight::Normal, 1.3, 0.0, palette.text_secondary)),
      Self::BadgeVerb => style(12.0, FontWeight::Medium, 1.2, 0.0, palette.text_secondary),
      Self::BadgeTarget => style(12.0, FontWeight::Normal, 1.2, 0.0, palette.text_primary),
      Self::BadgeMore => style(11.0, FontWeight::Normal, 1.4, 0.0, palette.text_secondary),
    }
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  SlashText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}
