//! The Add decision model route, loading project secrets before mounting the form.
use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::Text,
  core::Signal,
  node::Element,
};

use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{ConnectionsView, ProjectPage, Route},
  screens::project_settings::kit,
  ui::{message::message_text, state::load_error},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Server {
  TypeSafe,
  Laya,
  Local,
  Other,
}
impl DevtoolsInspectable for Server {}

#[derive(Clone, PartialEq)]
pub(super) struct Props {
  pub project: ShownProject,
}
impl DevtoolsInspectable for Props {}

pub(super) struct AddDecision {
  open: Signal<bool>,
}
impl Component for AddDecision {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    let open = ctx.signal(true);
    let navigation = Navigation::of(ctx);
    let project = ctx.props::<Props>().project.open.project.id();
    ctx.watch(&open, move |open| {
      if !open {
        navigation.replace(Route::Project(
          project,
          ProjectPage::Connections(ConnectionsView::default()),
        ));
      }
    });
    Self { open }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let project = ctx.props::<Props>().project.clone();
    let query = super::data::load(ctx, project.open.repo_root.clone());
    let read = crate::queries::state(&query);
    if let Some(error) = read.error {
      let error = load_error(
        ctx,
        "decision-add-load-error",
        (&ctx.t("connections.loadFailed"), &message_text(ctx, &error)),
        move || query.refresh(),
      );
      return kit::scroll(error);
    }
    let Some(listed) = read.data else {
      return kit::scroll(Text::new(&ctx.t("connections.reading")));
    };
    ctx.mount_keyed::<super::decision::editor::DecisionEditor>(
      "decision-add-editor",
      super::decision::editor::Props {
        project,
        connection: None,
        listed,
        close: self.open.clone(),
      },
    )
  }
}
