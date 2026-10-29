//! A source in a list (`Source row` `Q5DCn`): its kind's 14 `text_secondary` glyph 2 down, then its path in Geist
//! Mono 12.5, an optional note in 12/1.45 `text_secondary` and a meta line in 11 `text_muted`, 3 apart; its status
//! (a 13 glyph and a 12 word, 5 apart, `danger` unless it is available) and a remove cross, or the lock of a
//! source listed elsewhere; 12 apart and 12 over and under, a `border` line under it. The compact one
//! (`Source row/compact` `KsqMM`, an inspector's list and a run's snapshot) has a 13 glyph, the path in Geist Mono
//! 12 that wraps and its meta, 8 apart and 6 over and under, without status or line.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::sources::{check::Kind, listed::State};

use super::{
  icon::{Icon, icon},
  icon_button::IconButton,
  tooltip::Truncated,
};
use crate::theme::{ExtraSpacing, ExtraTypography, IconSize, SourcesText};

type Handler = Arc<dyn Fn() + Send + Sync>;

/// What ends a row.
enum Trailing {
  None,
  /// The cross that takes the source off its list, named this for tools.
  Remove(Arc<str>, Handler),
  /// The source is listed elsewhere: it is not removed here.
  Lock,
}

pub struct SourceRow {
  id: Arc<str>,
  path: Arc<str>,
  kind: Option<Kind>,
  note: Option<Arc<str>>,
  meta: Arc<str>,
  status: Option<State>,
  trailing: Trailing,
  compact: bool,
}

impl SourceRow {
  /// The row of the source at `path`, which names `kind` (`None` while nothing is there); `meta` says what it is and
  /// where it is listed. The path's element id is `<id>-path`.
  pub fn new(
    id: impl Into<Arc<str>>,
    path: impl Into<Arc<str>>,
    kind: Option<Kind>,
    meta: impl Into<Arc<str>>,
  ) -> Self {
    Self {
      id: id.into(),
      path: path.into(),
      kind,
      note: None,
      meta: meta.into(),
      status: None,
      trailing: Trailing::None,
      compact: false,
    }
  }

  /// A line between the path and the meta: what the source is for, or what runs do while it is missing.
  pub fn note(mut self, note: impl Into<Arc<str>>) -> Self {
    self.note = Some(note.into());
    self
  }

  /// Whether runs can read it now.
  pub fn status(mut self, state: State) -> Self {
    self.status = Some(state);
    self
  }

  /// A cross at its end, named `label`, that runs `remove`.
  pub fn removable(mut self, label: impl Into<Arc<str>>, remove: impl Fn() + Send + Sync + 'static) -> Self {
    self.trailing = Trailing::Remove(label.into(), Arc::new(remove));
    self
  }

  /// A lock at its end: it is listed elsewhere and changed there.
  pub fn locked(mut self) -> Self {
    self.trailing = Trailing::Lock;
    self
  }

  /// The compact row of an inspector.
  pub fn compact(mut self) -> Self {
    self.compact = true;
    self
  }

  pub fn render(self, ctx: &mut Ctx) -> Element {
    if self.compact {
      return self.compact_row(ctx);
    }
    let path = Truncated::new(format!("{}-path", self.id), self.path.clone(), SourcesText::Path.role())
      .mono()
      .render(ctx);
    let text = Column::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .spacing(ExtraSpacing::Fine.role())
      // In a row of its own the path gives up room and ends in an ellipsis, its whole value in a tooltip.
      .child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .min_width(Dimension::Px(0.0))
          .child(path),
      )
      .with_children(self.note.as_deref().map(|note| {
        Text::new(note)
          .id(format!("{}-note", self.id))
          .variant(ExtraTypography::Prose.role())
          .width(Dimension::Pct(100.0))
      }))
      .child(
        Text::new(&self.meta)
          .id(format!("{}-meta", self.id))
          .variant(TypographyStyle::Caption),
      );
    let status = self.status.map(|state| status(ctx, &self.id, state));
    let trailing = trailing(ctx, &self.id, self.trailing);
    Row::new()
      .id(self.id.to_string())
      .width(Dimension::Pct(100.0))
      .padding(Padding::vertical(SpacingSize::Md))
      .spacing(SpacingSize::Md)
      .align_items(Alignment::Start)
      .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .child(
        Column::new()
          .padding(Padding::new().top(ExtraSpacing::Tight.role()))
          .child(icon(kind_icon(self.kind), IconSize::Md, PaletteColor::TextSecondary)),
      )
      .child(text)
      .with_children(status)
      .with_children(trailing)
      .into()
  }

  fn compact_row(self, ctx: &mut Ctx) -> Element {
    let trailing = trailing(ctx, &self.id, self.trailing);
    Row::new()
      .id(self.id.to_string())
      .width(Dimension::Pct(100.0))
      .padding(Padding::vertical(ExtraSpacing::Snug.role()))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Start)
      .child(
        Column::new()
          .padding(Padding::new().top(ExtraSpacing::Hair.role()))
          .child(icon(kind_icon(self.kind), IconSize::Base, PaletteColor::TextSecondary)),
      )
      .child(
        Column::new()
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .spacing(ExtraSpacing::Tight.role())
          .child(
            Text::new(&self.path)
              .id(format!("{}-path", self.id))
              .variant(TypographyStyle::Mono)
              .width(Dimension::Pct(100.0)),
          )
          .child(
            Text::new(&self.meta)
              .id(format!("{}-meta", self.id))
              .variant(TypographyStyle::Caption)
              .width(Dimension::Pct(100.0)),
          ),
      )
      .with_children(trailing)
      .into()
  }
}

fn kind_icon(kind: Option<Kind>) -> Icon {
  match kind {
    Some(Kind::Folder) => Icon::Folder,
    Some(Kind::File) => Icon::FileText,
    None => Icon::File,
  }
}

/// A source's status: `circle-check` in `success` while runs read it, `file-x` or `ban` in `danger` otherwise.
fn status(ctx: &Ctx, id: &str, state: State) -> Column {
  let (glyph, mark, color, key) = match state {
    State::Available => (
      Icon::CircleCheck,
      PaletteColor::Success,
      PaletteColor::TextSecondary,
      "ui.source.available",
    ),
    State::Missing => (
      Icon::FileX,
      PaletteColor::Danger,
      PaletteColor::Danger,
      "ui.source.missing",
    ),
    State::Unreadable => (
      Icon::Ban,
      PaletteColor::Danger,
      PaletteColor::Danger,
      "ui.source.unreadable",
    ),
  };
  Column::new()
    .padding(Padding::new().top(ExtraSpacing::Hair.role()))
    .child(
      Row::new()
        .id(format!("{id}-status"))
        .spacing(ExtraSpacing::Pair.role())
        .align_items(Alignment::Center)
        .describe("state", state_name(state))
        .child(icon(glyph, IconSize::Base, mark))
        .child(
          Text::new(&ctx.t(key))
            .variant(TypographyStyle::Description)
            .color(color),
        ),
    )
}

const fn state_name(state: State) -> &'static str {
  match state {
    State::Available => "available",
    State::Missing => "missing",
    State::Unreadable => "unreadable",
  }
}

fn trailing(ctx: &mut Ctx, id: &str, trailing: Trailing) -> Option<Element> {
  match trailing {
    Trailing::None => None,
    Trailing::Remove(label, remove) => Some(
      IconButton::new(format!("{id}-remove"), Icon::X, label)
        .small()
        .on_click(move || remove())
        .render(ctx),
    ),
    Trailing::Lock => Some(
      icon(Icon::Lock, IconSize::Base, PaletteColor::TextMuted)
        .id(format!("{id}-lock"))
        .into(),
    ),
  }
}
