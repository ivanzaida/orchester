//! *Enforced by Orchester* on a native connection's page (`docs/architecture/native-agent.md` §5.4, the old design's
//! 04.H8): each permission option a profile can choose, and how Orchester enforces it for this connection's agents on
//! this computer, or why it cannot yet. The rows follow core's capability table, so they change with it. A listed
//! command is chosen by Orchester but not confined, which the group says under its rows.

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::{
  connection::{AgentProvider, invocation::Platform},
  permission::capability::{PermissionOption, Support, Unavailable, support},
};

use crate::screens::project_settings::kit::{Group, SettingRow, Tone, form_note, value};

/// Each option with its name and how Orchester enforces it when it does.
const OPTIONS: [(PermissionOption, &str, &str); 7] = [
  (
    PermissionOption::ReadOnly,
    "connections.native.enforced.readOnly",
    "connections.native.enforced.readOnlyHow",
  ),
  (
    PermissionOption::EditWorktree,
    "connections.native.enforced.editWorktree",
    "connections.native.enforced.editWorktreeHow",
  ),
  (
    PermissionOption::AskBeforeCommands,
    "connections.native.enforced.askBefore",
    "connections.native.enforced.askBeforeHow",
  ),
  (
    PermissionOption::AllowedCommands,
    "connections.native.enforced.allowed",
    "connections.native.enforced.allowedHow",
  ),
  (
    PermissionOption::AnyCommand,
    "connections.native.enforced.anyCommand",
    "connections.native.enforced.anyCommandHow",
  ),
  (
    PermissionOption::NetworkOff,
    "connections.native.enforced.networkOff",
    "connections.native.enforced.networkOffHow",
  ),
  (
    PermissionOption::NetworkOn,
    "connections.native.enforced.networkOn",
    "connections.native.enforced.networkOnHow",
  ),
];

/// The group of `provider`'s options on this platform.
pub(super) fn group(ctx: &mut Ctx, provider: AgentProvider) -> Element {
  let platform = Platform::current();
  let rows: Vec<Element> = OPTIONS
    .into_iter()
    .enumerate()
    .map(|(index, (option, name, how))| {
      let (hint, tone, state) = match support(provider, platform, option) {
        Support::Enforced(_) => (ctx.t(how), Tone::Muted, "connections.native.enforced.enforced"),
        Support::Unavailable(why) => (
          ctx.t(unavailable_key(why)),
          Tone::Warning,
          "connections.native.enforced.locked",
        ),
      };
      SettingRow::new(format!("connection-enforced-{index}"), ctx.t(name))
        .toned(hint, tone)
        .control(value(&format!("connection-enforced-{index}-state"), &ctx.t(state)))
        .render(ctx)
    })
    .collect();
  Group::new("connection-enforced", ctx.t("connections.native.enforced.title"))
    .rows(rows)
    .after(form_note(
      "connection-enforced-note",
      &ctx.t("connections.native.enforced.note"),
    ))
    .into()
}

const fn unavailable_key(why: Unavailable) -> &'static str {
  match why {
    Unavailable::CannotLimitCommands => "connections.native.enforced.cannotLimit",
    Unavailable::CannotConfineCommands => "connections.native.enforced.cannotConfine",
    Unavailable::CannotBlockNetwork => "connections.native.enforced.cannotBlockNetwork",
  }
}
