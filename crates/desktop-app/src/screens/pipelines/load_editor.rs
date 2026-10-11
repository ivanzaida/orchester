//! Reload keeps the live editor until a fresh read succeeds.
use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  core::Signal,
  node::Element,
};
use orchester_core::id::ResourceId;
use orchester_workspace::{background, pipelines::data::OpenedPipeline};

use super::{
  editor::{Editor, EditorProps},
  parts,
};
use crate::{
  project::ShownProject,
  ui::{
    message::message_text,
    state::{load_error, loading},
  },
};

#[derive(Clone, PartialEq)]
pub struct LoadProps {
  pub project: ShownProject,
  pub id: ResourceId,
}
impl DevtoolsInspectable for LoadProps {}
pub struct LoadEditor {
  retry: Signal<u64>,
  accepted: Signal<Option<(u64, OpenedPipeline)>>,
}
impl Component for LoadEditor {
  type Props = LoadProps;
  fn create(ctx: &mut Ctx) -> Self {
    Self {
      retry: ctx.signal(0),
      accepted: ctx.signal(None),
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<LoadProps>().clone();
    let revision = self.retry.get();
    let loaded = ctx
      .future(
        (props.project.open.repo_root.clone(), props.id, revision),
        |(root, id, _)| async move {
          background::run(move || OpenedPipeline::load_in(&root, id).map_err(super::work_error)).await?
        },
      )
      .state()
      .get();
    if !loaded.is_pending()
      && loaded.error.is_none()
      && let Some(opened) = &loaded.data
      && self.accepted.get_untracked().as_ref().map(|(n, _)| *n) != Some(revision)
    {
      self.accepted.set(Some((revision, opened.clone())));
    }
    let mut page = parts::page();
    if let Some((accepted, opened)) = self.accepted.get() {
      page = page.child(ctx.mount_keyed::<Editor>(
        &format!("pipeline-editor-{accepted}"),
        EditorProps {
          project: props.project,
          opened,
          reload: self.retry.clone(),
          loading: loaded.is_pending(),
        },
      ));
    }
    if loaded.is_pending() {
      page = page.child(loading("pipeline-loading", &ctx.t("pipelines.reading")));
    } else if let Some(error) = loaded.error {
      let retry = self.retry.clone();
      page = page.child(load_error(
        ctx,
        "pipeline-load-error",
        (&ctx.t("pipelines.loadFailed"), &message_text(ctx, &error)),
        move || retry.update(|v| *v += 1),
      ));
    }
    page
  }
}
