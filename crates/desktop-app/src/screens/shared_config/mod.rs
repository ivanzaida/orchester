//! What a shared project's configuration needs of this computer, as the screens show it
//! (`docs/design/shared-configuration.md`, decisions 7, 8 and 11; `Shared configuration · redesign` `UC5xk`): the
//! picker of the person's own connections with *Add an account…* ([`picker`]), the words that name who needs a
//! reference ([`needs`]), and binding one, which only the person does ([`bind_action`]). Sharing › On this computer
//! and a profile's *Runs here use* build on them.

pub(crate) mod picker;

use std::path::PathBuf;

use lurq::{
  app::ctx::{Ctx, FutureAction},
  core::Signal,
};
use orchester_core::{resource::ResourceKind, run::Answerer};
use orchester_store::BoundReference;
use orchester_workspace::{
  background,
  sharing::sync::{self, Needer},
};

use crate::screens::settings::orchester::{Said, sharing_said};

/// Who needs a reference, `Builder v4, Planner v2` or `Tool “Report crash”`; *named by no version now* for none.
pub(crate) fn needs(ctx: &Ctx, needers: &[Needer]) -> String {
  if needers.is_empty() {
    return ctx.t("sharing.bindings.nobody").to_string();
  }
  let separator = ctx.t("sharing.bindings.separator");
  needers
    .iter()
    .map(|needer| {
      let name = ("name", needer.name.clone());
      match needer.kind {
        ResourceKind::Tool => ctx.t_args("sharing.bindings.tool", [name]),
        _ => ctx.t_args(
          "sharing.bindings.version",
          [name, ("version", needer.number.get().to_string())],
        ),
      }
      .to_string()
    })
    .collect::<Vec<_>>()
    .join(&separator)
}

/// Binds a reference as the person chose; `done` counts each binding made, for the screens that read them. The
/// caller checks in its handler that a person pressed it ([`crate::screens::settings::orchester::PersonOnly`]): the
/// workspace takes it as the person's.
pub(crate) fn bind_action(ctx: &mut Ctx, root: PathBuf, done: Signal<u64>) -> FutureAction<BoundReference, (), Said> {
  ctx.future_action(move |choice: BoundReference| {
    let (root, done) = (root.clone(), done.clone());
    async move {
      background::run(move || sync::bind(&root, choice, Answerer::Person))
        .await
        .map_err(|_| Said::new("cloud.errors.local"))?
        .map_err(|error| sharing_said(&error))?;
      done.update(|count| *count += 1);
      Ok(())
    }
  })
}
