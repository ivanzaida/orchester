//! The files attached to a message in a conversation, a person's in a task's chat or in the Orchestrator chat, under
//! its words: a chip each, an image with its thumbnail, 6 apart and wrapping. A press on an image or a text file opens
//! its preview under the chips (the image larger, the text in the code box), with *Open*, which hands the project's
//! copy to its default program; a press on a PDF opens it there at once. What cannot be opened says so under them.

use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  core::Signal,
  images::ImageSource,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  attachment::{Attachment, AttachmentKind},
  id::AttachmentId,
};
use orchester_store::attachment_file;
use orchester_workspace::{attachments::copy_unreadable, background};

use super::{
  chip::AttachmentChip,
  image_key,
  preview::{Preview, Shown},
};
use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::icon::{Icon, icon},
};

/// The files of message `id`, kept in the project at `root`; `None` when it has none.
pub fn sent(ctx: &mut Ctx, id: &str, root: &Path, attachments: &[Attachment]) -> Option<Element> {
  if attachments.is_empty() {
    return None;
  }
  Some(ctx.mount_keyed::<SentAttachments>(
    &format!("sent-attachments-{id}"),
    Props {
      id: Arc::from(id),
      root: root.to_path_buf(),
      attachments: Arc::from(attachments),
    },
  ))
}

#[derive(Clone, PartialEq)]
struct Props {
  id: Arc<str>,
  root: PathBuf,
  attachments: Arc<[Attachment]>,
}

impl DevtoolsInspectable for Props {}

/// A text file's text, read for its preview.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Read(Option<Arc<str>>);

impl DevtoolsInspectable for Read {}

/// The name of a file that could not be opened or read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct NotOpened(Option<Arc<str>>);

impl DevtoolsInspectable for NotOpened {}

struct SentAttachments {
  open: Signal<Option<AttachmentId>>,
  failed: Signal<NotOpened>,
}

impl Component for SentAttachments {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(None),
      failed: ctx.signal(NotOpened::default()),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let opened = self.open.get().and_then(|open| {
      props
        .attachments
        .iter()
        .find(|attachment| attachment.id() == open)
        .cloned()
    });
    let text = self.text_of(ctx, &props, opened.as_ref());
    let chips: Vec<Element> = props
      .attachments
      .iter()
      .map(|attachment| self.chip(ctx, &props, attachment))
      .collect();
    let preview = opened.map(|attachment| self.preview(ctx, &props, &attachment, text));
    let failed = self.failed.get().0.map(|name| {
      Row::new()
        .id(format!("{}-attachments-failed", props.id))
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(icon(Icon::CircleAlert, IconSize::Sm, PaletteColor::TextMuted))
        .child(
          Text::new(&ctx.t_args("attachments.openFailed", [("name", name.to_string())]))
            .variant(ExtraTypography::Note.role()),
        )
    });
    Column::new()
      .id(format!("{}-attachments", props.id))
      .describe("count", props.attachments.len().to_string())
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .wrap()
          .spacing(ExtraSpacing::Snug.role())
          .with_children(chips),
      )
      .with_children(preview)
      .with_children(failed)
  }
}

impl SentAttachments {
  /// The text of the text file `opened`, read off the UI thread: `None` while it is read and for another kind,
  /// `Some(Err)` when its copy cannot be read.
  fn text_of(&self, ctx: &mut Ctx, props: &Props, opened: Option<&Attachment>) -> Option<Result<Arc<str>, ()>> {
    let reading = opened
      .filter(|attachment| attachment.media().kind() == AttachmentKind::Text)
      .map(|attachment| {
        (
          attachment_file(&props.root, attachment),
          attachment.name().as_str().to_owned(),
        )
      });
    let read = ctx.future(reading, |reading| async move {
      let Some((path, name)) = reading else {
        return Ok(Read(None));
      };
      background::run(move || std::fs::read_to_string(&path))
        .await?
        .map(|text| Read(Some(Arc::from(text))))
        .map_err(|error| {
          tracing::warn!(%error, "an attached file's copy could not be read");
          copy_unreadable(name)
        })
    });
    let state = read.state().get();
    if state.error.is_some() && !state.is_pending() {
      return Some(Err(()));
    }
    state.data.and_then(|read| read.0).map(Ok)
  }

  /// The chip of `attachment`: a press toggles the preview of an image or a text file, and opens a PDF.
  fn chip(&self, ctx: &Ctx, props: &Props, attachment: &Attachment) -> Element {
    let name = attachment.name().as_str();
    let path = attachment_file(&props.root, attachment);
    let mut chip = AttachmentChip::new(
      format!("{}-attachment-{}", props.id, attachment.id()),
      (name, attachment.media(), attachment.size()),
    )
    .opened(self.open.get() == Some(attachment.id()));
    if attachment.media().kind() == AttachmentKind::Image {
      chip = chip.thumbnail(ImageSource::File(path.clone()), image_key(attachment.id()));
    }
    let chip = if attachment.media().kind() == AttachmentKind::Pdf {
      let (failed, name) = (self.failed.clone(), Arc::<str>::from(name));
      chip.opens(
        ctx.t_args("attachments.open", [("name", name.to_string())]),
        move || {
          open_file(path.clone(), name.clone(), failed.clone());
        },
      )
    } else {
      let (open, id) = (self.open.clone(), attachment.id());
      chip.opens(
        ctx.t_args("attachments.preview", [("name", name.to_owned())]),
        move || {
          open.update(|shown| *shown = if *shown == Some(id) { None } else { Some(id) });
        },
      )
    };
    chip.render(ctx)
  }

  /// The preview of `attachment`: an image larger, or the text once read, or why it cannot be read; *Open* hands the
  /// copy to its default program.
  fn preview(
    &self,
    ctx: &mut Ctx,
    props: &Props,
    attachment: &Attachment,
    text: Option<Result<Arc<str>, ()>>,
  ) -> Element {
    let path = attachment_file(&props.root, attachment);
    let name: Arc<str> = Arc::from(attachment.name().as_str());
    let shown = match (attachment.media().kind(), text) {
      (AttachmentKind::Text, Some(Ok(text))) => Shown::Text(text),
      (AttachmentKind::Text, Some(Err(()))) => {
        Shown::Not(ctx.t_args("attachments.unreadable", [("name", name.to_string())]))
      }
      (AttachmentKind::Text, None) => Shown::Reading,
      _ => Shown::Image(ImageSource::File(path.clone()), image_key(attachment.id())),
    };
    let close = self.open.clone();
    let failed = self.failed.clone();
    Preview::new(
      format!("{}-attachment-preview", props.id),
      (&name, attachment.media(), attachment.size()),
      shown,
      move || close.set(None),
    )
    .opens(move || open_file(path.clone(), name.clone(), failed.clone()))
    .render(ctx)
  }
}

/// Hands the project's copy at `path` to its default program off the UI thread; one that does not open is named in
/// `failed`.
fn open_file(path: PathBuf, name: Arc<str>, failed: Signal<NotOpened>) {
  failed.set(NotOpened(None));
  std::thread::spawn(move || {
    if let Err(error) = orchester_exec::external::open_path(&path) {
      tracing::warn!(%error, "an attached file could not be opened");
      failed.set(NotOpened(Some(name)));
    }
  });
}
