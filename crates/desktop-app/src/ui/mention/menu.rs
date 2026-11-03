//! The @ menu (`@ menu` `qhBhC` in 08 `gOs7u`, 08b `nUjD7`, 08c `GdUNZ`; `Am5KT`, `vnuo8`, `u7Q7h` in a run's
//! Conversation): a `Menu` panel 560 wide that opens 8 over the composer's box with its left edge at the `@`, as the
//! composer sits at the bottom of the window, with the sections [`super::rows`] draws. The tasks and runs are read
//! when it opens. The repository's files (the project's checkout, or the run's worktree in a run's Conversation) are
//! listed off the UI thread each time it opens and while it is open when the project changes; the last listing stays
//! shown meanwhile, and only the rows shown are built, so a large repository keeps it responsive.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::SpacingSize,
  },
  components::{Placement, Popup},
  core::ElementRef,
  node::{Element, dimension::Dimension},
};
use orchester_core::{id::RunId, project::TaskKey, run::RunNumber};
use orchester_workspace::{
  background,
  mentions::{catalog::Catalog, files::FileIndex},
  messages::Message,
};
use parking_lot::Mutex;

use super::{
  MentionField, Shown,
  rows::{Files, nothing, rows},
};
use crate::{
  services::Services,
  ui::{menu::menu_panel, message::message_text},
};

/// The menu's width (`qhBhC`).
const WIDTH: f32 = 560.0;

/// The menu of the composer named `id`, over its box `anchor` at the field's `@`, for the project at `root` called
/// `project`; `own`, the task and run a run's Conversation shows, leads, and `run`, that run, is whose worktree's files
/// it offers (the project's checkout's without one). `revision` moves when the project's data change.
#[derive(Clone)]
pub struct MenuProps {
  pub id: &'static str,
  pub field: MentionField,
  pub anchor: ElementRef,
  pub root: PathBuf,
  pub project: Arc<str>,
  pub own: Option<(TaskKey, Option<RunNumber>)>,
  pub run: Option<RunId>,
  pub revision: u64,
}

/// Compared by what it shows; the anchor is the composer's for as long as it is shown.
impl PartialEq for MenuProps {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
      && self.field == other.field
      && self.root == other.root
      && self.project == other.project
      && self.own == other.own
      && self.run == other.run
      && self.revision == other.revision
  }
}

impl DevtoolsInspectable for MenuProps {}

pub fn menu(ctx: &mut Ctx, props: MenuProps) -> Element {
  ctx.mount_keyed::<MentionMenu>(&format!("{}-mentions", props.id), props)
}

/// Whose files the menu lists: the project's root, and the run whose worktree it lists there.
type Folder = (PathBuf, Option<RunId>);
/// The files of one folder as the menu listed them last.
type Kept = (Folder, FileIndex);
/// What a listing is for: the menu's opening and the project's revision while it is open.
type ListingKey = Option<(u64, u64)>;

struct MentionMenu {
  /// The `@` and the words typed after it the menu showed last: its focus goes back to the first row when they change.
  last: Mutex<Option<(usize, String)>>,
  /// Whether the menu was open at its last render, and how many times it opened: each opening lists the files again.
  opens: Mutex<(bool, u64)>,
  /// The files listed last, shown while they are listed again.
  kept: Mutex<Option<Kept>>,
}

impl Component for MentionMenu {
  type Props = MenuProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self {
      last: Mutex::new(None),
      opens: Mutex::new((false, 0)),
      kept: Mutex::new(None),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<MenuProps>().clone();
    let field = &props.field;
    if field.refocus.get() {
      field.refocus.set(false);
      field.controller.focus(ctx);
    }
    let text = field.draft.get();
    let typed = field.typed(&text);
    let closed = field.closed.get();
    if closed.is_some() && typed.map(|typed| typed.at) != closed {
      field.closed.set(None);
    }
    let open = field.focused.get() && typed.is_some_and(|typed| Some(typed.at) != closed);
    let catalog = read(ctx, (props.root.clone(), props.revision, open));
    let files = self.files(ctx, &props, open);
    let (Some(typed), true) = (typed, open) else {
      *field.shown.lock() = None;
      return Element::new();
    };
    let content = match catalog {
      Read::Reading => {
        *field.shown.lock() = None;
        return Element::new();
      }
      Read::Failed(reason) => {
        *field.shown.lock() = None;
        let reason = message_text(ctx, &reason);
        let words = ctx.t_args("composer.mention.unreadable", [("reason", reason)]);
        vec![nothing((&ctx.t("composer.mention.unreadableTitle"), &words))]
      }
      Read::Ready(catalog) => {
        let own = props.own.as_ref().map(|(key, number)| (key, *number));
        let mut offer = catalog.offer(typed.query, own);
        if let (Files::Listed, Some(index)) = &files {
          offer = offer.with_files(index.offer(typed.query));
        }
        let focused = self.focused(field, (typed.at, typed.query), offer.mentions().len());
        *field.shown.lock() = Some(Shown {
          replaces: typed.range(),
          rows: offer.mentions(),
        });
        rows(ctx, &props, (&offer, typed.query, &files.0), focused)
      }
    };
    let panel = menu_panel(&format!("{}-mention-menu", props.id))
      .width(Dimension::Px(WIDTH))
      .with_children(content);
    // Its left edge at the `@` (the field's marker), its bottom 8 over the box: the marker sits at the top of the
    // field, as far under the box's top as the last layout put it.
    let marker = field.marker();
    let under_top = (marker.y() - props.anchor.y()).max(0.0);
    let gap = ctx.theme().spacing_value(SpacingSize::Sm).to_px();
    Popup::new(marker, panel)
      .open_when(true)
      .dismiss_on_outside_click(false)
      .dismiss_on_escape(false)
      .switch_on_anchor_press(false)
      .placement(Placement::TopStart)
      .offset(0.0, gap + under_top)
      .into()
  }
}

impl MentionMenu {
  /// The row with the focus among `rows` for what was typed after the `@` at `at`: the first once that changed. The
  /// focus is read tracked on every path, so ↑ ↓ draw the menu again.
  fn focused(&self, field: &MentionField, (at, query): (usize, &str), rows: usize) -> usize {
    let focus = field.focus.get();
    let mut last = self.last.lock();
    if last.as_ref().is_none_or(|(was_at, was)| *was_at != at || was != query) {
      *last = Some((at, query.to_owned()));
      if focus != 0 {
        field.focus.set(0);
      }
      return 0;
    }
    focus.min(rows.saturating_sub(1))
  }

  /// How many times the menu opened, now that it is `open` or not.
  fn opened(&self, open: bool) -> u64 {
    let mut opens = self.opens.lock();
    if open && !opens.0 {
      opens.1 += 1;
    }
    opens.0 = open;
    opens.1
  }

  /// The repository's files the menu offers, and where they stand: listed off the UI thread once each time it opens,
  /// and again while it is open when the project's data change; the last listing of the same folder stays meanwhile.
  fn files(&self, ctx: &mut Ctx, props: &MenuProps, open: bool) -> (Files, Option<FileIndex>) {
    let folder = (props.root.clone(), props.run);
    let opened = self.opened(open);
    let listing = open.then_some((opened, props.revision));
    let read = list(ctx, (folder.clone(), listing));
    let mut kept = self.kept.lock();
    if kept.as_ref().is_some_and(|(was, _)| *was != folder) {
      *kept = None;
    }
    if let Some(index) = read.index {
      *kept = Some((folder, index));
    }
    match (read.failed, kept.as_ref()) {
      (Some(reason), _) => (Files::Unlisted(reason), None),
      (None, Some((_, index))) => (Files::Listed, Some(index.clone())),
      (None, None) => (Files::Listing, None),
    }
  }
}

/// The project's tasks and runs as the menu reads them.
enum Read {
  Reading,
  Ready(Catalog),
  Failed(Message),
}

/// The tasks and runs of the project at `root`, read off the UI thread while the menu is `open`, again when it opens
/// and when the project's tasks change (`revision`).
fn read(ctx: &mut Ctx, (root, revision, open): (PathBuf, u64, bool)) -> Read {
  let state = ctx
    .future((root, revision, open), |(root, _, open)| async move {
      if !open {
        return Ok(None);
      }
      background::run(move || Catalog::read_in(&root)).await?.map(Some)
    })
    .state()
    .get();
  let pending = state.is_pending();
  // A failed re-read keeps the last data; it must not stay answerable (rows to insert) as if current.
  match (state.error, state.data.flatten()) {
    (Some(error), _) if !pending => Read::Failed(error),
    (None, Some(catalog)) => Read::Ready(catalog),
    _ => Read::Reading,
  }
}

/// What one listing of the files gave: the index, or why there is none.
struct Listing {
  index: Option<FileIndex>,
  failed: Option<Message>,
}

/// The files of `folder` (the project's root, and the run whose worktree it lists), listed off the UI thread for each
/// `listing` (the menu's opening and the project's revision); nothing while the menu is closed (`None`).
fn list(ctx: &mut Ctx, (folder, listing): (Folder, ListingKey)) -> Listing {
  let programs = Services::of(ctx).programs;
  let state = ctx
    .future((folder, listing), move |((root, run), listing)| {
      let programs = programs.clone();
      async move {
        if listing.is_none() {
          return Ok(None);
        }
        background::run(move || FileIndex::read_in(&programs, &root, run))
          .await?
          .map(Some)
      }
    })
    .state()
    .get();
  let pending = state.is_pending();
  Listing {
    failed: state.error.filter(|_| !pending),
    index: state.data.flatten(),
  }
}
