//! Asynchronous saves, always carrying the exact reviewed body. A pipeline holds no draft (D-032): a save publishes the
//! next version, or is refused and stores nothing. A team pipeline's save waits for the team server's answer and is
//! stored only once the server took it (D-043); while it waits the action is pending, which the page shows as saving.
use std::path::PathBuf;

use lurq::app::ctx::{Ctx, FutureAction};
use orchester_core::resource::Pipeline;
use orchester_workspace::{
  background,
  messages::Message,
  pipelines::{
    data::OpenedPipeline,
    write::{self, Allowed, Stored},
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
pub type Action = FutureAction<Work, Stored, Message>;
pub fn action(ctx: &mut Ctx, project: orchester_core::id::ProjectId) -> Action {
  let services = crate::services::Services::of(ctx);
  ctx.future_action(move |work: Work| {
    let services = services.clone();
    async move {
      let (changes, servers) = (services.changes.clone(), services.servers.clone());
      let Work::Save {
        root,
        before,
        body,
        name,
        allowed,
      } = work;
      background::run(move || {
        let result =
          write::save_named_in(&root, &servers, &before, &body, &name, &allowed).map_err(|error| error.message());
        if result.is_ok() {
          use orchester_workspace::tasks::outside::Outside;
          changes.record(project, Outside::Pipelines);
          changes.record(project, Outside::Tasks);
        }
        result
      })
      .await?
    }
  })
}
