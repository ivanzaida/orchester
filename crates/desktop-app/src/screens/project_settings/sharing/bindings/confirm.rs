//! The person-only confirmation that binds an MCP server (`04d` `z0iFy`) or a secret (`04e` `LRXYn`): a modal with the
//! one chosen here, which the person may change in it, what binding it lets agents or tools do, that nothing about it
//! is sent, and *Only you can confirm this; the Orchestrator and tools can't.* Its action binds it only for a person's
//! input; synthetic input (an MCP client's) is refused and the dialog says so.

use lurq::{app::ctx::Ctx, components::SelectOption, node::Element};
use orchester_store::{Bound, BoundBy, BoundReference};
use orchester_workspace::sharing::sync::Here;

use super::{Ask, rows::Actions};
use crate::ui::{
  button::{ActionButton, ButtonKind},
  dialog::{Dialog, choice, line, lines},
  field::SelectField,
  icon::Icon,
};

/// The action, which only a person presses.
const CONFIRM: &str = "sharing-bind-confirm";

/// The modal asking about `ask`.
pub(super) fn dialog(ctx: &Ctx, here: &Here, ask: &Ask, actions: &Actions) -> Element {
  let chosen = actions.chosen.get();
  let (dialog, options) = match ask.reference {
    BoundReference::McpServer { .. } => {
      let options: Vec<(String, String)> = here
        .mcp_servers
        .iter()
        .map(|server| (server.id().to_string(), server.name().as_str().to_owned()))
        .collect();
      let local = label_of(&options, &chosen);
      (server(ctx, ask, &local), options)
    }
    BoundReference::Secret { .. } => {
      let options: Vec<(String, String)> = here
        .secrets_of(here_role(here, ask).as_deref())
        .into_iter()
        .map(|secret| (secret.id().to_string(), secret.name().as_str().to_owned()))
        .collect();
      (secret(ctx, ask), options)
    }
    // A connection is bound from its picker at once; it is never asked about.
    BoundReference::Connection { .. } => return Element::new(),
  };
  let label = ctx.t(match ask.reference {
    BoundReference::Secret { .. } => "sharing.bindings.secretHere",
    _ => "sharing.bindings.serverHere",
  });
  let select = SelectField::new(
    "sharing-bind-choice",
    label.clone(),
    actions.chosen.clone(),
    options
      .iter()
      .map(|(id, name)| SelectOption::new(id.clone(), name.clone()))
      .collect(),
  )
  .render(ctx);
  let action = action(ctx, ask, actions);
  dialog
    .part(choice("sharing-bind-choice-field", &label, select))
    .person_only(ctx.t("sharing.bindings.personOnly"))
    .action(action)
    .under(actions.person.reason(ctx, CONFIRM))
    .render(ctx, &actions.open)
}

/// *Use “GitHub (remote)” for github on this computer?* and what it lets agents do.
fn server(ctx: &Ctx, ask: &Ask, local: &str) -> Dialog {
  let names = [
    ("name", ask.name.clone()),
    ("local", local.to_owned()),
    ("needs", ask.needs.clone()),
  ];
  let what = lines(
    "sharing-bind-does",
    [
      line(
        "sharing-bind-agents",
        Icon::Bot,
        &ctx.t_args("sharing.bindings.server.agents", names.clone()),
      ),
      line(
        "sharing-bind-sign-in",
        Icon::KeyRound,
        &ctx.t("sharing.bindings.server.signIn"),
      ),
      line("sharing-bind-later", Icon::Undo2, &ctx.t("sharing.bindings.later")),
    ],
  );
  Dialog::new(
    "sharing-bind-server",
    Icon::Server,
    ctx.t_args("sharing.bindings.server.title", names),
    ctx.t("sharing.bindings.cancel"),
  )
  .part(what)
}

/// *Give a secret to SENTRY_TOKEN on this computer?* and what it lets tools do.
fn secret(ctx: &Ctx, ask: &Ask) -> Dialog {
  let names = [("name", ask.name.clone()), ("needs", ask.needs.clone())];
  let what = lines(
    "sharing-bind-does",
    [
      line(
        "sharing-bind-sends",
        Icon::KeyRound,
        &ctx.t_args("sharing.bindings.secret.sends", names.clone()),
      ),
      line(
        "sharing-bind-stays",
        Icon::Monitor,
        &ctx.t_args("sharing.bindings.secret.stays", names.clone()),
      ),
      line("sharing-bind-later", Icon::Undo2, &ctx.t("sharing.bindings.later")),
    ],
  );
  Dialog::new(
    "sharing-bind-secret",
    Icon::KeyRound,
    ctx.t_args("sharing.bindings.secret.title", names),
    ctx.t("sharing.bindings.cancel"),
  )
  .part(what)
}

/// *Use for github* or *Use this secret*: binds the chosen one, once a person pressed it.
fn action(ctx: &Ctx, ask: &Ask, actions: &Actions) -> ActionButton {
  let label = match ask.reference {
    BoundReference::Secret { .. } => ctx.t("sharing.bindings.secret.use"),
    _ => ctx.t_args("sharing.bindings.server.use", [("name", ask.name.as_str())]),
  };
  let button = ActionButton::new(CONFIRM, ButtonKind::Primary, label).icon(Icon::Check);
  if actions.busy {
    return button.loading();
  }
  if actions.chosen.get().is_empty() {
    return button.disabled();
  }
  let (actions, reference) = (actions.clone(), ask.reference);
  button.on_click(move || {
    if !actions.person.allows(CONFIRM) {
      return;
    }
    let Some(choice) = chosen(reference, &actions.chosen.get_untracked()) else {
      return;
    };
    actions.open.set(false);
    actions.bind.run(choice);
  })
}

/// `reference` bound by the person to the local one whose id is `local`.
fn chosen(reference: BoundReference, local: &str) -> Option<BoundReference> {
  match reference {
    BoundReference::McpServer { reference, .. } => Some(BoundReference::McpServer {
      reference,
      bound: Some(Bound {
        local: local.parse().ok()?,
        by: BoundBy::Person,
      }),
    }),
    BoundReference::Secret { reference, .. } => Some(BoundReference::Secret {
      reference,
      bound: Some(Bound {
        local: local.parse().ok()?,
        by: BoundBy::Person,
      }),
    }),
    BoundReference::Connection { .. } => None,
  }
}

/// The role, the secret's kind, of the reference `ask` is about.
fn here_role(here: &Here, ask: &Ask) -> Option<String> {
  here
    .references
    .iter()
    .find(|reference| reference.binding.reference == ask.reference)
    .and_then(|reference| reference.binding.role.clone())
}

/// The name of the option whose id is `chosen`; empty while none is.
fn label_of(options: &[(String, String)], chosen: &str) -> String {
  options
    .iter()
    .find(|(id, _)| id == chosen)
    .map(|(_, name)| name.clone())
    .unwrap_or_default()
}
