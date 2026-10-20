//! Values of the Memory screens: the task's Memory tab (`Task · Memory · dark` `a0PLO` in `g5dmWI`) and the
//! project's Memory page (`Memory · project page` `xY2Uj` in `x4pFb`), with their components (`Memory / Record row`
//! `J1utD`, `Memory / Proposal row` `LdHs2`, the record editors `t0jt2k` and `MO5Mu`, `Memory / Delete confirmation`
//! `YdVse`, `Memory / Given row` `Q0BAI`, `Memory / Group heading` `CJvK7`).
use std::sync::Arc;

use lurq::{
  app::theme::{SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::{
  ExtraColor,
  typography::{mono, style},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryText {
  /// 10.5/500 `text_muted`, tracking 0.5: a group's heading (`fqONf` in `CJvK7`).
  GroupLabel,
  /// Geist Mono 10.5 `text_muted`: a group's count (`ZJUZb` in `CJvK7`).
  GroupCount,
  /// 10.5/500 `text_secondary`: the latest handoff's flag (`J7IR4` in `J1utD`).
  Flag,
  /// 11.5/500 `text_muted`: the label of a handoff's list (`HzwHN` in `J1utD`).
  ListLabel,
  /// 11.5 `text_secondary`, 1.45: a notice's text in an editor (`vgQVY` in `t0jt2k`), what deprecating
  /// keeps (`J617rZ` in `YdVse`).
  NoticeText,
  /// 11.5/500 `accent_text`: a link under a panel or an editor's fact (`RkGXO`, `I7UAFO`).
  Link,
  /// 10.5 `text_muted`: the scope beside a record the next agent gets (`U93fCI` in `Q0BAI`).
  Scope,
  /// 13 `text_secondary`, 1.4: what the project's page says under its title (`lzVXB` in `xY2Uj`).
  Lead,
  /// 14/600 `text_primary`: the title of the project's panel or of a record's inspector (`wJDd2`, `jIMDn`).
  PanelTitle,
}

impl MemoryText {
  pub const ALL: [Self; 9] = [
    Self::GroupLabel,
    Self::GroupCount,
    Self::Flag,
    Self::ListLabel,
    Self::NoticeText,
    Self::Link,
    Self::Scope,
    Self::Lead,
    Self::PanelTitle,
  ];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::GroupLabel => "memory_group_label",
      Self::GroupCount => "memory_group_count",
      Self::Flag => "memory_flag",
      Self::ListLabel => "memory_list_label",
      Self::NoticeText => "memory_notice_text",
      Self::Link => "memory_link",
      Self::Scope => "memory_scope",
      Self::Lead => "memory_lead",
      Self::PanelTitle => "memory_panel_title",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::GroupLabel => style(10.5, FontWeight::Medium, 1.3, 0.5, palette.text_muted),
      Self::GroupCount => mono(style(10.5, FontWeight::Normal, 1.3, 0.0, palette.text_muted)),
      Self::Flag => style(10.5, FontWeight::Medium, 1.3, 0.0, palette.text_secondary),
      Self::ListLabel => style(11.5, FontWeight::Medium, 1.45, 0.0, palette.text_muted),
      Self::NoticeText => style(11.5, FontWeight::Normal, 1.45, 0.0, palette.text_secondary),
      Self::Link => style(
        11.5,
        FontWeight::Medium,
        1.3,
        0.0,
        palette.get(ExtraColor::AccentText.role()),
      ),
      Self::Scope => style(10.5, FontWeight::Normal, 1.3, 0.0, palette.text_muted),
      Self::Lead => style(13.0, FontWeight::Normal, 1.4, 0.0, palette.text_secondary),
      Self::PanelTitle => style(14.0, FontWeight::SemiBold, 1.3, 0.0, palette.text_primary),
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemorySpacing {
  /// 18: between the groups of the task's list (`sglU2`).
  Groups,
  /// 28: a proposal's actions in line with its text, past its 16 glyph and the 12 gap (`foPMI` in `LdHs2`).
  TextIndent,
  /// 20: the sides of the task's record list (`sglU2`).
  ListSide,
}

impl MemorySpacing {
  pub const ALL: [Self; 3] = [Self::Groups, Self::TextIndent, Self::ListSide];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Groups => "memory_groups",
      Self::TextIndent => "memory_text_indent",
      Self::ListSide => "memory_list_side",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::Groups => 18.0,
      Self::TextIndent => 28.0,
      Self::ListSide => 20.0,
    }
  }
}

pub(super) fn install(theme: &Theme) {
  for spacing in MemorySpacing::ALL {
    theme.set_spacing_value(spacing.role(), spacing.value());
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  MemoryText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}
