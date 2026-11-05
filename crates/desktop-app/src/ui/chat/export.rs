//! *Export conversation…* of both chats (ORC-327, 07d `alIPD` and 09d `A2YMy`, dialogs `hysCz` and `rbdgW`;
//! `docs/design/copy-and-export.md`): the dialog of Storage's Clean up, 520 wide, its tile `download` in the neutral
//! tone beside *Export conversation* and what it covers. Under *Include* two radio rows in one `border` outline, radius
//! 6, the chosen one on `hover`: *Messages only*, the default, and *Messages and tool calls*, each with what it adds
//! 24 in under its name. Under *Saves as* the file's name in Geist Mono on `surface_input` and *Markdown · ≈ size*.
//! Its footer says *Enter saves* and *Esc cancels* before *Cancel*, *Copy Markdown* and the primary *Save…*, which
//! opens the system's save dialog on that name (`services::save_picker`). The choice of *Include* is kept for the
//! next export while the app runs.
//!
//! The Markdown is `orchester_workspace::export`'s, built off the UI thread with the values of the project's secrets
//! masked; a save that failed says why under the dialog's parts and the dialog stays, one that wrote the file closes
//! it. *Copy conversation as Markdown* in the chats' menus copies the messages only without the dialog, which opens to
//! say why when it could not.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    ctx::{Ctx, FutureAction},
    i18n::I18n,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::memory::SecretScreen;
use orchester_workspace::{
  background,
  export::{self, Include, Words},
  i18n::NAMESPACE,
  messages::Message,
  secrets::Vault,
};
use parking_lot::Mutex;

use crate::{
  services::{
    Services,
    save_picker::{SaveAsk, SavePicker},
  },
  theme::{DialogText, ExtraColor, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    clipboard::Clipboard,
    dialog::{Dialog, Tone},
    field::field_error,
    icon::{Icon, icon},
    message::message_text,
    pressable::{Focus, pressable},
  },
};

/// The dialog's width (`hysCz`).
const WIDTH: f32 = 520.0;

/// The choice of *Include* the last export made, for the next one.
static REMEMBERED: Mutex<Include> = Mutex::new(Include::Messages);

/// The Markdown of a conversation with what it includes, its words and the screen that masks the project's secrets.
pub type Build = Arc<dyn Fn(Include, Words<'_>, &SecretScreen) -> String + Send + Sync>;

/// What an export is of: the project it masks the secrets of, the file's name, what each choice includes and the
/// Markdown.
#[derive(Clone)]
pub struct Source {
  pub root: PathBuf,
  pub file_name: String,
  /// What *Messages only* and *Messages and tool calls* take, under their names.
  pub hints: (Arc<str>, Arc<str>),
  pub build: Build,
}

/// What one press asked for: the file to save at, once the person chose it, or the clipboard.
struct Job {
  source: Source,
  include: Include,
  save: Option<SaveAsk>,
}

/// How a press went.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Done {
  Saved,
  /// The save dialog was closed without a choice.
  Cancelled,
  /// The Markdown to put on the clipboard.
  Copy(Arc<str>),
}

impl lurq::app::component::DevtoolsInspectable for Done {}

type ExportAction = FutureAction<Job, Done, Message>;

/// An export's state on a screen: the dialog is open, what it includes, the last press and how it went.
#[derive(Clone)]
pub struct Exporting {
  pub open: Signal<bool>,
  include: Signal<Include>,
  action: ExportAction,
  /// The last copy put the Markdown on the clipboard, or did not.
  copied: Signal<Option<bool>>,
  /// Why the last press wrote or copied nothing.
  failure: Signal<Option<Message>>,
}

impl Exporting {
  /// Made in the screen's `create`: lurq keeps a component's actions by the order it asks for them.
  pub fn new(ctx: &mut Ctx) -> Self {
    let services = Services::of(ctx);
    let (vault, picker, i18n) = (services.vault, SavePicker::of(ctx), ctx.i18n().clone());
    let action = ctx.future_action(move |job: Job| run(job, (vault.clone(), picker.clone(), i18n.clone())));
    let exporting = Self {
      open: ctx.signal(false),
      include: ctx.signal(*REMEMBERED.lock()),
      action,
      copied: ctx.signal(None),
      failure: ctx.signal(None),
    };
    let (done, clipboard) = (exporting.clone(), Clipboard::of(ctx));
    ctx.watch(&exporting.action.state(), move |state| {
      if let Some(why) = state.error.clone().filter(|_| !state.is_pending()) {
        done.failed(why);
        return;
      }
      if !state.is_fulfilled() {
        return;
      }
      match &state.data {
        Some(Done::Saved) => done.open.set(false),
        Some(Done::Copy(markdown)) => {
          let copied = clipboard.copy(markdown);
          if !copied {
            tracing::warn!("the conversation's Markdown could not be put on the clipboard");
          }
          done.copied.set(Some(copied));
          // The menu's copy that failed opens the dialog, whose Copy Markdown says so.
          if !copied && !done.open.get_untracked() {
            done.open.set(true);
          }
        }
        Some(Done::Cancelled) | None => {}
      }
    });
    exporting
  }

  /// Why the last press did nothing, said in the dialog, which opens for it when the menu's copy failed.
  fn failed(&self, why: Message) {
    self.failure.set(Some(why));
    if !self.open.get_untracked() {
      self.open.set(true);
    }
  }

  /// A new press: what the last one said goes.
  fn pressed(&self) {
    self.copied.set(None);
    self.failure.set(None);
  }

  /// Opens the dialog afresh.
  pub fn show(&self) {
    self.pressed();
    self.open.set(true);
  }

  /// *Copy conversation as Markdown*: the messages only, on the clipboard.
  pub fn copy_messages(&self, source: Source) {
    self.start(source, Include::Messages, None);
  }

  /// The dialog over the window while it is open: `id` names it, `what` says what it covers.
  pub fn dialog(&self, ctx: &mut Ctx, id: &str, (source, what): (&Source, Arc<str>)) -> Option<Element> {
    if !self.open.get() {
      return None;
    }
    let include = self.include.get();
    let pending = self.action.state().get().is_pending();
    let save = {
      let (exporting, source) = (self.clone(), source.clone());
      let ask = (ctx.t("ui.chat.export.pickerTitle"), ctx.t("ui.chat.export.fileKind"));
      move || {
        let ask = SaveAsk {
          title: ask.0.clone(),
          file_name: source.file_name.clone(),
          kind: ask.1.clone(),
        };
        exporting.start(source.clone(), exporting.include.get_untracked(), Some(ask));
      }
    };
    let copy = {
      let (exporting, source) = (self.clone(), source.clone());
      move || exporting.start(source.clone(), exporting.include.get_untracked(), None)
    };
    let save_button =
      ActionButton::new(format!("{id}-save"), ButtonKind::Primary, ctx.t("ui.chat.export.save")).icon(Icon::Download);
    let save_button = if pending {
      save_button.loading()
    } else {
      save_button.on_click(save.clone())
    };
    let (copy_icon, copy_label) = match self.copied.get() {
      Some(true) => (Icon::Check, ctx.t("ui.copied")),
      Some(false) => (Icon::TriangleAlert, ctx.t("ui.copyFailed")),
      None => (Icon::Copy, ctx.t("ui.chat.export.copyMarkdown")),
    };
    let copy_button = ActionButton::new(format!("{id}-copy"), ButtonKind::Secondary, copy_label).icon(copy_icon);
    let copy_button = if pending {
      copy_button.disabled()
    } else {
      copy_button.on_click(copy)
    };
    let failure = self
      .failure
      .get()
      .map(|why| Element::from(field_error(&format!("{id}-error"), &message_text(ctx, &why))));
    let parts = (
      self.choices(ctx, id, (include, &source.hints)),
      file(ctx, id, (source, include)),
    );
    let dialog = Dialog::new(
      id,
      Icon::Download,
      ctx.t("ui.chat.export.title"),
      ctx.t("ui.chat.export.cancel"),
    )
    .tone(Tone::Neutral)
    .width(WIDTH)
    .what(what)
    .part(parts.0)
    .part(parts.1)
    .on_enter(ctx.t("ui.chat.export.enterSaves"), save)
    .action(copy_button)
    .action(save_button)
    .under(failure);
    Some(dialog.render(ctx, &self.open))
  }

  /// *Include* and its two rows.
  fn choices(&self, ctx: &Ctx, id: &str, (include, hints): (Include, &(Arc<str>, Arc<str>))) -> Column {
    let row = |choice: Include, (name, hint): (Arc<str>, &Arc<str>), first: bool| {
      let chosen = include == choice;
      let pick = self.include.clone();
      let row = Focus::Keyboard
        .apply(pressable(move || {
          *REMEMBERED.lock() = choice;
          pick.set(choice);
        }))
        .id(format!("{id}-{}", choice_name(choice)))
        .describe("a11y_name", name.clone())
        .describe("checked", if chosen { "true" } else { "false" })
        .width(Dimension::Pct(100.0))
        .child(option((chosen, &name, hint)));
      let row = if chosen {
        row.background(ExtraColor::Hover.role())
      } else {
        row
      };
      let row = row.hovered_style(Style::new().background(ExtraColor::Hover.role()));
      if first {
        row
      } else {
        row.border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
      }
    };
    let rows = Column::new()
      .id(format!("{id}-include"))
      .width(Dimension::Pct(100.0))
      .rounded(RadiusSize::Md)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .clip()
      .child(row(
        Include::Messages,
        (ctx.t("ui.chat.export.messagesOnly"), &hints.0),
        true,
      ))
      .child(row(
        Include::WithTools,
        (ctx.t("ui.chat.export.withTools"), &hints.1),
        false,
      ));
    group(&format!("{id}-include-group"), &ctx.t("ui.chat.export.include"), rows)
  }

  /// Saves at the file the person chooses (`save`), or copies, the Markdown with what `include` says; nothing while a
  /// press is under way.
  fn start(&self, source: Source, include: Include, save: Option<SaveAsk>) {
    if self.action.state().get_untracked().is_pending() {
      return;
    }
    self.pressed();
    self.action.run(Job { source, include, save });
  }
}

/// The work of one press: the file the person chooses first when it saves, then the Markdown with the project's
/// secrets masked, written there or handed back for the clipboard.
async fn run(job: Job, (vault, picker, i18n): (Vault, SavePicker, I18n)) -> Result<Done, Message> {
  let path = match job.save.clone() {
    Some(ask) => match picker.pick(ask).await {
      Some(path) => Some(path),
      None => return Ok(Done::Cancelled),
    },
    None => None,
  };
  background::run(move || {
    let screen = export::screen_in(&job.source.root, vault.get())?;
    let words = |message: &Message| {
      let args = message.args.iter().map(|(name, value)| (*name, value.as_str()));
      i18n.t_ns_args(NAMESPACE, message.key, args).to_string()
    };
    let markdown = (job.source.build)(job.include, &words, &screen);
    match path {
      Some(path) => export::write_to(&path, &markdown).map(|()| Done::Saved),
      None => Ok(Done::Copy(markdown.into())),
    }
  })
  .await?
}

const fn choice_name(choice: Include) -> &'static str {
  match choice {
    Include::Messages => "messages-only",
    Include::WithTools => "with-tools",
  }
}

/// An option's radio and name, and what it takes 24 in under them (`Option · Messages only` `A1B4E`).
fn option((chosen, name, hint): (bool, &str, &str)) -> Column {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, SpacingSize::Sm))
    .spacing(SpacingSize::Xs)
    .child(
      Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(crate::ui::toggle::radio_mark(chosen))
        .child(Text::new(name).variant(DialogText::Option.role()).nowrap()),
    )
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .padding(Padding::new().left(SpacingSize::Xl))
        .child(
          Text::new(hint)
            .variant(DialogText::Detail.role())
            .width(Dimension::Pct(100.0)),
        ),
    )
}

/// *Saves as* and the file: its name in Geist Mono and *Markdown · ≈ size* (`File` `f3F3kh`). The size is of the
/// Markdown as it is built now, before the secrets are masked.
fn file(ctx: &mut Ctx, id: &str, (source, include): (&Source, Include)) -> Column {
  let words = |message: &Message| message_text(ctx, message).to_string();
  let bytes = (source.build)(include, &words, &SecretScreen::new(Vec::new())).len();
  let size = message_text(ctx, &export::size(bytes));
  let format = ctx.t_args("ui.chat.export.format", [("size", &*size)]);
  let line = Row::new()
    .id(format!("{id}-file"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, SpacingSize::Sm))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Md)
    .background(PaletteColor::SurfaceInput)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(icon(Icon::FileText, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(&source.file_name)
        .id(format!("{id}-file-name"))
        .variant(ExtraTypography::PropertyMono.role())
        .nowrap()
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(
      Text::new(&format)
        .id(format!("{id}-file-format"))
        .variant(DialogText::Detail.role())
        .nowrap(),
    );
  group(&format!("{id}-saves-as"), &ctx.t("ui.chat.export.savesAs"), line)
}

/// A part of the dialog under its caption, 6 apart (`Include` `bZ5qS`, `Saves as` `f7ItA`).
fn group(id: &str, caption: &str, content: impl Into<Element>) -> Column {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .child(Text::new(caption).variant(DialogText::Detail.role()))
    .child(content)
}
