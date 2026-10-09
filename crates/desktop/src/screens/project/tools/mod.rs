//! Tools (06.01 `ZFwWl`, 06.02 `YEWFo`): the project's tool library, and one tool in its editor.
//! Each tool follows Draft saved → Publish vN → Published vN; saving a draft does not publish it.
//! Only command tools can be created and edited in this version: HTTP, database, MCP and pull
//! request tools need project secrets or approvals and are shown as unavailable.

mod editor;
mod fields;
mod inspector;
mod library;
mod row;

use std::path::PathBuf;

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
  },
  components::Column,
  core::Signal,
  layout::{
    Alignment,
    layout_kind::{Justify, ScrollState},
  },
  node::{Element, dimension::Dimension},
};
use orchester_core::id::ResourceId;

use self::editor::ToolEditor;
use super::ProjectNav;
use crate::{
  activity::{OperationSpec, operations},
  background,
  messages::{self, Message},
  navigation::route::{Page, Route, ToolSelection},
  tools::data::{ProjectTools, ToolChange, ToolEntry, apply_in},
  ui::{icon::Icon, message_text, panel::RecoveryPanel},
  workspace::OpenProject,
};

/// The tool a change applied to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Changed(ResourceId);

impl lurq::app::component::DevtoolsInspectable for Changed {}

type ChangeArgs = (PathBuf, ToolChange, OperationSpec);
type ChangeAction = FutureAction<ChangeArgs, Changed, Message>;

/// What the editor edits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Editing {
  New,
  /// Boxed: a tool is much larger than nothing.
  Tool(Box<ToolEntry>),
}

/// Provided to the editor when it is mounted: the data it starts from and how to save.
#[derive(Clone)]
pub struct EditorContext {
  pub project: OpenProject,
  pub tools: ProjectTools,
  pub editing: Editing,
  pub change: ChangeAction,
  pub nav: ProjectNav,
}

/// The project's tools, and the revision they were read at.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Loaded {
  revision: u64,
  tools: ProjectTools,
}

impl lurq::app::component::DevtoolsInspectable for Loaded {}

pub struct ToolsScreen {
  /// Bumped after a change, to read the tools again.
  revision: Signal<u64>,
  nav: ProjectNav,
  /// Where the library is scrolled: kept by the screen, so a tab that goes inactive and back
  /// keeps it.
  scroll: ScrollState,
}

impl Component for ToolsScreen {
  type Props = OpenProject;

  fn create(ctx: &mut Ctx) -> Self {
    let nav = ctx
      .use_context::<ProjectNav>()
      .expect("the project shell provides ProjectNav");
    let revision = ctx.signal(0_u64);
    let change = change_action(ctx);
    let (reload, select) = (revision.clone(), nav.clone());
    ctx.watch(&change.state(), move |state| {
      if let (false, Some(Changed(id))) = (state.is_pending(), state.data) {
        // The saved tool takes the place of what was edited, a new tool's form included.
        select.replace(Page::Tools(ToolSelection::Tool(id)));
        reload.update(|revision| *revision += 1);
      }
    });
    Self {
      revision,
      nav,
      scroll: ScrollState::new(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let open = ctx.props::<OpenProject>().clone();
    // Futures are matched by call order: the action comes first, as in `create`.
    let change = change_action(ctx);
    let deps = (open.repo_root.clone(), self.revision.get());
    let load = ctx.future(deps, |(repo_root, revision)| async move {
      background::run(move || ProjectTools::load_in(&repo_root))
        .await?
        .map(|tools| Loaded { revision, tools })
        .map_err(|error| {
          tracing::error!(%error, "cannot read the project's tools");
          messages::store_error(&error)
        })
    });
    let state = load.state().get();
    let selection = match Route::current_page(ctx) {
      Some(Page::Tools(selection)) => selection,
      _ => ToolSelection::Library,
    };
    match (state.data, state.error) {
      (_, Some(error)) => centered(failed(ctx, &error)),
      (None, None) => Column::new().into(),
      (Some(loaded), None) => self.loaded(ctx, &open, loaded, selection, change),
    }
  }
}

impl ToolsScreen {
  fn loaded(
    &self,
    ctx: &mut Ctx,
    open: &OpenProject,
    loaded: Loaded,
    selection: ToolSelection,
    change: ChangeAction,
  ) -> Element {
    let editing = match selection {
      ToolSelection::Library => return library::library(ctx, open, &loaded.tools, (&self.nav, &self.scroll)),
      ToolSelection::New => Editing::New,
      ToolSelection::Tool(id) => match loaded.tools.tool(id) {
        Some(entry) => Editing::Tool(Box::new(entry.clone())),
        // A tool that no longer exists: the library shows what there is.
        None => return library::library(ctx, open, &loaded.tools, (&self.nav, &self.scroll)),
      },
    };
    let key = match &editing {
      Editing::New => format!("tool-editor-new-{}", loaded.revision),
      Editing::Tool(entry) => format!("tool-editor-{}-{}", entry.id, loaded.revision),
    };
    ctx.provide(EditorContext {
      project: open.clone(),
      tools: loaded.tools,
      editing,
      change,
      nav: self.nav.clone(),
    });
    ctx.mount_keyed::<ToolEditor>(&key, ())
  }
}

fn centered(content: Element) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(content)
    .into()
}

fn failed(ctx: &Ctx, error: &Message) -> Element {
  RecoveryPanel::new(Icon::AlertCircle, &ctx.t("tools.loadFailed"), &message_text(ctx, error)).into()
}

fn change_action(ctx: &mut Ctx) -> ChangeAction {
  let activity = crate::activity::of(ctx);
  ctx.future_action(move |(repo_root, change, spec): ChangeArgs| {
    let activity = activity.clone();
    async move {
      let root = repo_root.clone();
      let apply = async move {
        let applied = background::run(move || apply_in(&repo_root, change)).await?;
        applied.map(Changed).map_err(|error| {
          tracing::error!(%error, "cannot save the tool");
          messages::store_error(&error)
        })
      };
      let target = |changed: &Changed| Some(operations::tool(&root, changed.0));
      activity.track_to(spec, apply, target).await
    }
  })
}
