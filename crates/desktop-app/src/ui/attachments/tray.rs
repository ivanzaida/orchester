//! A composer's attachments: the chips of its files over the message, 6 apart and wrapping, with the loader while
//! files are read; the preview of the file whose chip was pressed (a text file's text, an image larger) over them;
//! and, under the box, why the files last given were not attached. The box itself is the drop target: files dragged
//! over it light its stroke in `border_focus`, 2 wide, and a drag of folders only is turned away.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    events::FileDragEvent,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  images::ImageSource,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::attachment::{MediaType, NewAttachment};

use super::{
  chip::AttachmentChip,
  draft::DraftAttachments,
  image_key,
  preview::{Preview, Shown, busy},
};
use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    icon::{Icon, icon},
    message::message_text,
  },
};

/// Puts pasted text back into the words of the message.
pub type PutBack = Arc<dyn Fn(String) + Send + Sync>;

/// `words` with `text` after them, a blank line apart when they hold some already: pasted text put back into the
/// message.
pub fn put_back_into(words: &mut String, text: &str) {
  if words.trim().is_empty() {
    text.clone_into(words);
  } else {
    let kept = words.trim_end().to_owned();
    *words = format!("{kept}\n\n{text}");
  }
}

/// The chips of the files `draft` holds, with the preview of the one opened over them; `None` while it holds none
/// and reads none. `id` names the composer; `put_back` puts a text file's text back into the message.
pub fn tray(ctx: &mut Ctx, id: &str, draft: &DraftAttachments, put_back: &PutBack) -> Option<Element> {
  let held = draft.attached.get().0;
  let reading = draft.reading.get() > 0;
  if held.is_empty() && !reading {
    return None;
  }
  let opened = draft.preview.get();
  let preview = opened
    .and_then(|open| held.iter().find(|file| file.id() == open))
    .map(|file| preview(ctx, id, (draft, file), put_back));
  let chips: Vec<Element> = held
    .iter()
    .map(|file| chip(ctx, id, draft, file).opened(opened == Some(file.id())).render(ctx))
    .collect();
  let busy = reading.then(|| busy(&format!("{id}-attachments-reading"), &ctx.t("attachments.adding")));
  let row = Row::new()
    .id(format!("{id}-attachments"))
    .describe("count", held.len().to_string())
    .width(Dimension::Pct(100.0))
    .wrap()
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .with_children(chips)
    .with_children(busy);
  Some(
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .with_children(preview)
      .child(row)
      .into(),
  )
}

/// The chip of `file`: its thumbnail for an image, a press that opens its preview for an image or text, and its
/// cross.
fn chip(ctx: &Ctx, id: &str, draft: &DraftAttachments, file: &NewAttachment) -> AttachmentChip {
  let name = file.name().as_str();
  let mut chip = AttachmentChip::new(
    format!("{id}-attachment-{}", file.id()),
    (name, file.media(), file.size()),
  );
  if file.media().kind() == orchester_core::attachment::AttachmentKind::Image {
    chip = chip.thumbnail(ImageSource::Bytes(file.content().clone()), image_key(file.id()));
  }
  if file.media() != MediaType::Pdf {
    let (preview, open) = (draft.preview.clone(), file.id());
    chip = chip.opens(
      ctx.t_args("attachments.preview", [("name", name.to_owned())]),
      move || preview.update(|shown| *shown = if *shown == Some(open) { None } else { Some(open) }),
    );
  }
  let (remover, gone) = (draft.clone(), file.id());
  chip.removable(
    ctx.t_args("attachments.remove", [("name", name.to_owned())]),
    move || remover.remove(gone),
  )
}

/// The preview of `file`: its text, with *Put back in the message*, or the image.
fn preview(ctx: &mut Ctx, id: &str, (draft, file): (&DraftAttachments, &NewAttachment), put_back: &PutBack) -> Element {
  let shown = match file.text() {
    Some(text) => Shown::Text(Arc::from(text)),
    None => Shown::Image(ImageSource::Bytes(file.content().clone()), image_key(file.id())),
  };
  let close = draft.preview.clone();
  let preview = Preview::new(
    format!("{id}-attachment-preview"),
    (file.name().as_str(), file.media(), file.size()),
    shown,
    move || close.set(None),
  );
  let preview = match file.text() {
    Some(text) => {
      let (text, put_back, draft, gone) = (text.to_owned(), put_back.clone(), draft.clone(), file.id());
      preview.puts_back(move || {
        put_back(text.clone());
        draft.remove(gone);
      })
    }
    None => preview,
  };
  preview.render(ctx)
}

/// Why the files last given were not attached, a line each with the alert icon, under the box; `None` when all were.
pub fn refusals(ctx: &Ctx, id: &str, draft: &DraftAttachments) -> Option<Element> {
  let refused = draft.refused.get().0;
  if refused.is_empty() {
    return None;
  }
  let lines = refused.iter().enumerate().map(|(at, why)| {
    Row::new()
      .id(format!("{id}-attachments-refused-{at}"))
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .child(icon(Icon::CircleAlert, IconSize::Sm, PaletteColor::TextMuted))
      .child(
        Text::new(&message_text(ctx, why))
          .variant(ExtraTypography::Note.role())
          .flex(1.0)
          .min_width(Dimension::Px(0.0)),
      )
      .into()
  });
  Some(
    Column::new()
      .id(format!("{id}-attachments-refused"))
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Tight.role())
      .with_children(lines.collect::<Vec<Element>>())
      .into(),
  )
}

/// `boxed`, a composer's box, as the drop target of `draft`'s files: lit while files are dragged over it, a drag
/// of folders only turned away, a drop attached.
pub fn drop_target(boxed: Column, draft: &DraftAttachments) -> Column {
  boxed
    .drag_over(|style| style.border(Border::inside(BorderSize::Md, PaletteColor::BorderFocus)))
    .on_file_drag_enter(|event: FileDragEvent| {
      if event.paths.files().next().is_none() {
        event.refuse();
      }
    })
    .on_file_drop(draft.on_drop())
}
