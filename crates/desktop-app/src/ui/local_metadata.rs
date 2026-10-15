//! Scope the shell's path/name fields to the ORC-24 known-secret projection while its page is active.
use std::path::PathBuf;

use lurq::app::{component::DevtoolsInspectable, ctx::Ctx};
use orchester_workspace::background;

use crate::{
  routes::{ProjectPage, Route, TaskView},
  services::Services,
};

#[derive(Clone, PartialEq, Eq)]
struct Request {
  active: bool,
  root: Option<PathBuf>,
  values: Vec<String>,
  revision: u64,
}
impl DevtoolsInspectable for Request {}

pub(crate) fn allowed(ctx: &mut Ctx, root: Option<PathBuf>, values: Vec<String>, revision: u64) -> bool {
  let active = matches!(
    Route::parse(&ctx.route_path()),
    Ok(Route::Project(_, ProjectPage::Task(_, TaskView::LocalIntegration)))
  );
  let vault = ctx.use_context::<Services>().map(|services| services.vault);
  let request = Request {
    active,
    root,
    values,
    revision,
  };
  // Register a fixed-order opaque future on every render, including outside this feature.
  let state = ctx
    .future(request, move |request| {
      let vault = vault.clone();
      async move {
        if !request.active {
          return Ok::<_, ()>(true);
        }
        let Some(vault) = vault else {
          return Ok(false);
        };
        let Some(root) = request.root else {
          return Ok(false);
        };
        background::run(move || {
          orchester_workspace::tasks::local_integration::metadata_allowed(&root, &request.values, vault.get())
        })
        .await
        .map_err(|_| ())?
        .map_err(|_| ())
      }
    })
    .state()
    .get();
  !active || (!state.is_pending() && state.error.is_none() && state.data == Some(true))
}
