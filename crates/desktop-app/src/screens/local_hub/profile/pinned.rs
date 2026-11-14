//! The pinned action of a local profile while nothing is changed (`Actions` `o8tFHR`, as a team profile's panel has
//! it): the fix of the first missing part of *Ready?* that has one (*Use Claude Code · Work for Builder*, *Add a Claude
//! Code connection*, …) with what it asks of the person at its end. Once the panel holds a change the bar is Save's and
//! Cancel's ([`super::super::save_bar`]).

use lurq::{app::ctx::Ctx, node::Element};
use orchester_workspace::agents::local_hub::LocalProfile;

use super::ready::Fix;
use crate::{
  screens::team_hub::{
    bar,
    profile::{
      fix::Offer,
      parts::{Part, State},
    },
    words::cli_key,
  },
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    pressable::Press,
  },
};

/// The first fix `parts` offer, with the part it fixes.
pub(super) fn first(parts: &[Part<Fix>]) -> Option<Fix> {
  parts.iter().find_map(|part| match &part.state {
    State::Missing { fix: Some(fix), .. } => Some(fix.clone()),
    State::Missing { fix: None, .. } | State::Ready(_) => None,
  })
}

/// The bar of `fix` of the profile `shown`, named `name`, which `press` does; `connection` names the account *Use*
/// keeps (*Claude Code · Default*), and `all` is *Use for all* with how many profiles it sets. Off while a save is
/// under way.
pub(super) fn bar(
  ctx: &Ctx,
  (name, shown, connection): (&str, &LocalProfile, &str),
  (fix, press, all): (&Fix, Press, Option<bar::UseAll>),
  busy: bool,
) -> Element {
  let cli = shown.cli.map(|cli| ctx.t(cli_key(cli)).to_string()).unwrap_or_default();
  let server = |id| {
    shown
      .mcp_servers
      .iter()
      .find_map(|(found, server)| (*found == id).then(|| server.clone()))
      .unwrap_or_default()
  };
  let (words, glyph, hint) = match fix {
    Fix::UseConnection => (
      ctx.t_args(
        "team_hub.fix.action.use",
        [("connection", connection), ("profile", name)],
      ),
      Icon::Check,
      ctx.t("team_hub.fix.hint.use"),
    ),
    Fix::AddConnection => (
      ctx.t_args("team_hub.fix.action.add", [("cli", &cli)]),
      Icon::Plus,
      ctx.t("team_hub.fix.hint.add"),
    ),
    Fix::SignIn => (
      ctx.t_args("team_hub.fix.action.signIn", [("cli", &cli)]),
      Icon::LogIn,
      ctx.t("team_hub.fix.hint.signIn"),
    ),
    Fix::SetPath(id) => (
      ctx.t_args("team_hub.fix.action.setPath", [("server", server(*id))]),
      Icon::FolderOpen,
      ctx.t_args("team_hub.fix.hint.done", [("profile", name)]),
    ),
    Fix::AddValue(id) => (
      ctx.t_args("team_hub.fix.action.addValue", [("name", server(*id))]),
      Icon::KeyRound,
      ctx.t_args("team_hub.fix.hint.done", [("profile", name)]),
    ),
  };
  let button = ActionButton::new(format!("local-hub-fix-{}", fix.name()), ButtonKind::Primary, words)
    .icon(glyph)
    .pinned();
  let button = if busy { button.disabled() } else { button.press(press) };
  let hint = match &all {
    Some((count, _)) => bar::use_all_hint(ctx, *count, &cli),
    None => hint.to_string(),
  };
  let all = all.map(|(count, press)| {
    let button = bar::use_all(ctx, "local-hub", count);
    if busy { button.disabled() } else { button.press(press) }.into()
  });
  bar::bar([button.into()].into_iter().chain(all).collect(), Some(hint))
}
