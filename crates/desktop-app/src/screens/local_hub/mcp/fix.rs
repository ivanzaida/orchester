//! The pinned fix of a local MCP server while nothing is changed (`Actions` in L2 `IpzZ7`, as a profile's panel and a
//! team server's pin theirs): *Set program path*, which asks for the program's file, or *Add a value for …*, which asks
//! for the secret's value. What either takes is a change of the panel's, so the bar is then *Save*'s and *Cancel*'s.

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::sharing::Missing;
use orchester_workspace::readiness::Group;

use super::{super::model::Entry, form::Form, notes, secrets::Taken};
use crate::{
  screens::team_hub::bar,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    pressable::Press,
  },
};

/// The bar of the fix of what `entry` lacks first, when it needs the person and the panel can fix it; `None` otherwise.
pub(super) fn bar(ctx: &Ctx, entry: &Entry, form: &Form, secrets: &[Taken]) -> Option<Element> {
  if entry.group != Group::NeedsYouHere {
    return None;
  }
  let (button, hint) = match notes::first_missing(entry)? {
    Missing::LocalMcpProgram { .. } => {
      let button = ActionButton::new(
        "local-hub-mcp-fix-set-path",
        ButtonKind::Primary,
        ctx.t("local_hub.mcp.fix.setPath"),
      )
      .icon(Icon::FolderOpen);
      let choose = form.choose.clone();
      let button = if choose.state().get().is_pending() {
        button.loading()
      } else {
        button.press(Press::run(move || choose.run(())))
      };
      (button, ctx.t("local_hub.mcp.fix.setPathHint"))
    }
    Missing::LocalSecret { name, .. } => {
      let secret = secrets.iter().find(|secret| &secret.name == name)?.clone();
      let asking = form.asking.clone();
      let button = ActionButton::new(
        "local-hub-mcp-fix-add-value",
        ButtonKind::Primary,
        ctx.t_args("local_hub.mcp.fix.addValue", [("name", name.to_string())]),
      )
      .icon(Icon::KeyRound)
      .press(Press::run(move || asking.ask(&secret)));
      (button, ctx.t("local_hub.mcp.fix.addValueHint"))
    }
    _ => return None,
  };
  Some(bar::bar(vec![button.pinned().into()], Some(hint.to_string())))
}
