//! Project agent profiles, layer Agents Gg0hY.
mod actions;
mod active;
mod editor;
mod enforcement;
mod entries;
mod fields;
mod list;
mod panels;
mod permissions;
mod policy_panel;
mod uses;
mod words;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::Column,
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::id::ResourceId;
use orchester_workspace::{agents::data::ProjectAgents, background, messages, tasks::outside::Outside};

use crate::{
  project::ShownProject,
  ui::{
    message::message_text,
    state::{load_error, loading},
  },
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum View {
  List,
  New,
  Edit(ResourceId),
}
impl DevtoolsInspectable for View {}

#[derive(Clone, PartialEq)]
pub struct Props {
  pub project: ShownProject,
  pub selected: Option<ResourceId>,
}
impl DevtoolsInspectable for Props {}

pub struct AgentsScreen {
  view: Signal<View>,
  revision: Signal<u64>,
  retry: Signal<u64>,
}
pub fn screen(ctx: &mut Ctx, project: &ShownProject, selected: Option<ResourceId>) -> Element {
  ctx.mount_keyed::<AgentsScreen>(
    &format!("agents-{}-{selected:?}", project.open.project.id()),
    Props {
      project: project.clone(),
      selected,
    },
  )
}

impl Component for AgentsScreen {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    Self {
      view: ctx.signal(ctx.props::<Props>().selected.map_or(View::List, View::Edit)),
      revision: ctx.signal(0),
      retry: ctx.signal(0),
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let project = ctx.props::<Props>().project.clone();
    let revision = self.revision.get();
    let outside = project.feed.outside.get();
    let dep = (
      revision,
      self.retry.get(),
      outside.of(Outside::Connections),
      outside.of(Outside::Pipelines),
      outside.of(Outside::Profiles),
      // The profiles' active uses: the runs that run them start and end.
      project.feed.states.get(),
    );
    let hold = crate::services::Services::of(ctx).hold;
    let loaded = ctx
      .future((project.open.repo_root.clone(), dep), move |(root, (revision, ..))| {
        tracing::trace!(target: crate::app_log::READ_LOG, read = "agents");
        let hold = hold.clone();
        async move {
          hold.released().await;
          background::run(move || ProjectAgents::for_screen(&root))
            .await?
            .map(|listed| (revision, listed))
            .map_err(|error| messages::store_error(&error))
        }
      })
      .state()
      .get();
    let frame = Column::new()
      .id("screen-agents")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0));
    if loaded.is_pending() && loaded.data.as_ref().is_none_or(|(saved, _)| *saved != revision) {
      return frame.child(loading("agents-loading", &ctx.t("agents.reading")));
    }
    let error = loaded.error.clone().filter(|_| loaded.is_rejected());
    if let Some(error) = &error
      && (loaded.data.is_none() || self.view.get() == View::List)
    {
      let retry = self.retry.clone();
      return frame.child(load_error(
        ctx,
        "agents-load-error",
        (&ctx.t("agents.loadFailed"), &message_text(ctx, error)),
        move || retry.update(|n| *n += 1),
      ));
    }
    let reading = loaded.is_pending();
    let Some((_, listed)) = loaded.data else {
      return frame.child(loading("agents-loading", &ctx.t("agents.reading")));
    };
    frame.child(self.content(ctx, project, listed, error, reading))
  }
}
impl AgentsScreen {
  fn content(
    &self,
    ctx: &mut Ctx,
    project: ShownProject,
    listed: ProjectAgents,
    load_error: Option<messages::Message>,
    reading: bool,
  ) -> Element {
    let revision = self.revision.get();
    match self.view.get() {
      View::List => list::list(ctx, &project, &listed, self.view.clone()),
      view => {
        let entry = match view {
          View::Edit(id) => listed.profile(id).cloned(),
          _ => None,
        };
        if matches!(view, View::Edit(_)) && entry.is_none() {
          return lurq::components::Text::new(&ctx.t("agents.gone")).into();
        }
        let key = format!("agent-editor-{:?}-{revision}", entry.as_ref().map(|e| e.id));
        ctx.mount_keyed::<editor::Editor>(
          &key,
          editor::Props {
            project,
            listed,
            entry,
            view: self.view.clone(),
            revision: self.revision.clone(),
            retry: self.retry.clone(),
            load_error,
            reading,
          },
        )
      }
    }
  }
}
