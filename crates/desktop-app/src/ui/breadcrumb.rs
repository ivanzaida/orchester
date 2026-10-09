//! Breadcrumb (`Crumbs` `A7q3o` on the task page, `VpX0E` on New task): where a page sits, its crumbs
//! in 12 `text_muted`, 6 apart, the page's own last in `text_secondary`; a key in Geist Mono (`KON-12`).
//! The task page separates crumbs with a `/` in 12 `text_muted`, New task with a 12 `chevron-right`:
//! the boards differ, so the caller picks. A crumb that opens its page is underlined under the pointer.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, TypographyStyle},
  },
  components::{Row, Text},
  layout::Alignment,
  node::{Element, Style, border::Border, dimension::Dimension},
};

use super::{
  icon::{Icon, icon},
  pressable::pressable,
  tooltip::Truncated,
};
use crate::theme::{ExtraSpacing, IconSize};

/// The task page's separator: a mark, the same in every language.
const SLASH: &str = "/";

/// What stands between two crumbs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrumbSeparator {
  Slash,
  Chevron,
}

pub struct Crumb {
  label: Arc<str>,
  mono: bool,
  open: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl Crumb {
  /// A crumb that opens its page.
  pub fn link(label: impl Into<Arc<str>>, open: impl Fn() + Send + Sync + 'static) -> Self {
    Self {
      label: label.into(),
      mono: false,
      open: Some(Arc::new(open)),
    }
  }

  /// The page itself, the last crumb.
  pub fn here(label: impl Into<Arc<str>>) -> Self {
    Self {
      label: label.into(),
      mono: false,
      open: None,
    }
  }

  /// The label is an identifier, e.g. a task's key.
  pub fn mono(mut self) -> Self {
    self.mono = true;
    self
  }

  /// The crumb, cut to its room with its whole label in a tooltip; a link's label is `<id>-label`.
  fn render(self, ctx: &mut Ctx, id: String, last: bool) -> Element {
    let typography = if self.mono {
      TypographyStyle::Mono
    } else {
      TypographyStyle::Description
    };
    let color = if last {
      PaletteColor::TextSecondary
    } else {
      PaletteColor::TextMuted
    };
    match self.open {
      Some(open) => {
        let name = self.label.clone();
        let text = Truncated::new(format!("{id}-label"), self.label, typography)
          .color(color)
          .render(ctx);
        pressable(move || open())
          .id(id)
          .min_width(Dimension::Px(0.0))
          .describe("a11y_name", name)
          .hovered_style(Style::new().border_bottom(Border::inside(BorderSize::Sm, PaletteColor::TextMuted)))
          .child(text)
          .into()
      }
      None => Truncated::new(id, self.label, typography).color(color).render(ctx),
    }
  }
}

/// `crumbs` in order, the page itself last; crumb `n` has the element id `<id>-<n>`, the separator
/// before it `<id>-separator-<n>`.
pub fn breadcrumb(ctx: &mut Ctx, id: &str, separator: CrumbSeparator, crumbs: Vec<Crumb>) -> Row {
  let count = crumbs.len();
  let mut row = Row::new()
    .id(id.to_owned())
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center);
  for (index, crumb) in crumbs.into_iter().enumerate() {
    if index > 0 {
      let mark = match separator {
        CrumbSeparator::Slash => Text::new(SLASH)
          .variant(TypographyStyle::Description)
          .color(PaletteColor::TextMuted),
        CrumbSeparator::Chevron => icon(Icon::ChevronRight, IconSize::Sm, PaletteColor::TextMuted),
      };
      row = row.child(mark.id(format!("{id}-separator-{index}")));
    }
    row = row.child(crumb.render(ctx, format!("{id}-{index}"), index + 1 == count));
  }
  row
}
