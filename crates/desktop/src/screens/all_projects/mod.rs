//! All projects (01.06 `Z7IZZ`, empty 01.06 C `A4FJG`): the projects the application database
//! knows, with where each one's runner stands, its active runs and the runs that need the user,
//! read without starting a runner. A row opens its project; *Add project* opens setup; *Open
//! project folder…* adds the project a folder already keeps (one removed from the list); the ×
//! removes a project from the list after a confirmation (01.06 A `EtMXj`, see [`remove`]), which
//! never touches its folder or its data. A project whose folder is gone, or whose data cannot be
//! read, is listed with why and cannot be opened.

mod remove;
mod row;

use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction, StreamEmitter},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_store::StoreError;

use self::{
  remove::{RemoveProject, RemoveProjectProps},
  row::{Rows, column_headings, project_row},
};
use crate::{
  activity::{Activity, operations},
  background,
  messages::{self, Message},
  navigation::Navigation,
  runner::Runners,
  screens::welcome,
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    icon::{Icon, IconSize, icon},
    message_text,
    notice::ownership_note,
    panel::RecoveryPanel,
  },
  workspace::{ListedData, OpenProject, ProjectEntry},
};

/// Width of the empty state's explanation and size of its icon surface (`pbFut`, `cwLjd`).
const EMPTY_WIDTH: f32 = 640.0;
const EMPTY_ICON_SIZE: f32 = 48.0;

/// Opens a project's folder, with what to say when it cannot be opened.
type OpenAction = FutureAction<PathBuf, OpenProject, Message>;
/// Asks for a folder in the native dialog and opens its project; `None` when no folder was chosen.
type PickAction = FutureAction<Arc<str>, Option<OpenProject>, Message>;

pub struct AllProjects {
  /// Counts removals, so the list is read again after one.
  revision: Signal<u64>,
  /// The project chosen for removal, and whether its confirmation is open.
  removing: Signal<Option<ProjectEntry>>,
  confirming: Signal<bool>,
}

impl Component for AllProjects {
  type Props = ();

  fn create(ctx: &mut Ctx) -> Self {
    let navigation = navigation(ctx);
    let (open, pick) = (open_action(ctx), pick_action(ctx));
    let shown = navigation.clone();
    ctx.watch(&open.state(), move |state| {
      if let Some(project) = state.data.clone().filter(|_| !state.is_pending()) {
        shown.show_project(project);
      }
    });
    ctx.watch(&pick.state(), move |state| {
      if let Some(Some(project)) = state.data.clone().filter(|_| !state.is_pending()) {
        navigation.show_project(project);
      }
    });
    Self {
      revision: ctx.signal(0),
      removing: ctx.signal(None),
      confirming: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    // The actions first, in the order `create` asked for them.
    let (open, pick) = (open_action(ctx), pick_action(ctx));
    let navigation = navigation(ctx);
    let runners = ctx
      .use_context::<Runners>()
      .expect("the root provides Runners before mounting screens");
    let work = work_changes(ctx, &runners);
    let workspace = navigation.workspace().clone();
    let load = ctx.future((workspace, self.revision.get()), |(workspace, _)| async move {
      background::run(move || workspace.entries()).await?.map_err(|error| {
        tracing::error!(%error, "cannot read the projects of the workspace");
        messages::store_error(&error)
      })
    });
    let state = load.state().get();
    let data = self.data(ctx, state.data.clone().unwrap_or_default(), work);
    let (opened, picked) = (open.state().get(), pick.state().get());
    let failure = [(opened.is_pending(), opened.error), (picked.is_pending(), picked.error)]
      .into_iter()
      .find_map(|(pending, error)| error.filter(|_| !pending));
    let content = match (&state.data, &state.error) {
      (_, Some(error)) => RecoveryPanel::new(
        Icon::AlertCircle,
        &ctx.t("projects.loadFailed"),
        &message_text(ctx, error),
      )
      .into(),
      (Some(entries), None) if entries.is_empty() => empty(ctx),
      (Some(entries), None) => self.list(
        ctx,
        entries,
        &data,
        &Rows {
          runners: &runners,
          open: &open,
        },
      ),
      (None, None) => Column::new().into(),
    };
    Column::new()
      .id("all-projects")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(welcome::topbar(ctx))
      .child(
        Column::new()
          .width(Dimension::Pct(100.0))
          .flex(1.0)
          .padding_vertical(ExtraSpacing::Rail.role())
          .padding_horizontal(ExtraSpacing::Page.role())
          .spacing(SpacingSize::Lg)
          .child(heading(ctx, &navigation, &pick))
          .with_children(failure.map(|failure| open_error(ctx, &failure)))
          .child(content),
      )
      .child(self.remove_dialog(ctx, &runners))
  }
}

impl AllProjects {
  /// Column headings, the rows and what removing means (`z69IPR`, `U5geh`).
  fn list(&self, ctx: &Ctx, entries: &[ProjectEntry], data: &ListedData, rows: &Rows<'_>) -> Element {
    let (removing, confirming) = (self.removing.clone(), self.confirming.clone());
    let list = Column::new()
      .id("project-list")
      .width(Dimension::Pct(100.0))
      .child(column_headings(ctx))
      .with_children(entries.iter().enumerate().map(|(index, entry)| {
        let (removing, confirming, chosen) = (removing.clone(), confirming.clone(), entry.clone());
        project_row(ctx, index, (entry, data.of(entry.project.id)), rows, move || {
          removing.set(Some(chosen.clone()));
          confirming.set(true);
        })
      }));
    Column::new()
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .spacing(SpacingSize::Lg)
      .child(ScrollVertical::new(list).width(Dimension::Pct(100.0)))
      .child(ownership_note(&ctx.t("projects.removal.note")))
      .into()
  }

  /// What each listed project's database says, read after the list is shown and again when a
  /// runner reports; the previous data stays on screen while it is read again.
  fn data(&self, ctx: &mut Ctx, entries: Vec<ProjectEntry>, work: u64) -> ListedData {
    let load = ctx.future((entries, work), |(entries, _)| async move {
      background::run(move || ListedData::read(&entries)).await
    });
    load.state().get().data.unwrap_or_default()
  }

  /// The confirmation of the project chosen for removal (01.06 A `EtMXj`).
  fn remove_dialog(&self, ctx: &mut Ctx, runners: &Runners) -> Element {
    let Some(entry) = self.removing.get().filter(|_| self.confirming.get()) else {
      return Column::new().into();
    };
    let agents_working = runners
      .status_of(&entry.project.repo_root)
      .and_then(|status| status.work().map(|work| work.agents_working()))
      .unwrap_or(0);
    let props = RemoveProjectProps {
      entry: entry.clone(),
      agents_working,
      open: self.confirming.clone(),
      revision: self.revision.clone(),
    };
    ctx.mount_keyed::<RemoveProject>(&format!("remove-project-{}", entry.project.id), props)
  }
}

fn navigation(ctx: &mut Ctx) -> Navigation {
  ctx
    .use_context::<Navigation>()
    .expect("the root provides Navigation before mounting screens")
}

/// "All projects", what it lists, and the actions (`QLGxO`).
fn heading(ctx: &Ctx, navigation: &Navigation, pick: &PickAction) -> Element {
  let setup = navigation.clone();
  let add = ActionButton::new(ctx.t("projects.add"))
    .primary()
    .icon(Icon::Plus)
    .id("all-projects-add")
    .on_click(move || setup.show_setup())
    .render(ctx);
  let (run, title) = (pick.clone(), ctx.t("projects.openFolderTitle"));
  let open_folder = ActionButton::new(ctx.t("projects.openFolder"))
    .icon(Icon::FolderOpen)
    .id("all-projects-open-folder")
    .enabled(!pick.state().get().is_pending())
    .on_click(move || run.run(title.clone()))
    .render(ctx);
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::End)
    .child(
      Column::new()
        .flex(1.0)
        .spacing(ExtraSpacing::Inline.role())
        .child(Text::new(&ctx.t("projects.title")).variant(ExtraTypography::ScreenTitle.role()))
        .child(
          Text::new(&ctx.t("projects.description"))
            .variant(TypographyStyle::Body)
            .color(PaletteColor::TextSecondary),
        ),
    )
    .child(open_folder)
    .child(add)
    .into()
}

/// No projects in the list (`k0D71S`); the actions stay in the heading.
fn empty(ctx: &Ctx) -> Element {
  let surface = Row::new()
    .size(Dimension::Px(EMPTY_ICON_SIZE), Dimension::Px(EMPTY_ICON_SIZE))
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .rounded(RadiusSize::Lg)
    .child(icon(Icon::Folders, IconSize::Large, PaletteColor::TextPrimary));
  Column::new()
    .id("all-projects-empty")
    .width(Dimension::Pct(100.0))
    .padding_top(SpacingSize::Sm)
    .spacing(SpacingSize::Md)
    .child(surface)
    .child(
      Column::new()
        .width(Dimension::Px(EMPTY_WIDTH))
        .spacing(ExtraSpacing::Inline.role())
        .child(Text::new(&ctx.t("projects.empty.title")).variant(ExtraTypography::DialogTitle.role()))
        .child(Text::new(&ctx.t("projects.empty.summary")).variant(ExtraTypography::Summary.role())),
    )
    .into()
}

/// Opens the project in a listed folder; the row's project was checked when the list was read,
/// but its folder may have gone since.
fn open_action(ctx: &mut Ctx) -> OpenAction {
  let workspace = navigation(ctx).workspace().clone();
  ctx.future_action(move |repo_root: PathBuf| {
    let workspace = workspace.clone();
    async move {
      let folder = repo_root.clone();
      background::run(move || workspace.open(&repo_root))
        .await?
        .map_err(|error| {
          tracing::error!(%error, "cannot open a listed project");
          open_failure(&folder, &error)
        })
    }
  })
}

/// *Open project folder…*: the native folder dialog, then the project the folder keeps.
fn pick_action(ctx: &mut Ctx) -> PickAction {
  let workspace = navigation(ctx).workspace().clone();
  let activity = Activity::of(ctx);
  ctx.future_action(move |title: Arc<str>| {
    let (workspace, activity) = (workspace.clone(), activity.clone());
    async move {
      // Listed while the dialog is open; closing it without a folder leaves no event.
      let running = activity.start(operations::repository_pick());
      let picked = background::run(move || rfd::FileDialog::new().set_title(&*title).pick_folder()).await?;
      let Some(folder) = picked else {
        return Ok(None);
      };
      running.finish();
      let chosen = folder.clone();
      background::run(move || workspace.open(&chosen))
        .await?
        .map(Some)
        .map_err(|error| {
          tracing::error!(%error, "cannot open the project of a chosen folder");
          open_failure(&folder, &error)
        })
    }
  })
}

/// Why the project in `folder` could not be opened (01.06 D `ucI7v`), naming the folder.
fn open_failure(folder: &Path, error: &StoreError) -> Message {
  let key = match error {
    StoreError::NotInitialized(_) => "projects.errors.noProject",
    StoreError::SchemaTooNew { .. } => "projects.errors.newerData",
    _ => "projects.errors.notRead",
  };
  Message::new(key).with("path", folder.display().to_string())
}

/// The error line between the heading and the list (`tmE5H`); it stays until the next attempt.
fn open_error(ctx: &Ctx, failure: &Message) -> Element {
  Row::new()
    .id("all-projects-error")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::AlertCircle, IconSize::Dismiss, PaletteColor::TextPrimary))
    .child(
      Text::new(&message_text(ctx, failure))
        .variant(TypographyStyle::FieldLabel)
        .color(PaletteColor::TextPrimary)
        .flex(1.0),
    )
    .into()
}

/// Counts the runners' reports, so the runner states and counts follow them.
fn work_changes(ctx: &mut Ctx, runners: &Runners) -> u64 {
  let changes = ctx.stream(runners.clone(), |runners, emitter: StreamEmitter<u64, ()>| async move {
    let mut updates = runners.work_changes();
    loop {
      let revision = *updates.borrow_and_update();
      if !emitter.emit(revision) || updates.changed().await.is_err() {
        return;
      }
    }
  });
  changes.state().get().data.unwrap_or(0)
}
