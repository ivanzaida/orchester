//! Values of the agent's live todo list (`Live todo list · redesign` `Ucmyu`, its parts and states `zE3lp`) that the
//! UI kit's roles do not carry: the pinned box over a chat's composer, its items, and the chip a step's divider or an
//! answer's name line folds it into (`docs/design/live-todo-list.md`).

use std::sync::Arc;

use lurq::{
  app::theme::{ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::{mono, style};

/// Text of the todo list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TodoText {
  /// 12/500 `text_primary`, 1.3: *Plan* in the box's head (`nzjt8` in `Y8rS8W`).
  Title,
  /// Geist Mono 11 `text_muted`, 1.3: the count, done of all (`KZoI5`).
  Count,
  /// 11.5 `text_muted`, 1.3: who keeps it and in which step (`yqoQa`).
  Owner,
  /// 12.5 `text_secondary`, 1.4: an item (`zZ87W`); a done item in `text_muted` (`IWAoP`).
  Item,
  /// 12.5/500 `text_primary`, 1.4: the item in progress (`GXkof`).
  Current,
  /// 12/500 `text_secondary`, 1.3: *Show*, *Show all*, *Show less* (`AISCX`, `j48U8`, `Cfm2h`).
  Action,
  /// 12 `text_muted`, 1.3: what a step left undone when it stopped (`kpgG7` in `gt3Yh`).
  Note,
  /// 11/500 `text_secondary`, 1.3: the chip's *Plan · 7 of 7* (`h2VkQT` in `bvhZU`).
  Chip,
}

impl TodoText {
  pub const ALL: [Self; 8] = [
    Self::Title,
    Self::Count,
    Self::Owner,
    Self::Item,
    Self::Current,
    Self::Action,
    Self::Note,
    Self::Chip,
  ];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Title => "todo_title",
      Self::Count => "todo_count",
      Self::Owner => "todo_owner",
      Self::Item => "todo_item",
      Self::Current => "todo_current",
      Self::Action => "todo_action",
      Self::Note => "todo_note",
      Self::Chip => "todo_chip",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::Title => style(12.0, FontWeight::Medium, 1.3, 0.0, palette.text_primary),
      Self::Count => mono(style(11.0, FontWeight::Normal, 1.3, 0.0, palette.text_muted)),
      Self::Owner => style(11.5, FontWeight::Normal, 1.3, 0.0, palette.text_muted),
      Self::Item => style(12.5, FontWeight::Normal, 1.4, 0.0, palette.text_secondary),
      Self::Current => style(12.5, FontWeight::Medium, 1.4, 0.0, palette.text_primary),
      Self::Action => style(12.0, FontWeight::Medium, 1.3, 0.0, palette.text_secondary),
      Self::Note => style(12.0, FontWeight::Normal, 1.3, 0.0, palette.text_muted),
      Self::Chip => style(11.0, FontWeight::Medium, 1.3, 0.0, palette.text_secondary),
    }
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  TodoText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}
