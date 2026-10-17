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
  pressable::Press,
  tooltip::Truncated,
};
use crate::{
  navigation::link::Open,
  theme::{ExtraSpacing, IconSize},
};

/// The most room an ancestor crumb takes: a longer name is cut there with an ellipsis. Ancestors keep
/// their room and the page's own crumb gives up what the row lacks, so no crumb is squeezed away.
const ANCESTOR_MAX_WIDTH: f32 = 160.0;

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
  typography: TypographyStyle,
  open: Option<Press>,
  ancestor: bool,
}

impl Crumb {
  /// A crumb that opens its page, a place of the window, with `open` by a browser's rules (`navigation::link`).
  pub fn to(label: impl Into<Arc<str>>, open: Open) -> Self {
    Self::pressed(label, Press::Open(open))
  }

  /// A crumb that goes back to a panel of its page, which is no place of its own: the page shows it in place.
  pub fn link(label: impl Into<Arc<str>>, open: impl Fn() + Send + Sync + 'static) -> Self {
    Self::pressed(label, Press::run(open))
  }

  fn pressed(label: impl Into<Arc<str>>, press: Press) -> Self {
    Self {
      label: label.into(),
      typography: TypographyStyle::Description,
      open: Some(press),
      ancestor: false,
    }
  }

  /// The page itself, the last crumb.
  pub fn here(label: impl Into<Arc<str>>) -> Self {
    Self {
      label: label.into(),
      typography: TypographyStyle::Description,
      open: None,
      ancestor: false,
    }
  }

  /// The label is an identifier, e.g. a task's key.
  pub fn mono(mut self) -> Self {
    self.typography = TypographyStyle::Mono;
    self
  }

  /// An ancestor of the page even when it ends the trail (the code viewer's folders, whose page names the file
  /// in its title): in `text_muted` like the others (`gxF9b` in `o1Uea4`).
  pub fn ancestor(mut self) -> Self {
    self.ancestor = true;
    self
  }

  /// The page title in the last crumb (Effective instructions, c3yZoo).
  pub fn title(mut self) -> Self {
    self.typography = TypographyStyle::Title;
    self
  }

  /// The crumb, cut to its room with its whole label in a tooltip; a link's label is `<id>-label`.
  fn render(self, ctx: &mut Ctx, id: String, last: bool) -> Element {
    let typography = self.typography;
    let color = if self.ancestor {
      PaletteColor::TextMuted
    } else if last && typography == TypographyStyle::Title {
      PaletteColor::TextPrimary
    } else if last {
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
        open
          .button()
          .id(id)
          .flex_shrink(0.0)
          .max_width(Dimension::Px(ANCESTOR_MAX_WIDTH))
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
    .max_width(Dimension::Pct(100.0))
    .min_width(Dimension::Px(0.0))
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
