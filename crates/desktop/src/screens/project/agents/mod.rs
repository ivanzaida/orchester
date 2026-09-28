//! Agents (04.03 `WPmtS`): the project's agent profiles beside the selected profile's editor.
//! Each profile follows Draft saved → Publish vN → Published vN; saving a draft does not
//! publish it. Importing profiles does not exist yet and is unavailable.

mod editor;
mod enforcement;
mod fields;
mod folders;
mod list;
mod mcp;
mod permissions;
mod tabs;

use std::path::PathBuf;

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::SpacingSize,
  },
  components::{Column, Row},
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension},
};
use orchester_core::id::ResourceId;

use self::{
  editor::ProfileEditor,
  list::{ListSelection, profile_list},
};
use super::{ProjectNav, page};
use crate::{
  activity::{Activity, operations},
  agents::data::{ProfileChange, ProfileEntry, ProjectAgents, apply_in},
  background,
  messages::{self, Message},
  navigation::route::{AgentSelection, Page, Route},
  ui::{button::ActionButton, icon::Icon, message_text, panel::RecoveryPanel},
  workspace::OpenProject,
};

/// The profile a change applied to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Changed(ResourceId);

impl lurq::app::component::DevtoolsInspectable for Changed {}

type ChangeAction = FutureAction<(PathBuf, ProfileChange), Changed, Message>;

/// What the editor edits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Editing {
  New,
  /// Boxed: a profile is much larger than nothing.
  Profile(Box<ProfileEntry>),
}

/// Provided to the editor when it is mounted: the data it starts from and how to save.
#[derive(Clone)]
pub struct EditorContext {
  pub project: OpenProject,
  pub agents: ProjectAgents,
  pub editing: Editing,
  pub change: ChangeAction,
  pub nav: ProjectNav,
}

/// Agents of the project, and the revision they were read at.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Loaded {
  revision: u64,
  agents: ProjectAgents,
}

impl lurq::app::component::DevtoolsInspectable for Loaded {}

pub struct AgentsScreen {
  /// Bumped after a change, to read the profiles again.
  revision: Signal<u64>,
  nav: ProjectNav,
}

impl Component for AgentsScreen {
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
        // The saved profile takes the place of what was edited, a new profile's form included.
        select.replace(Page::Agents(AgentSelection::Profile(id)));
        reload.update(|revision| *revision += 1);
      }
    });
    Self { revision, nav }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let open = ctx.props::<OpenProject>().clone();
    // Futures are matched by call order: the action comes first, as in `create`.
    let change = change_action(ctx);
    let deps = (open.repo_root.clone(), self.revision.get());
    let load = ctx.future(deps, |(repo_root, revision)| async move {
      background::run(move || ProjectAgents::load_in(&repo_root))
        .await?
        .map(|agents| Loaded { revision, agents })
        .map_err(|error| {
          tracing::error!(%error, "cannot read the project's agent profiles");
          messages::store_error(&error)
        })
    });
    let state = load.state().get();
    match (state.data, state.error) {
      (_, Some(error)) => self.page(ctx, &open, None, failed(ctx, &error)),
      (None, None) => self.page(ctx, &open, None, Column::new().into()),
      (Some(loaded), None) => self.loaded(ctx, &open, loaded, change),
    }
  }
}

impl AgentsScreen {
  fn loaded(&self, ctx: &mut Ctx, open: &OpenProject, loaded: Loaded, change: ChangeAction) -> Element {
    let selection = match Route::current_page(ctx) {
      Some(Page::Agents(selection)) => selection,
      _ => AgentSelection::First,
    };
    let editing = match selection {
      AgentSelection::New => Some(Editing::New),
      AgentSelection::Profile(id) => loaded
        .agents
        .profile(id)
        .or_else(|| loaded.agents.profiles.first())
        .cloned()
        .map(|entry| Editing::Profile(Box::new(entry))),
      AgentSelection::First => loaded
        .agents
        .profiles
        .first()
        .cloned()
        .map(|entry| Editing::Profile(Box::new(entry))),
    };
    let Some(editing) = editing else {
      let empty = empty(ctx, &loaded.agents, &self.nav);
      return self.page(ctx, open, Some(&loaded.agents), empty);
    };
    let key = match &editing {
      Editing::New => format!("agent-editor-new-{}", loaded.revision),
      Editing::Profile(entry) => format!("agent-editor-{}-{}", entry.id, loaded.revision),
    };
    ctx.provide(EditorContext {
      project: open.clone(),
      agents: loaded.agents,
      editing,
      change,
      nav: self.nav.clone(),
    });
    ctx.mount_keyed::<ProfileEditor>(&key, ())
  }

  /// The page without an editor: loading, failed, or no profiles yet.
  fn page(&self, ctx: &Ctx, open: &OpenProject, agents: Option<&ProjectAgents>, content: Element) -> Element {
    let breadcrumb = ctx.t_args("agents.breadcrumb", [("project", open.project.name().as_str())]);
    let list = agents.map(|agents| profile_list(ctx, agents, ListSelection::None, &self.nav));
    Column::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(page::topbar(&breadcrumb, []))
      .child(page::heading(&ctx.t("agents.title"), &ctx.t("agents.description")))
      .child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .flex(1.0)
          .with_children(list)
          .child(
            Column::new()
              .flex(1.0)
              .height(Dimension::Pct(100.0))
              .padding(SpacingSize::Section)
              .align_items(Alignment::Center)
              .justify(Justify::Center)
              .child(content),
          ),
      )
      .into()
  }
}

fn failed(ctx: &Ctx, error: &Message) -> Element {
  RecoveryPanel::new(
    Icon::AlertCircle,
    &ctx.t("agents.loadFailed"),
    &message_text(ctx, error),
  )
  .into()
}

/// No profiles yet. A profile needs a connection, so without one the way there is offered.
fn empty(ctx: &Ctx, agents: &ProjectAgents, nav: &ProjectNav) -> Element {
  let nav = nav.clone();
  if agents.connections.is_empty() {
    let action = ActionButton::new(ctx.t("agents.openConnections"))
      .icon(Icon::Plug)
      .id("agents-open-connections")
      .on_click(move || nav.show(Page::CONNECTIONS))
      .render(ctx);
    return RecoveryPanel::new(Icon::Bot, &ctx.t("agents.emptyTitle"), &ctx.t("agents.needsConnection"))
      .action(action)
      .into();
  }
  let action = ActionButton::new(ctx.t("agents.new"))
    .primary()
    .icon(Icon::Plus)
    .id("empty-new-agent")
    .on_click(move || nav.show(Page::Agents(AgentSelection::New)))
    .render(ctx);
  RecoveryPanel::new(Icon::Bot, &ctx.t("agents.emptyTitle"), &ctx.t("agents.emptyDetail"))
    .action(action)
    .into()
}

fn change_action(ctx: &mut Ctx) -> ChangeAction {
  let activity = Activity::of(ctx);
  let open = ctx.props::<OpenProject>().clone();
  ctx.future_action(move |(repo_root, change): (PathBuf, ProfileChange)| {
    let (activity, spec) = (activity.clone(), operations::profile_change(&open, &change));
    async move {
      let root = repo_root.clone();
      let apply = async move {
        let applied = background::run(move || apply_in(&repo_root, change)).await?;
        applied.map(Changed).map_err(|error| {
          tracing::error!(%error, "cannot save the agent profile");
          messages::store_error(&error)
        })
      };
      let target = |changed: &Changed| Some(operations::profile(&root, changed.0));
      activity.track_to(spec, apply, target).await
    }
  })
}
