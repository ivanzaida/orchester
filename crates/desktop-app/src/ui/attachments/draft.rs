//! The files a composer holds before its message is sent, in the order the person attached them, and why the last
//! ones it was given were not taken. Files come from *Attach* (the system's picker), a drop on the composer and a
//! paste into its message: an image or files on the clipboard, or text longer than core's long-paste limit, which
//! becomes a text attachment instead of words of the message. Reading a file and turning a pasted image into a PNG
//! happen off the UI thread (`orchester_workspace::attachments`, which reads only what the person gave, refusing a
//! folder or a shortcut without following it), then the message's limits decide what is added.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::Ctx,
    events::{ClipboardImage, FileDropEvent, PasteEvent},
  },
  core::Signal,
};
use orchester_core::{
  attachment::{NewAttachment, limits::is_long_paste},
  id::AttachmentId,
};
use orchester_workspace::{
  attachments::{self, Refused},
  messages::Message,
};
use tokio::runtime::Handle;

use crate::services::{Services, attachment_picker::AttachmentPicker};

/// The files a draft holds, in the order they were attached.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Attached(pub Vec<NewAttachment>);

impl DevtoolsInspectable for Attached {}

/// Why the files last given were not taken, in the person's words.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Refusals(pub Vec<Message>);

impl DevtoolsInspectable for Refusals {}

/// What the person gave to attach.
enum Given {
  Files(Vec<PathBuf>),
  Image(ClipboardImage),
  Text(String),
}

/// The names a pasted image and pasted text are attached under, in the person's language.
#[derive(Clone)]
struct PastedNames {
  image: Arc<str>,
  text: Arc<str>,
}

/// A composer's files: what it holds, what it refused last, how many reads are under way, and which file's preview
/// is open.
#[derive(Clone)]
pub struct DraftAttachments {
  pub attached: Signal<Attached>,
  pub refused: Signal<Refusals>,
  /// Files and pastes being read, which the composer shows until they are added.
  pub reading: Signal<usize>,
  /// The file whose preview is open over the box.
  pub preview: Signal<Option<AttachmentId>>,
  runtime: Handle,
  names: PastedNames,
}

impl DraftAttachments {
  /// A composer's files, starting with `kept`, the ones its draft kept.
  pub fn new(ctx: &mut Ctx, kept: Vec<NewAttachment>) -> Self {
    let names = PastedNames {
      image: ctx.t("attachments.pastedImage"),
      text: ctx.t("attachments.pastedText"),
    };
    Self {
      attached: ctx.signal(Attached(kept)),
      refused: ctx.signal(Refusals::default()),
      reading: ctx.signal(0),
      preview: ctx.signal(None),
      runtime: Services::of(ctx).runtime,
      names,
    }
  }

  /// The files held now, outside a render.
  pub fn held(&self) -> Vec<NewAttachment> {
    self.attached.get_untracked().0
  }

  /// Holds `files` instead, as another draft's, and forgets what was refused.
  pub fn replace(&self, files: Vec<NewAttachment>) {
    if self.attached.get_untracked().0 != files {
      self.attached.set(Attached(files));
    }
    self.refused.set(Refusals::default());
    self.preview.set(None);
  }

  /// Drops the file `id`.
  pub fn remove(&self, id: AttachmentId) {
    self.attached.update(|held| held.0.retain(|file| file.id() != id));
    if self.preview.get_untracked() == Some(id) {
      self.preview.set(None);
    }
  }

  /// The files of a message just sent are no longer the draft's: `sent` goes, what was added since stays.
  pub fn sent(&self, sent: &[NewAttachment]) {
    self.attached.update(|held| held.0.retain(|file| !sent.contains(file)));
  }

  /// Reads the files at `paths`, which the person picked or dropped, and adds those the message takes.
  pub fn add_files(&self, paths: Vec<PathBuf>) {
    if !paths.is_empty() {
      self.add(Given::Files(paths));
    }
  }

  /// What a paste into the message does: files or an image on the clipboard are attached instead of their text, and
  /// text past the long-paste limit becomes a text attachment; shorter text is pasted as words.
  pub fn on_paste(&self) -> impl Fn(PasteEvent) + Send + Sync + 'static {
    let draft = self.clone();
    move |event: PasteEvent| {
      let given = if !event.files().is_empty() {
        Given::Files(event.files().to_vec())
      } else if let Some(image) = event.image() {
        Given::Image(image.clone())
      } else if let Some(text) = event.text().filter(|text| is_long_paste(text)) {
        Given::Text(text.to_owned())
      } else {
        return;
      };
      event.prevent_default();
      draft.add(given);
    }
  }

  /// What a drop of files on the composer does: they are read and attached; a folder among them is refused.
  pub fn on_drop(&self) -> impl Fn(FileDropEvent) + Send + Sync + 'static {
    let draft = self.clone();
    move |event: FileDropEvent| {
      event.prevent_default();
      draft.add_files(event.paths.paths().map(PathBuf::from).collect());
    }
  }

  /// What *Attach* does: the picker opens, and the files chosen in it are attached.
  pub fn on_attach(&self, picker: AttachmentPicker, title: Arc<str>) -> impl Fn() + Send + Sync + 'static {
    let draft = self.clone();
    move || {
      let (draft, pick) = (draft.clone(), picker.pick(title.clone()));
      draft.runtime.clone().spawn(async move { draft.add_files(pick.await) });
    }
  }

  /// Reads `given` off the UI thread, then adds what the message's limits allow and says why the rest was refused.
  fn add(&self, given: Given) {
    self.reading.update(|reading| *reading += 1);
    let draft = self.clone();
    self.runtime.spawn_blocking(move || {
      let (read, mut refused) = draft.read(given);
      draft.attached.update(|held| refused.extend(attachments::add(&mut held.0, read)));
      draft
        .refused
        .set(Refusals(refused.iter().map(Refused::message).collect()));
      draft.reading.update(|reading| *reading = reading.saturating_sub(1));
    });
  }

  /// The files of `given`, read and checked, and the ones refused with why. Blocks on the file system.
  fn read(&self, given: Given) -> (Vec<NewAttachment>, Vec<Refused>) {
    let pasted = |name: &Arc<str>, content: Result<Vec<u8>, ()>| {
      let attached = content
        .map_err(|()| Refused::new(name.as_ref(), orchester_core::attachment::AttachmentRefusal::Unreadable))
        .and_then(|content| attachments::pasted(name, content));
      match attached {
        Ok(attachment) => (vec![attachment], Vec::new()),
        Err(refused) => (Vec::new(), vec![refused]),
      }
    };
    match given {
      Given::Files(paths) => attachments::read_files(&paths),
      Given::Image(image) => {
        let png = image.to_png().map_err(|error| {
          tracing::warn!(%error, "a pasted image could not be encoded as PNG");
        });
        pasted(&self.names.image, png)
      }
      Given::Text(text) => pasted(&self.names.text, Ok(text.into_bytes())),
    }
  }
}
