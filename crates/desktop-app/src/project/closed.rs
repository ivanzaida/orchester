//! A closed task's waiting completion, followed with the project's feed (ORC-150): whenever the project's tasks were
//! saved (by a screen, automation, the merge queue or the team server's sync) or its runs changed state, each task
//! closed while its latest run waited to be completed has that completion settled
//! (`orchester_workspace::tasks::completion::settle_closed_in`), and the tasks are read again once one was. And how
//! often the shared projects' caches changed, which reads the tasks again too: the desktop's copy of a shared task
//! changes only when the team server's does.

use lurq::app::ctx::{Ctx, StreamEmitter};
use orchester_workspace::{
  background,
  messages::{self, Message},
  sharing::Servers,
  tasks::{completion::settle_closed_in, outside::Outside},
  workspace::OpenProject,
};

use crate::services::Services;

/// Settles the completions closed tasks of `open` wait with, again whenever `changes` moves. Renders nothing.
pub(super) fn settle(ctx: &mut Ctx, (open, changes): (&OpenProject, u64)) {
  let services = Services::of(ctx);
  let open = open.clone();
  ctx.future((open.repo_root.clone(), changes), move |(root, _)| {
    let (services, project) = (services.clone(), open.project.id());
    async move {
      let programs = services.programs.clone();
      let settled = background::run(move || settle_closed_in((&root, &programs)))
        .await?
        .map_err(|error| {
          tracing::error!(%error, "cannot settle the completions of the project's closed tasks");
          messages::store_error(&error)
        })?;
      // Their Summary, every Needs you and the list read them again.
      if settled {
        services.changes.record(project, Outside::Tasks);
      }
      Ok::<_, Message>(settled)
    }
  });
}

/// How often the shared projects' caches changed; `servers` wakes the stream for every shared project.
pub(super) fn shared_changes(ctx: &mut Ctx, servers: &Servers) -> u64 {
  let count = ctx.stream(servers.clone(), |servers, emitter: StreamEmitter<u64, ()>| async move {
    let mut updates = servers.subscribe_shared();
    loop {
      let revision = *updates.borrow_and_update();
      if !emitter.emit(revision) || updates.changed().await.is_err() {
        return;
      }
    }
  });
  count.state().get().data.unwrap_or(0)
}
