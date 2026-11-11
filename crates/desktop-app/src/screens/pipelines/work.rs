//! Asynchronous saves, always carrying the exact reviewed body. A pipeline holds no draft (D-032): a save publishes the
//! next version, or is refused and stores nothing.
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
    name: orchester_core::name::Name,
    allowed: Allowed,
  },
}
pub type Action = FutureAction<Work, OpenedPipeline, Message>;
pub fn action(ctx: &mut Ctx, project: orchester_core::id::ProjectId) -> Action {
  let services = crate::services::Services::of(ctx);
  ctx.future_action(move |work: Work| {
    let services = services.clone();
    async move {
      let changes = services.changes.clone();
      let Work::Save {
        root,
        before,
        body,
        name,
        allowed,
      } = work;
      // A version published in a shared project goes to its team server at once (02).
      let published = root.clone();
      let result = background::run(move || {
        let result = write::save_named_in(&root, &before, &body, &name, &allowed).map_err(|error| error.message());
        if result.is_ok() {
          use orchester_workspace::tasks::outside::Outside;
          changes.record(project, Outside::Pipelines);
          changes.record(project, Outside::Tasks);
        }
        result
      })
      .await?;
      if result.is_ok() {
        crate::screens::hub::team::send_published(&services, &published).await;
      }
      result
    }
  })
}
