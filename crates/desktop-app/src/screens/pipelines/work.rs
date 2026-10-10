//! Asynchronous saves and publishes, always carrying the exact reviewed body.
use std::path::PathBuf;

use lurq::app::ctx::{Ctx, FutureAction};
use orchester_core::resource::Pipeline;
use orchester_workspace::{
  background,
  messages::Message,
  pipelines::{
    data::OpenedPipeline,
    write::{self, Allowed},
  },
};

#[derive(Clone)]
pub enum Work {
  Save {
    root: PathBuf,
    before: OpenedPipeline,
    body: Pipeline,
    allowed: Allowed,
  },
  Publish {
    root: PathBuf,
    before: OpenedPipeline,
    allowed: Allowed,
  },
}
pub type Action = FutureAction<Work, OpenedPipeline, Message>;
pub fn action(ctx: &mut Ctx, project: orchester_core::id::ProjectId) -> Action {
  let changes = crate::services::Services::of(ctx).changes;
  ctx.future_action(move |work: Work| {
    let changes = changes.clone();
    async move {
      background::run(move || {
        let publishing = matches!(work, Work::Publish { .. });
        let result = match work {
          Work::Save {
            root,
            before,
            body,
            allowed,
          } => write::save_in(&root, &before, body, &allowed),
          Work::Publish { root, before, allowed } => write::publish_in(&root, before.id, &before.body, &allowed),
        }
        .map_err(|error| error.message());
        if result.is_ok() {
          use orchester_workspace::tasks::outside::Outside;
          changes.record(project, Outside::Pipelines);
          if publishing {
            changes.record(project, Outside::Tasks);
          }
        }
        result
      })
      .await?
    }
  })
}
