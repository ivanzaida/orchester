//! What approving a proposal replaces, in its review editor: *Approving replaces the active decision key* and what
//! that does, then the active record. The task's tab quotes the active text with who wrote it (`i09ZK` in
//! `t0jt2k`); the project's page shows the change as a diff (`P4cv22` in 05e) headed by who proposed it and what the
//! person edited: the active text removed, with its status, who wrote it and how many invocations were given it, and
//! the proposed text as the editor holds it now added.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::memory::Memory;
use orchester_workspace::memory::draft::DraftEdit;

use super::{RecordEditor, Replaced};
use crate::{
  screens::memory::words,
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography, HubSpacing, IconSize, MemoryText},
  ui::icon::{Icon, icon},
};

/// Which side of the diff a line is.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
  Removed,
  Added,
}

impl RecordEditor {
  /// The notice of `replaced`, which approving `proposal` replaces; as a diff on the project's `page`.
  pub(super) fn replaces(
    &self,
    ctx: &Ctx,
    (replaced, proposal): (&Replaced, Option<&Memory>),
    (id, page): (&str, bool),
  ) -> Element {
    let old = &replaced.memory;
    let kind = words::kind_lower(ctx, old.content().kind());
    let title = match old.content().key() {
      Some(key) => ctx.t_args("memory.editor.replacesKeyed", [("kind", &*kind), ("key", key.as_str())]),
      None => ctx.t_args("memory.editor.replaces", [("kind", &*kind)]),
    };
    if page {
      // The record's page: the group's head, the diff and what approving does (`IZHh4` in 05e).
      let group = format!("{id}-replaces");
      let note = crate::screens::hub::parts::note(
        &format!("{group}-note"),
        Icon::Info,
        &ctx.t("memory.editor.replacesTextDiff"),
      );
      return crate::screens::hub::parts::group(
        &group,
        crate::screens::hub::parts::head(&group, &title, None, None),
        Column::new()
          .width(Dimension::Pct(100.0))
          .spacing(SpacingSize::Sm)
          .child(self.diff(ctx, (replaced, proposal), id))
          .child(note),
      )
      .into();
    }
    let notice = Column::new()
      .id(format!("{id}-replaces"))
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
      .spacing(ExtraSpacing::Snug.role())
      .background(PaletteColor::SurfacePanel)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(ExtraRadius::Tile.role())
      .child(
        Row::new()
          .spacing(SpacingSize::Sm)
          .align_items(Alignment::Center)
          .child(icon(Icon::Replace, IconSize::Base, PaletteColor::TextSecondary))
          .child(Text::new(&title).variant(TypographyStyle::Label)),
      )
      .child(Text::new(&ctx.t("memory.editor.replacesText")).variant(MemoryText::NoticeText.role()));
    let quoted = words::join(ctx, &[old.content().text().to_owned(), replaced.meta.to_string()]);
    notice
      .child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .padding(
            Padding::new()
              .left(ExtraSpacing::Loose.role())
              .top(ExtraSpacing::Tight.role())
              .bottom(ExtraSpacing::Tight.role()),
          )
          .border_left(Border::inside(BorderSize::Md, PaletteColor::BorderStrong))
          .child(
            Text::new(&quoted)
              .flex(1.0)
              .variant(MemoryText::NoticeText.role())
              .color(PaletteColor::TextMuted),
          ),
      )
      .into()
  }

  /// The active text removed and the proposed text added, each with who wrote it (`d5dy4`).
  fn diff(&self, ctx: &Ctx, (replaced, proposal): (&Replaced, Option<&Memory>), id: &str) -> Column {
    let mut was = vec![ctx.t("memory.status.active").to_string(), replaced.meta.to_string()];
    match replaced.given {
      Some(1) => was.push(ctx.t("memory.delete.givenToOne").to_string()),
      Some(count) => was.push(
        ctx
          .t_args("memory.delete.givenTo", [("count", count.to_string())])
          .to_string(),
      ),
      None => {}
    }
    let mut proposed = Vec::new();
    if let Some(agent) = &replaced.proposer {
      proposed.push(
        ctx
          .t_args("memory.editor.diffProposedBy", [("agent", &**agent)])
          .to_string(),
      );
    }
    let edits = proposal.map(|memory| self.draft().edits_of(memory)).unwrap_or_default();
    if !edits.is_empty() {
      let named: Vec<String> = edits.iter().map(|edit| edit_words(ctx, *edit).to_string()).collect();
      let list = named.join(&ctx.t("memory.editor.editListSeparator"));
      proposed.push(ctx.t_args("memory.editor.diffEdited", [("edits", list)]).to_string());
    }
    // Who proposed it and what the person edited heads the box (`SGCy8` in `P4cv22`, 05e).
    let head = (!proposed.is_empty()).then(|| {
      Text::new(&words::join(ctx, &proposed))
        .id(format!("{id}-diff-head"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .width(Dimension::Pct(100.0))
    });
    Column::new()
      .id(format!("{id}-diff"))
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(HubSpacing::Row.role(), SpacingSize::Md))
      .spacing(SpacingSize::Xs)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Lg)
      .with_children(head)
      .child(diff_line(
        ctx,
        (&format!("{id}-removed"), Side::Removed),
        replaced.memory.content().text(),
        &words::join(ctx, &was),
      ))
      .child(diff_line(
        ctx,
        (&format!("{id}-added"), Side::Added),
        &self.text.get(),
        "",
      ))
  }
}

/// One side of the diff: its sign, the text and who wrote it (`F3K05U`, `nQVST`).
fn diff_line(ctx: &Ctx, (id, side): (&str, Side), text: &str, meta: &str) -> Row {
  let (background, sign, color, kind) = match side {
    Side::Removed => (
      ExtraColor::DiffRemoved,
      "memory.editor.removedSign",
      PaletteColor::Danger,
      "removed",
    ),
    Side::Added => (
      ExtraColor::DiffAdded,
      "memory.editor.addedSign",
      PaletteColor::Success,
      "added",
    ),
  };
  let mut body = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Tight.role())
    .child(
      Text::new(text)
        .id(format!("{id}-text"))
        .variant(ExtraTypography::Prose.role())
        .color(PaletteColor::TextPrimary),
    );
  if !meta.is_empty() {
    body = body.child(
      Text::new(meta)
        .id(format!("{id}-meta"))
        .variant(TypographyStyle::Caption),
    );
  }
  Row::new()
    .id(id.to_owned())
    .describe("kind", kind)
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(
      ExtraSpacing::Loose.role(),
      ExtraSpacing::Snug.role(),
    ))
    .spacing(SpacingSize::Sm)
    .rounded(RadiusSize::Sm)
    .background(background.role())
    .child(
      Text::new(&ctx.t(sign))
        .variant(TypographyStyle::Mono)
        .color(color)
        .nowrap(),
    )
    .child(body)
}

/// What the person changed, as the added line names it (*importance Normal → High*).
fn edit_words(ctx: &Ctx, edit: DraftEdit) -> Arc<str> {
  match edit {
    DraftEdit::Kind { from, to } => ctx.t_args(
      "memory.editor.editedKind",
      [
        ("from", words::kind_name(ctx, from).to_string()),
        ("to", words::kind_name(ctx, to).to_string()),
      ],
    ),
    DraftEdit::Key => ctx.t("memory.editor.editedKey"),
    DraftEdit::Importance { from, to } => ctx.t_args(
      "memory.editor.editedImportance",
      [
        ("from", words::importance(ctx, from).to_string()),
        ("to", words::importance(ctx, to).to_string()),
      ],
    ),
    DraftEdit::Text => ctx.t("memory.editor.editedText"),
    DraftEdit::Tags => ctx.t("memory.editor.editedTags"),
  }
}
