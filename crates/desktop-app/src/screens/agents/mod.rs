//! Project agent profiles, layer Agents Gg0hY.
mod actions;
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
}
impl DevtoolsInspectable for Props {}

pub struct AgentsScreen {
  view: Signal<View>,
  revision: Signal<u64>,
}
pub fn screen(ctx: &mut Ctx, project: &ShownProject) -> Element {
  ctx.mount_keyed::<AgentsScreen>(
    &format!("agents-{}", project.open.project.id()),
    Props {
      project: project.clone(),
    },
  )
}

impl Component for AgentsScreen {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    Self {
      view: ctx.signal(View::List),
      revision: ctx.signal(0),
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let project = ctx.props::<Props>().project.clone();
    let revision = self.revision.get();
    let outside = project.feed.outside.get();
    let dep = (
      revision,
      outside.of(Outside::Connections),
      outside.of(Outside::Pipelines),
      outside.of(Outside::Profiles),
      project.feed.changes.get(),
    );
    let hold = crate::services::Services::of(ctx).hold;
    let loaded = ctx
      .future((project.open.repo_root.clone(), dep), move |(root, (revision, ..))| {
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
    let Some((_, listed)) = loaded.data else {
      let state = match loaded.error.clone().filter(|_| loaded.is_rejected()) {
        Some(error) => {
          let revision = self.revision.clone();
          load_error(
            ctx,
            "agents-load-error",
            (&ctx.t("agents.loadFailed"), &message_text(ctx, &error)),
            move || revision.update(|n| *n += 1),
          )
        }
        None => loading("agents-loading", &ctx.t("agents.reading")).into(),
      };
      return frame.child(state);
    };
    let content = match self.view.get() {
      View::List => list::list(ctx, &project, &listed, self.view.clone()),
      view => {
        let entry = match view {
          View::Edit(id) => listed.profile(id).cloned(),
          _ => None,
        };
        if matches!(view, View::Edit(_)) && entry.is_none() {
          return frame.child(lurq::components::Text::new(&ctx.t("agents.gone")));
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
          },
        )
      }
    };
    frame.child(content)
  }
}
