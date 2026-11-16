//! An admin's *Publish again by name* of a team tool whose latest version names its secret by the id it had on the
//! publisher's computer: the version goes to the team again in the team's form, with the secret by the name it has in
//! this project ([`team_body_edit::save_tool`]). A project that has no secret of that id cannot say its name, and the
//! refusal says so in place. The design has no frame of its own for it.

use std::path::Path;

use lurq::app::{
  component::DevtoolsInspectable,
  ctx::{Ctx, FutureAction},
};
use orchester_core::{
  id::ResourceId,
  resource::{Tool, VersionNumber},
};
use orchester_workspace::{
  agents::{team_body_edit, team_body_edit::Latest},
  messages::Message,
};

use crate::{
  screens::team_hub::bar,
  services::Services,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    pressable::Press,
  },
};

/// A press: the version the tool is published again on, and the tool it holds.
#[derive(Clone, Debug, PartialEq)]
struct Again {
  base: VersionNumber,
  tool: Tool,
}
impl DevtoolsInspectable for Again {}

pub(super) struct Republish {
  action: FutureAction<Again, (), Message>,
}

impl Republish {
  pub(super) fn new(ctx: &mut Ctx, root: &Path, id: ResourceId) -> Self {
    let servers = Services::of(ctx).servers;
    let root = root.to_path_buf();
    let action = ctx.future_action(move |again: Again| {
      let (servers, root) = (servers.clone(), root.clone());
      async move {
        team_body_edit::save_tool(&servers, &root, (id, again.base), &again.tool)
          .await
          .map(|_| ())
          .map_err(|refused| refused.message())
      }
    });
    Self { action }
  }

  /// The pinned bar that publishes `latest` again by name; unavailable while the team server cannot be reached
  /// (`unreached`, which the hint says).
  pub(super) fn bar(&self, ctx: &Ctx, latest: &Latest<Tool>, unreached: Option<&Message>) -> lurq::node::Element {
    let button = ActionButton::new(
      "team-hub-tool-republish",
      ButtonKind::Primary,
      ctx.t("team_hub.tool.republish.button"),
    )
    .icon(Icon::Upload)
    .pinned();
    let (button, hint) = if self.action.state().get().is_pending() {
      (button.loading(), ctx.t("team_hub.tool.republish.hint"))
    } else if unreached.is_some() {
      (button.disabled(), ctx.t("team_hub.tool.republish.unreached"))
    } else {
      let (action, latest) = (self.action.clone(), latest.clone());
      (
        button.press(Press::run(move || {
          action.run(Again {
            base: latest.number,
            tool: latest.body.clone(),
          });
        })),
        ctx.t("team_hub.tool.republish.hint"),
      )
    };
    bar::bar(vec![button.into()], Some(hint.to_string()))
  }

  /// Why the team or this computer refused the last press.
  pub(super) fn refused(&self) -> Option<Message> {
    let state = self.action.state().get();
    state.error.clone().filter(|_| !state.is_pending())
  }
}
