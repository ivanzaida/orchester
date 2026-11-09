//! A conversation's row in History and what a person does with it (ORC-328, ORC-329; `08 · History · row menu`
//! `q30JYF`, `08b · rename in place` `sLeGr`, `08c · Pinned group · right-click` `r4DVU`, `08d · Archived` `qAfeu`). A
//! press opens the conversation. Under the pointer the row ends in a `⋯` button (`More`, a small icon button, 6 from
//! the row's end) that opens its menu (`Row menu` `Y5qr8`, 180 wide, under the button at its end), which a right-click
//! opens at the pointer too: *Rename*, *Pin* or *Unpin*, *Archive*, and after a line *Delete…* in `danger`, which asks
//! in a modal first ([`super::delete`]); the row whose menu is open stays on `hover`. An archived row says when it was
//! archived, and under the pointer offers *Unarchive* (`Unarchive` `N8Rj0F`, a compact secondary button) before its
//! `⋯`, whose menu holds *Rename*, *Unarchive* and *Delete…*. *Rename*, F2 over the row or a double-click turn the
//! title into a field (`Title field` `Ztw6Q`: the title in 12.5/500, 3 over it, 2 at its sides) over *Enter saves*
//! and *Esc cancels* in key caps and 11 `text_muted` (`Keys` `K2NgV`, 5 under them). Enter keeps the person's title,
//! which wins over the automatic one from then on; a blank one is refused and the field says why. Esc, which the
//! screen hears first, cancels.

use std::{path::Path, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    events::{KeyboardEvent, MouseButton, mouse_event::MouseEvent},
    theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Placement, Popup, Row, Text, TextOverflow},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, Style, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::ChatId, time::Timestamp};
use orchester_store::ChatListing;
use orchester_workspace::{
  background,
  messages::Message,
  orchestrator::{
    chat::{archive_in, pin_in, rename_in},
    watching::LeadCounts,
  },
};

use super::{
  Deleting, FOUND_ROW_HEIGHT, Item, OpenedAt, ROW_HEIGHT, Renaming, Rows,
  meta::{self, when},
  search::{Matched, marks},
};
use crate::{
  navigation::{Navigation, link::Open},
  routes::{OrchestratorChat, ProjectPage, Route},
  services::Services,
  theme::{ExtraColor, ExtraSpacing, OrchestratorText},
  ui::{
    button::{ActionButton, ButtonKind},
    field::FieldInput,
    icon::Icon,
    icon_button::IconButton,
    kbd::key_cap,
    menu::{MenuItem, menu_at_end, menu_panel, separator},
    message::message_text,
    pressable::opening,
  },
};

/// The row menu's width (`Row menu` `Y5qr8`).
const MENU_WIDTH: f32 = 180.0;

/// What Enter in the title field keeps: the person's `title` of `chat`.
#[derive(Clone)]
struct Renamed {
  rows: Rows,
  chat: ChatId,
  title: String,
}

type RenameAction = FutureAction<Renamed, (), Message>;

/// A change of a conversation's place in History that its menu or its *Unarchive* makes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Change {
  Pin,
  Unpin,
  Archive,
  Unarchive,
}

impl Change {
  /// Makes the change to `chat` of the project at `root` now. Blocks on SQLite.
  fn make(self, root: &Path, chat: ChatId) -> Result<(), Message> {
    let now = Timestamp::now();
    match self {
      Self::Pin => pin_in(root, chat, Some(now)),
      Self::Unpin => pin_in(root, chat, None),
      Self::Archive => archive_in(root, chat, Some(now)),
      Self::Unarchive => archive_in(root, chat, None),
    }
  }

  /// What the row says when the change did not happen, with its reason.
  const fn refused_key(self) -> &'static str {
    match self {
      Self::Pin => "orchestrator.historyPanel.notPinned",
      Self::Unpin => "orchestrator.historyPanel.notUnpinned",
      Self::Archive => "orchestrator.historyPanel.notArchived",
      Self::Unarchive => "orchestrator.historyPanel.notUnarchived",
    }
  }
}

/// A change that did not happen, and why.
#[derive(Clone, Debug, PartialEq)]
struct Unchanged {
  change: Change,
  reason: Message,
}

impl DevtoolsInspectable for Unchanged {}

/// Makes a change of a conversation of the project at the path.
type ChangeAction = FutureAction<(Arc<Path>, ChatId, Change), (), Unchanged>;

/// A row of the list: a group's heading, or a conversation with its menu and its title field.
pub(super) struct ItemView {
  /// The pointer is over the row: it shows its `⋯` button, and F2 renames it.
  hovered: Signal<bool>,
  menu: Signal<bool>,
  /// Where a right-click opened the menu, in the window; `None` while the `⋯` button opened it.
  pointer: Signal<Option<(f32, f32)>>,
  row: ElementRef,
  button: ElementRef,
  /// The title typed in the field, seeded with the conversation's for each edit.
  title: Signal<String>,
  seeded: Signal<bool>,
  /// Set to put the focus in the title field.
  focus: Signal<bool>,
  input: ElementRef,
  rename: RenameAction,
  /// Why the last Enter kept no title, until the next edit.
  refused: Signal<Option<Message>>,
  change: ChangeAction,
  /// Why the last *Pin*, *Unpin*, *Archive* or *Unarchive* did not happen.
  unchanged: Signal<Option<Unchanged>>,
}

impl Component for ItemView {
  type Props = Item;

  fn create(ctx: &mut Ctx) -> Self {
    let runners = Services::of(ctx).runners;
    let changed = runners.clone();
    let rename: RenameAction = ctx.future_action(move |renamed: Renamed| {
      let runners = changed.clone();
      async move {
        let Renamed { rows, chat, title } = renamed;
        let root = rows.root.clone();
        background::run(move || rename_in(&root, chat, &title))
          .await
          .and_then(|saved| saved)?;
        rows.renaming.update(|editing| {
          if editing.0 == Some(chat) {
            *editing = Renaming(None);
          }
        });
        runners.chat_changed(&rows.root);
        Ok(())
      }
    });
    let change: ChangeAction = ctx.future_action(move |(root, chat, change): (Arc<Path>, ChatId, Change)| {
      let runners = runners.clone();
      async move {
        let kept = root.clone();
        background::run(move || change.make(&kept, chat))
          .await
          .and_then(|made| made)
          .map_err(|reason| Unchanged { change, reason })?;
        runners.chat_changed(&root);
        Ok(())
      }
    });
    let (refused, unchanged) = (ctx.signal(None), ctx.signal(None));
    let said = refused.clone();
    ctx.watch(&rename.state(), move |state| said.set(state.error.clone()));
    let said = unchanged.clone();
    ctx.watch(&change.state(), move |state| said.set(state.error.clone()));
    Self {
      hovered: ctx.signal(false),
      menu: ctx.signal(false),
      pointer: ctx.signal(None),
      row: ElementRef::new(),
      button: ElementRef::new(),
      title: ctx.signal(String::new()),
      seeded: ctx.signal(false),
      focus: ctx.signal(false),
      input: ElementRef::new(),
      rename,
      refused,
      change,
      unchanged,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    match ctx.props::<Item>().clone() {
      Item::Heading(heading) => super::heading(ctx, heading),
      Item::Chat {
        listing,
        current,
        today,
        matched,
        leads,
        rows,
      } => {
        if rows.renaming.get().0 == Some(listing.chat.id()) {
          self.field(ctx, &listing, &rows)
        } else {
          if self.seeded.get_untracked() {
            self.seeded.set(false);
          }
          self.chat(ctx, (&listing, matched.as_ref(), &leads), (current, today), &rows)
        }
      }
    }
  }
}

impl ItemView {
  /// Turns the title of `listing` into the field.
  fn start_rename(&self, listing: &ChatListing, rows: &Rows) -> Arc<dyn Fn() + Send + Sync> {
    let (renaming, title, seeded, focus) = (
      rows.renaming.clone(),
      self.title.clone(),
      self.seeded.clone(),
      self.focus.clone(),
    );
    let (refused, words, chat) = (self.refused.clone(), listing.chat.title().to_owned(), listing.chat.id());
    Arc::new(move || {
      title.set(words.clone());
      seeded.set(true);
      refused.set(None);
      focus.set(true);
      renaming.set(Renaming(Some(chat)));
    })
  }

  /// The conversation's row, saying what it `leads`: a press opens it, at the first match while the search `matched`
  /// its messages; a right-click or its `⋯` button opens its menu.
  fn chat(
    &self,
    ctx: &mut Ctx,
    (listing, matched, leads): (&ChatListing, Option<&Matched>, &LeadCounts),
    (current, today): (bool, bool),
    rows: &Rows,
  ) -> Element {
    let id = listing.chat.id();
    let open = self.menu.get();
    let more = self.hovered.get() || open;
    let rename = self.start_rename(listing, rows);
    let navigation = Navigation::of(ctx);
    let route = Route::Project(rows.project, ProjectPage::Orchestrator(OrchestratorChat::Chat(id)));
    let (enter, leave, over) = (self.hovered.clone(), self.hovered.clone(), self.hovered.clone());
    let (twice, by_key) = (rename.clone(), rename.clone());
    let (pointer, menu) = (self.pointer.clone(), self.menu.clone());
    let (to, opened_at) = (navigation.to(route), rows.opened_at.clone());
    let at = matched
      .and_then(Matched::opens_at)
      .map(|seq| OpenedAt { chat: id, seq });
    let press: Open = Arc::new(move |place| {
      opened_at.set(at);
      to(place);
    });
    let height = if matched.is_some_and(|matched| matched.messages.is_some()) {
      FOUND_ROW_HEIGHT
    } else {
      ROW_HEIGHT
    };
    let row = opening(press)
      .id(format!("orchestrator-history-{id}"))
      .describe("current", current.to_string())
      .describe("pinned", listing.pinned.is_some().to_string())
      .ref_element(self.row.clone())
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(height))
      .padding(
        Padding::symmetric(ExtraSpacing::Loose.role(), ExtraSpacing::TextArea.role()).right(if more {
          ExtraSpacing::Snug.role()
        } else {
          ExtraSpacing::Loose.role()
        }),
      )
      .spacing(SpacingSize::Xs)
      .align_items(Alignment::Center)
      .rounded(RadiusSize::Md)
      .hovered_style(Style::new().background(ExtraColor::Hover.role()))
      .on_mouse_enter(move || enter.set(true))
      .on_mouse_leave(move || leave.set(false))
      .on_dblclick(move |_: MouseEvent| twice())
      .on_mouse_click(MouseButton::Right, move |event: MouseEvent| {
        pointer.set(Some((event.x, event.y)));
        menu.set(true);
      })
      .on_key_down(move |event: KeyboardEvent| {
        if event.key == "F2" && !event.text_input_focused && over.get_untracked() {
          event.prevent_default();
          by_key();
        }
      });
    let row = if current || open {
      row.background(ExtraColor::Hover.role())
    } else {
      row
    };
    let row = row
      .child(self.words(ctx, (listing, matched, leads), (current, today)))
      .with_children(more.then(|| self.unarchive(ctx, listing, rows)).flatten())
      .with_children(more.then(|| self.more(ctx, listing)));
    Column::new()
      .width(Dimension::Pct(100.0))
      .child(row)
      .with_children(open.then(|| self.menu(ctx, listing, rows, &rename)))
      .into()
  }

  /// The title over when it last changed, or was archived, and how many messages it holds, or after the eye what it
  /// `leads` and when it last changed, or why a change of its place did not happen; while the search `matched` it, the
  /// places in the title marked, the line of its messages under it and where the matches are.
  fn words(
    &self,
    ctx: &Ctx,
    (listing, matched, leads): (&ChatListing, Option<&Matched>, &LeadCounts),
    (current, today): (bool, bool),
  ) -> Column {
    let (meta, color, leading) = match (self.unchanged.get(), matched) {
      (Some(Unchanged { change, reason }), _) => {
        let reason = message_text(ctx, &reason);
        let said = ctx.t_args(change.refused_key(), [("reason", &*reason)]);
        (said, PaletteColor::Danger, false)
      }
      (None, Some(matched)) => (
        matched.meta(ctx, &when(ctx, listing, today)),
        PaletteColor::TextMuted,
        false,
      ),
      (None, None) => match meta::leads(ctx, listing, leads, today) {
        Some(leads) => (leads, PaletteColor::TextMuted, true),
        None => (meta::meta(ctx, listing, today), PaletteColor::TextMuted, false),
      },
    };
    let id = listing.chat.id();
    let title = Text::new(listing.chat.title())
      .id(format!("orchestrator-history-title-{id}"))
      .variant(OrchestratorText::Emphasis.role())
      .color(if current {
        PaletteColor::TextPrimary
      } else {
        PaletteColor::TextSecondary
      })
      .width(Dimension::Pct(100.0))
      .nowrap()
      .text_overflow(TextOverflow::Elipsis);
    let title = match matched {
      Some(matched) if !matched.title.is_empty() => title.highlights(marks(ctx, &matched.title)),
      _ => title,
    };
    Column::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .spacing(ExtraSpacing::Tight.role())
      .child(title)
      .with_children(matched.and_then(|matched| matched.line(ctx, id)))
      .child(meta::line(id, (&meta, color), leading))
  }

  /// The `⋯` button that opens the row's menu under it; pressed-looking while it holds the menu open.
  fn more(&self, ctx: &mut Ctx, listing: &ChatListing) -> Element {
    let (menu, pointer) = (self.menu.clone(), self.pointer.clone());
    let button = IconButton::new(
      format!("orchestrator-history-more-{}", listing.chat.id()),
      Icon::Ellipsis,
      ctx.t_args("orchestrator.historyPanel.more", [("title", listing.chat.title())]),
    )
    .small()
    .element(self.button.clone())
    .on_click(move || {
      pointer.set(None);
      menu.update(|open| *open = !*open);
    });
    if self.menu.get() && self.pointer.get().is_none() {
      button.active().render(ctx)
    } else {
      button.render(ctx)
    }
  }

  /// *Unarchive* before an archived row's `⋯` button under the pointer; nothing for a row History lists.
  fn unarchive(&self, ctx: &Ctx, listing: &ChatListing, rows: &Rows) -> Option<Element> {
    listing.archived?;
    let make = self.changer(listing, rows, Change::Unarchive);
    Some(
      ActionButton::new(
        format!("orchestrator-history-unarchive-{}", listing.chat.id()),
        ButtonKind::Secondary,
        ctx.t("orchestrator.historyPanel.unarchive"),
      )
      .icon(Icon::ArchiveRestore)
      .compact()
      .tight()
      .on_click(make)
      .into(),
    )
  }

  /// Makes `change` to the conversation of `listing`, forgetting why the last change did not happen.
  fn changer(&self, listing: &ChatListing, rows: &Rows, change: Change) -> impl Fn() + Send + Sync + 'static {
    let (action, unchanged, root, id) = (
      self.change.clone(),
      self.unchanged.clone(),
      rows.root.clone(),
      listing.chat.id(),
    );
    move || {
      unchanged.set(None);
      action.run((root.clone(), id, change));
    }
  }

  /// The row's menu: *Rename*, *Pin* or *Unpin* and *Archive* of a row History lists, *Unarchive* of an archived one,
  /// and after a line *Delete…*; under the `⋯` button at its end, or at the pointer that opened it.
  fn menu(&self, ctx: &mut Ctx, listing: &ChatListing, rows: &Rows, rename: &Arc<dyn Fn() + Send + Sync>) -> Element {
    let id = listing.chat.id();
    let rename = rename.clone();
    let (place, label, glyph, change) = match (listing.archived, listing.pinned) {
      (Some(_), _) => (
        "restore",
        "orchestrator.historyPanel.unarchive",
        Icon::ArchiveRestore,
        Change::Unarchive,
      ),
      (None, Some(_)) => ("pin", "orchestrator.historyPanel.unpin", Icon::PinOff, Change::Unpin),
      (None, None) => ("pin", "orchestrator.historyPanel.pin", Icon::Pin, Change::Pin),
    };
    let archive = listing.archived.is_none().then(|| {
      MenuItem::new(
        format!("orchestrator-history-archive-{id}"),
        ctx.t("orchestrator.historyPanel.archive"),
        self.changer(listing, rows, Change::Archive),
      )
      .icon(Icon::Archive)
      .render(ctx, &self.menu)
    });
    let deleting = rows.deleting.clone();
    let panel = menu_panel(&format!("orchestrator-history-menu-{id}"))
      .width(Dimension::Px(MENU_WIDTH))
      .child(
        MenuItem::new(
          format!("orchestrator-history-rename-{id}"),
          ctx.t("orchestrator.historyPanel.rename"),
          move || rename(),
        )
        .icon(Icon::Pencil)
        .render(ctx, &self.menu),
      )
      .child(
        MenuItem::new(
          format!("orchestrator-history-{place}-{id}"),
          ctx.t(label),
          self.changer(listing, rows, change),
        )
        .icon(glyph)
        .render(ctx, &self.menu),
      )
      .with_children(archive)
      .child(separator(ctx))
      .child(
        MenuItem::new(
          format!("orchestrator-history-delete-{id}"),
          ctx.t("orchestrator.historyPanel.delete.item"),
          move || deleting.set(Deleting(Some(id))),
        )
        .icon(Icon::Trash2)
        .danger()
        .render(ctx, &self.menu),
      );
    match self.pointer.get() {
      // The menu's corner at the pointer: the popup is placed from the row's bottom start.
      Some((x, y)) => {
        let (left, top, _, height) = self.row.rect();
        Popup::new(self.row.clone(), panel)
          .open(self.menu.clone())
          .placement(Placement::BottomStart)
          .offset(x - left, y - (top + height))
          .switch_on_anchor_press(false)
          .into()
      }
      None => menu_at_end(self.button.clone(), self.menu.clone(), panel).into(),
    }
  }

  /// The title as a field over the keys that save and cancel it, and why the last Enter kept no title.
  fn field(&self, ctx: &mut Ctx, listing: &ChatListing, rows: &Rows) -> Element {
    let id = listing.chat.id();
    if !self.seeded.get_untracked() {
      self.title.set(listing.chat.title().to_owned());
      self.seeded.set(true);
      self.focus.set(true);
    }
    if self.focus.get() {
      ctx.focus(&self.input);
      self.focus.set(false);
    }
    let field = FieldInput::new(
      format!("orchestrator-history-rename-field-{id}"),
      ctx.t("orchestrator.historyPanel.title"),
      self.title.clone(),
    )
    .text_style(OrchestratorText::Emphasis.role())
    .element(self.input.clone());
    let field = match self.refused.get() {
      Some(reason) => {
        let reason = message_text(ctx, &reason);
        field.error(ctx.t_args("orchestrator.historyPanel.notSaved", [("reason", &*reason)]))
      }
      None => field,
    };
    let (rename, title, input, rows) = (
      self.rename.clone(),
      self.title.clone(),
      self.input.clone(),
      rows.clone(),
    );
    Column::new()
      .id(format!("orchestrator-history-renaming-{id}"))
      .width(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .top(ExtraSpacing::Fine.role())
          .left(ExtraSpacing::Tight.role())
          .right(ExtraSpacing::Tight.role())
          .bottom(ExtraSpacing::Pair.role()),
      )
      .spacing(ExtraSpacing::Snug.role())
      .on_key_down(move |event: KeyboardEvent| {
        if event.key != "Enter" || !input.focus_signal().get_untracked() {
          return;
        }
        event.prevent_default();
        if rename.state().get_untracked().is_pending() {
          return;
        }
        rename.run(Renamed {
          rows: rows.clone(),
          chat: id,
          title: title.get_untracked(),
        });
      })
      .child(field.render(ctx))
      .child(keys(ctx, id))
      .into()
  }
}

/// *Enter saves* and *Esc cancels* under the title field, each key in its cap.
fn keys(ctx: &Ctx, id: ChatId) -> Row {
  let pair = |cap: &str, key: &str, does: &str| {
    Row::new()
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .child(key_cap(&format!("orchestrator-history-rename-{cap}-{id}"), key))
      .child(Text::new(does).variant(TypographyStyle::Caption).nowrap())
  };
  Row::new()
    .id(format!("orchestrator-history-rename-keys-{id}"))
    .padding(Padding::horizontal(ExtraSpacing::Tight.role()))
    .spacing(SpacingSize::Lg)
    .align_items(Alignment::Center)
    .child(pair(
      "enter",
      &ctx.t("orchestrator.historyPanel.enterKey"),
      &ctx.t("orchestrator.historyPanel.saves"),
    ))
    .child(pair(
      "esc",
      &ctx.t("orchestrator.historyPanel.escKey"),
      &ctx.t("orchestrator.historyPanel.cancels"),
    ))
}
