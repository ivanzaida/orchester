//! The code viewer (`docs/design/code-viewer.md`; layer Files in `orchester2.pen`, `Files · plan_limits.rs ·
//! lines 441–442` `o1Uea4` and its states): a project's file at a version, read-only. Its header names the
//! file and where it was read, with *Go to file…*, the version switch, *Copy* and *Open in editor*; under it
//! the folder tree and the file, its lines highlighted as on a task's Changes tab, a range of them
//! selected. A version that no longer exists, a path the page refuses, a binary or too large file and no
//! file at all each show their own state. The file's text is sensitive: it is painted but never in the MCP
//! or DevTools tree and is covered in screenshots, so automation driving the window cannot read files
//! through it; file names and the tree stay visible.

mod actions;
mod code;
mod find;
mod follow;
mod goto;
mod header;
mod keys;
mod line;
mod reads;
mod search;
mod shown_lines;
mod states;
mod tree;
mod version;

use std::{
  collections::BTreeSet,
  sync::{Arc, atomic::AtomicBool},
};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::{Column, Row},
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension},
};
use orchester_core::id::RunId;
use orchester_workspace::files::{
  FileList, FilePath, FileView, LineRange, PathError, RunFacts, Version, VersionChoice, Versions,
};

use self::reads::Shown;
use crate::{
  navigation::{Navigation, link::Open},
  project::ShownProject,
  routes::{FilesView, ProjectPage, Route},
};

/// The folders open in the tree.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OpenFolders(Arc<BTreeSet<Arc<str>>>);

impl DevtoolsInspectable for OpenFolders {}

impl OpenFolders {
  pub fn set(&self) -> &BTreeSet<Arc<str>> {
    &self.0
  }

  /// These folders and `more`.
  #[must_use]
  pub fn with<'a>(&self, more: impl IntoIterator<Item = &'a str>) -> Self {
    let mut set = (*self.0).clone();
    set.extend(more.into_iter().map(Arc::from));
    Self(Arc::new(set))
  }

  /// These folders with `folder` open or closed.
  #[must_use]
  pub fn toggled(&self, folder: &str) -> Self {
    let mut set = (*self.0).clone();
    if !set.remove(folder) {
      set.insert(Arc::from(folder));
    }
    Self(Arc::new(set))
  }
}

/// The run the page was opened from, kept while another version is shown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OpenedFrom(Option<RunId>);

impl DevtoolsInspectable for OpenedFrom {}

/// What the page says after a press it did not carry out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
  /// Synthetic input pressed *Copy* or *Open in editor*.
  PersonOnly,
  /// *Open in editor* found, as it opened, that the file does not open ([`orchester_workspace::files::open_with`]).
  NotOpened(orchester_workspace::files::Refusal),
  /// The file could not be opened.
  OpenFailed,
}

impl Refusal {
  pub const fn key(self) -> &'static str {
    match self {
      Self::PersonOnly => "files.personOnly",
      Self::NotOpened(refusal) => actions::refusal_key(refusal),
      Self::OpenFailed => "files.openFailed",
    }
  }
}

impl DevtoolsInspectable for Refusal {}

/// The page's own state, kept while its route changes file, version or lines.
#[derive(Clone)]
pub struct View {
  pub open: Signal<OpenFolders>,
  /// The file whose folders were opened for it, so they open once per file.
  opened_for: Signal<Option<FilePath>>,
  run: Signal<OpenedFrom>,
  pub retry: Signal<u64>,
  pub find: find::FindState,
  pub goto: goto::GotoState,
  pub version_open: Signal<bool>,
  pub version_anchor: ElementRef,
  /// The header's title row and its actions, measured after each layout: whether the actions fit beside the
  /// title ([`header::Room`]).
  pub title_row: ElementRef,
  pub actions: ElementRef,
  pub header_room: Signal<header::Room>,
  pub refusal: Signal<Option<Refusal>>,
  /// Whether the highlighting under way is still wanted: a newer text turns the older pass's flag off.
  pub highlighting: Arc<parking_lot::Mutex<Arc<AtomicBool>>>,
}

/// The page as one render sees it.
pub struct Page<'a> {
  pub project: &'a ShownProject,
  pub route: &'a FilesView,
  /// The route's file, checked; `None` when it names none.
  pub path: Option<Result<FilePath, PathError>>,
  pub versions: Option<Arc<Versions>>,
  /// The file's read, when the route names a file the page opens.
  pub read: Option<Shown<FileView>>,
  /// Why the text shown may be out of date: reading it again failed more than for a moment.
  pub stale: Option<reads::Failure>,
  pub view: &'a View,
}

impl Page<'_> {
  pub fn file(&self) -> Option<&FilePath> {
    self.path.as_ref().and_then(|path| path.as_ref().ok())
  }

  pub fn at(&self) -> &Version {
    self.route.at()
  }

  /// The versions that exist, once they are read.
  pub fn choices(&self) -> &[VersionChoice] {
    self.versions.as_ref().map_or(&[], |versions| &versions.choices)
  }

  /// The choice of the version shown, once the versions are read.
  pub fn shown_choice(&self) -> Option<&VersionChoice> {
    self.choices().iter().find(|choice| choice.version == *self.at())
  }

  /// The run the page was opened from, once it is named.
  pub fn run_facts(&self) -> Option<&RunFacts> {
    self.versions.as_ref()?.run.as_ref()
  }

  /// The route to `file` at `at`, on no particular lines.
  pub fn route_to(&self, file: Option<&str>, at: Version) -> Route {
    Route::Project(
      self.project.open.project.id(),
      ProjectPage::Files(FilesView::new(file, at)),
    )
  }

  /// What opens `file` at the version shown where a press asks; in the active tab, as a new entry of its history.
  pub fn open_file(&self, ctx: &mut Ctx, file: &str) -> Open {
    Navigation::of(ctx).to(self.route_to(Some(file), self.at().clone()))
  }

  /// Shows the same file and lines at `at`, as a new entry of the tab's history.
  pub fn switch_to(&self, ctx: &mut Ctx, at: Version) -> impl Fn() + Send + Sync + 'static + use<> {
    let navigation = Navigation::of(ctx);
    let view = self.route.clone().at_version(at);
    let route = Route::Project(self.project.open.project.id(), ProjectPage::Files(view));
    move || navigation.go(route.clone())
  }

  /// Selects `lines` of the file, as a new entry of the tab's history.
  pub fn select_lines(&self, ctx: &mut Ctx) -> impl Fn(Option<LineRange>) + Send + Sync + 'static + use<> {
    let navigation = Navigation::of(ctx);
    let (project, view) = (self.project.open.project.id(), self.route.clone());
    move |lines| {
      let view = view.clone().with_line(lines);
      navigation.go(Route::Project(project, ProjectPage::Files(view)));
    }
  }
}

#[derive(Clone, PartialEq)]
pub struct Props {
  project: ShownProject,
  route: FilesView,
}

impl DevtoolsInspectable for Props {}

struct FilesScreen {
  view: View,
  following: follow::Following,
  /// The file and version the page showed last: moving to others closes the version menu and *Go to file*.
  shown: parking_lot::Mutex<Option<(Option<String>, Version)>>,
}

impl Component for FilesScreen {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      view: View {
        open: ctx.signal(OpenFolders::default()),
        opened_for: ctx.signal(None),
        run: ctx.signal(OpenedFrom::default()),
        retry: ctx.signal(0),
        find: find::FindState::new(ctx),
        goto: goto::GotoState::new(ctx),
        version_open: ctx.signal(false),
        version_anchor: ElementRef::new(),
        title_row: ElementRef::new(),
        actions: ElementRef::new(),
        header_room: ctx.signal(header::Room::default()),
        refusal: ctx.signal(None),
        highlighting: Arc::default(),
      },
      following: follow::Following::default(),
      shown: parking_lot::Mutex::new(None),
    }
  }

  /// Keeps the header's measured room after a layout that changed it, e.g. a resized window.
  fn after_layout(&self) {
    let view = &self.view;
    if !view.title_row.is_attached() || !view.actions.is_attached() {
      return;
    }
    let room = header::Room::new(view.title_row.width(), view.actions.width());
    if room != view.header_room.get_untracked() {
      view.header_room.set(room);
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let view = &self.view;
    let path = props.route.file().map(FilePath::new);
    let run = match props.route.at() {
      Version::Run(run) => Some(*run),
      _ => view.run.get().0,
    };
    if run != view.run.get_untracked().0 {
      view.run.set(OpenedFrom(run));
    }
    self.open_folders_of(path.as_ref().and_then(|path| path.as_ref().ok()));
    self.close_menus_on_move(&props.route);
    let root = props.project.open.repo_root.clone();
    let wake = self
      .following
      .wake(ctx, &props.project, (props.route.at(), view.retry.get()));
    let file = path.as_ref().and_then(|path| path.as_ref().ok());
    let choices = reads::choices(ctx, &root, (props.route.at(), run, file), wake);
    // A future belongs to its place among the render's futures: the tree's comes before those read only with a
    // file, so selecting the first file does not read the tree again.
    let list = reads::list(ctx, &root, props.route.at(), (wake, &self.following.list));
    let following = (
      wake,
      &self.following.file,
      &self.following.reads,
      self.following.follows_runs(),
    );
    let (read, stale) = file
      .map(|file| reads::file(ctx, &root, (props.route.at(), file), following))
      .unzip();
    let page = Page {
      project: &props.project,
      route: &props.route,
      path: path.clone(),
      versions: choices.value().cloned(),
      read,
      stale: stale.flatten(),
      view,
    };
    let gone = matches!(&list, Shown::Failed(failure) if failure.gone());
    let header = header::header(ctx, &page, gone);
    let body = if gone {
      states::version_gone(ctx, &page)
    } else {
      body(ctx, &page, &list)
    };
    let search = goto::search(ctx, &page, list.value());
    let keys = keys::handler(&page);
    Column::new()
      .id("screen-files")
      .describe("at", page.at().to_string())
      .describe("reads", self.following.reads.started().to_string())
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .on_key_down(keys)
      .child(header)
      .child(body)
      .child(search)
  }
}

impl FilesScreen {
  /// Closes the version menu and *Go to file* once the page shows another file or version, however it got
  /// there (a link, automation's `navigate`, the tree).
  fn close_menus_on_move(&self, route: &FilesView) {
    let now = (route.file().map(str::to_owned), route.at().clone());
    let mut shown = self.shown.lock();
    if shown.as_ref().is_some_and(|before| *before != now) {
      if self.view.version_open.get_untracked() {
        self.view.version_open.set(false);
      }
      self.view.goto.close();
    }
    *shown = Some(now);
  }

  /// Opens the folders down to `file` the first time the page shows it.
  fn open_folders_of(&self, file: Option<&FilePath>) {
    let Some(file) = file else {
      return;
    };
    if self.view.opened_for.get_untracked().as_ref() == Some(file) {
      return;
    }
    self.view.opened_for.set(Some(file.clone()));
    let open = self.view.open.get_untracked().with(file.folders());
    self.view.open.set(open);
  }
}

/// The tree beside the file, or what stands for the file.
fn body(ctx: &mut Ctx, page: &Page<'_>, list: &Shown<FileList>) -> Element {
  let tree = tree::panel(ctx, page, list);
  let file = code::pane(ctx, page);
  Row::new()
    .id("files-body")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .min_height(Dimension::Px(0.0))
    .child(tree)
    .child(file)
    .into()
}

pub fn screen(ctx: &mut Ctx, project: &ShownProject, route: &FilesView) -> Element {
  ctx.mount_keyed::<FilesScreen>(
    &format!("files-{}", project.open.project.id()),
    Props {
      project: project.clone(),
      route: route.clone(),
    },
  )
}
