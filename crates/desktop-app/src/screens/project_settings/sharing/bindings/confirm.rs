//! The person-only confirmation that binds an MCP server (`04d` `z0iFy`) or a secret (`04e` `LRXYn`): a modal with,
//! under its title, the one chosen here with where it is, which the person may change in it; then what binding it lets
//! agents or tools do, that nothing about it is sent, and *Only you can confirm this; the Orchestrator and tools
//! can't.* Its action binds it only for a person's input; synthetic input (an MCP client's) is refused and the dialog
//! says so.

use lurq::{
  app::ctx::Ctx,
  components::{Column, SelectOption},
  node::Element,
};
use orchester_store::{Bound, BoundBy, BoundReference};
use orchester_workspace::sharing::sync::Here;

use super::{Ask, rows::Actions};
use crate::{
  screens::shared_config::server_detail,
  ui::{
    button::{ActionButton, ButtonKind},
    dialog::{Dialog, choice, line, lines},
    field::SelectField,
    icon::Icon,
  },
};

/// The action, which only a person presses.
const CONFIRM: &str = "sharing-bind-confirm";

/// One the modal offers: its id, its name, and the select's words for it, `GitHub (remote) ·
/// api.githubcopilot.com/mcp`.
struct Offered {
  id: String,
  name: String,
  words: String,
}

/// The modal asking about `ask`.
pub(super) fn dialog(ctx: &Ctx, here: &Here, ask: &Ask, actions: &Actions) -> Element {
  let chosen = actions.chosen.get();
  let detailed = |name: &str, detail: &str| -> String {
    ctx
      .t_args("sharing.bindings.detailed", [("name", name), ("detail", detail)])
      .to_string()
  };
  let (dialog, options, what) = match ask.reference {
    BoundReference::McpServer { .. } => {
      let options: Vec<Offered> = here
        .mcp_servers
        .iter()
        .map(|server| Offered {
          id: server.id().to_string(),
          name: server.name().as_str().to_owned(),
          words: detailed(server.name().as_str(), &server_detail(server)),
        })
        .collect();
      let local = name_of(&options, &chosen);
      let (dialog, what) = server(ctx, ask, &local);
      (dialog, options, what)
    }
    BoundReference::Secret { .. } => {
      let project = ctx.t("sharing.bindings.projectSecret");
      let options: Vec<Offered> = here
        .secrets_of(here_role(here, ask).as_deref())
        .into_iter()
        .map(|secret| Offered {
          id: secret.id().to_string(),
          name: secret.name().as_str().to_owned(),
          words: detailed(secret.name().as_str(), &project),
        })
        .collect();
      let (dialog, what) = secret(ctx, ask);
      (dialog, options, what)
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
      .map(|offered| SelectOption::new(offered.id.clone(), offered.words.clone()))
      .collect(),
  )
  .render(ctx);
  let action = action(ctx, ask, actions);
  dialog
    .part(choice("sharing-bind-choice-field", &label, select))
    .part(what)
    .person_only(ctx.t("sharing.bindings.personOnly"))
    .action(action)
    .under(actions.person.reason(ctx, CONFIRM))
    .render(ctx, &actions.open)
}

/// *Use “GitHub (remote)” for github on this computer?* and what it lets agents do.
fn server(ctx: &Ctx, ask: &Ask, local: &str) -> (Dialog, Column) {
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
  let dialog = Dialog::new(
    "sharing-bind-server",
    Icon::Server,
    ctx.t_args("sharing.bindings.server.title", names),
    ctx.t("sharing.bindings.cancel"),
  );
  (dialog, what)
}

/// *Give a secret to SENTRY_TOKEN on this computer?* and what it lets tools do.
fn secret(ctx: &Ctx, ask: &Ask) -> (Dialog, Column) {
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
  let dialog = Dialog::new(
    "sharing-bind-secret",
    Icon::KeyRound,
    ctx.t_args("sharing.bindings.secret.title", names),
    ctx.t("sharing.bindings.cancel"),
  );
  (dialog, what)
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

/// The role, the secret's kind, of the reference `ask` is about: found by its kind and id, whatever it is bound to.
fn here_role(here: &Here, ask: &Ask) -> Option<String> {
  here
    .references
    .iter()
    .find(|reference| same_reference(reference.binding.reference, ask.reference))
    .and_then(|reference| reference.binding.role.clone())
}

/// `a` and `b` are the same reference, bound or not.
fn same_reference(a: BoundReference, b: BoundReference) -> bool {
  match (a, b) {
    (BoundReference::Connection { reference: a, .. }, BoundReference::Connection { reference: b, .. })
    | (BoundReference::McpServer { reference: a, .. }, BoundReference::McpServer { reference: b, .. }) => a == b,
    (BoundReference::Secret { reference: a, .. }, BoundReference::Secret { reference: b, .. }) => a == b,
    _ => false,
  }
}

/// The name of the option whose id is `chosen`; empty while none is.
fn name_of(options: &[Offered], chosen: &str) -> String {
  options
    .iter()
    .find(|offered| offered.id == chosen)
    .map(|offered| offered.name.clone())
    .unwrap_or_default()
}
