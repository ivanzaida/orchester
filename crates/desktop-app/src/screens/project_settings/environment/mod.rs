//! Environment (`S06 · Project settings · Environment` `EnL2D`, the Secrets tab it was): Variables, the project's
//! variables with their editor ([`variables`], from Runs), over Secrets, the secrets by environment
//! (`screens::secrets`), 28 apart. A secret's page and New secret are pages under the tab, without the variables.
mod variables;

use lurq::{
  app::ctx::Ctx,
  components::Column,
  node::{Element, dimension::Dimension},
};

use super::{Status, TabProps, page};
use crate::{
  routes::{SecretsPanel, SecretsView},
  screens::secrets,
  theme::ExtraSpacing,
};

/// The tab showing `view`: its variables and the secrets' list, or a page of a secret.
pub(super) fn tab(ctx: &mut Ctx, props: TabProps, view: SecretsView) -> Element {
  let shown = secrets::screen(ctx, &props.project, view);
  if is_page(view) {
    // The variables' editor is gone with its edit, so the status it said goes too.
    if props.status.get_untracked() != Status::Saved {
      props.status.set(Status::Saved);
    }
    return page::scroll(shown);
  }
  let variables = ctx.mount_keyed::<variables::Variables>("project-settings-environment", props);
  page::scroll(
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Roomy.role())
      .child(variables)
      .child(shown),
  )
}

/// Whether `view` is a page under the tab (a secret's or New secret), which shows no variables and no save status.
pub(super) const fn is_page(view: SecretsView) -> bool {
  view.secret.is_some() || matches!(view.panel, SecretsPanel::Add)
}
