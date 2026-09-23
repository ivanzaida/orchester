//! The most recently used project cannot be opened (moved, deleted, damaged or newer). No design
//! exists for this state yet, so it is a recovery panel (`iN90G`) with what is known.

use std::path::Path;

use lurq::{
  app::{ctx::Ctx, theme::TypographyStyle},
  components::Text,
  node::Element,
};

use crate::{
  messages::Message,
  navigation::{Navigation, Screen},
  root::centered,
  ui::{button::ActionButton, icon::Icon, message_text, panel::RecoveryPanel},
};

pub fn render(ctx: &Ctx, navigation: &Navigation, name: &str, repo_root: &Path, reason: &Message) -> Element {
  let navigation = navigation.clone();
  let setup = ActionButton::new(ctx.t("recovery.projectUnavailable.setUpNew"))
    .icon(Icon::Plus)
    .id("set-up-new-project")
    .on_click(move || navigation.show(Screen::Setup))
    .render(ctx);
  let panel = RecoveryPanel::new(
    Icon::AlertCircle,
    &ctx.t_args("recovery.projectUnavailable.title", [("name", name)]),
    &message_text(ctx, reason),
  )
  .detail(
    Text::new(&repo_root.display().to_string())
      .variant(TypographyStyle::Caption)
      .selectable(true),
  )
  .action(setup);
  centered(panel.into())
}
