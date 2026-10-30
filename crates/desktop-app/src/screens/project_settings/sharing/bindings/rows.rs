//! A reference's row on On this computer (`Claude Code “Work”` `mKubw`, `github` `n21X6J`, `STAGING_DB_URL` `cE1bT`
//! in `07`): its tile, its name over who needs it and how it stands (in `danger` while it is missing), and at its end
//! what it is bound to with a dot, *by name*, a *by role* badge or *chosen here*, and *Change*; or the picker that
//! binds it; or *Waits for you* with *Confirm…* for an MCP server found here by its name.

use std::sync::Arc;

use lurq::{
  app::ctx::{Ctx, FutureAction},
  core::Signal,
  node::Element,
};
use orchester_core::{id::ConnectionId, sharing::ConnectionRole};
use orchester_store::{Bound, BoundBy, BoundReference};
use orchester_workspace::sharing::sync::{Here, HereReference, Standing, chosen_at};

use super::{Ask, bind_picked};
use crate::{
  screens::{
    hub::parts::{Tone as DotTone, quiet, status},
    project_settings::kit::{SettingRow, Tone, tile},
    settings::orchester::{PersonOnly, Said, day_of},
    shared_config::{
      needs,
      picker::{self, Item, Opener, SELECT_WIDTH},
    },
  },
  ui::{
    button::{ActionButton, ButtonKind},
    chip::{BadgeTone, badge},
    icon::Icon,
  },
};

/// The kinds of reference, each a group of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
  Connection,
  McpServer,
  Secret,
}

impl Kind {
  const fn of(reference: BoundReference) -> Self {
    match reference {
      BoundReference::Connection { .. } => Self::Connection,
      BoundReference::McpServer { .. } => Self::McpServer,
      BoundReference::Secret { .. } => Self::Secret,
    }
  }

  const fn glyph(self) -> Icon {
    match self {
      Self::Connection => Icon::Plug,
      Self::McpServer => Icon::Server,
      Self::Secret => Icon::KeyRound,
    }
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Connection => "connection",
      Self::McpServer => "server",
      Self::Secret => "secret",
    }
  }
}

/// What the rows do: bind a connection, or ask the person about an MCP server or a secret.
#[derive(Clone)]
pub(super) struct Actions {
  pub bind: FutureAction<BoundReference, (), Said>,
  pub person: PersonOnly,
  pub asking: Signal<Option<Ask>>,
  pub open: Signal<bool>,
  pub chosen: Signal<String>,
  /// A binding is being made: the controls wait.
  pub busy: bool,
}

impl Actions {
  /// Opens the modal about `ask` with `local` chosen in it.
  fn ask(&self, ask: Ask, local: String) {
    self.chosen.set(local);
    self.asking.set(Some(ask));
    self.open.set(true);
  }
}

/// The rows of the references of `kind`.
pub(super) fn rows(ctx: &mut Ctx, here: &Here, kind: Kind, actions: &Actions) -> Vec<Element> {
  here
    .references
    .iter()
    .filter(|reference| Kind::of(reference.binding.reference) == kind)
    .enumerate()
    .map(|(index, reference)| row(ctx, here, reference, (kind, index), actions))
    .collect()
}

fn row(
  ctx: &mut Ctx,
  here: &Here,
  reference: &HereReference,
  (kind, index): (Kind, usize),
  actions: &Actions,
) -> Element {
  let id = format!("sharing-binding-{}-{index}", kind.name());
  let needs = needs(ctx, &reference.needed_by);
  let (hint, tone) = hint(ctx, here, reference, &needs);
  let controls = match &reference.standing {
    Standing::Bound { local, by } => bound(ctx, &id, here, reference, (local, *by), actions),
    Standing::Found { local, .. } => found(ctx, &id, reference, *local, (&needs, actions)),
    Standing::Missing => missing(ctx, &id, here, reference, &needs, actions),
  };
  SettingRow::new(id, reference.binding.name.clone())
    .lead(tile(ctx, kind.glyph()))
    .toned(hint, tone)
    .controls(controls)
    .render(ctx)
}

/// Who needs it and how it stands, and the tone that says it.
fn hint(ctx: &Ctx, here: &Here, reference: &HereReference, needs: &str) -> (Arc<str>, Tone) {
  let role = reference.binding.role.clone().unwrap_or_default();
  let args = |more: Vec<(&'static str, String)>| {
    let mut args = vec![("needs", needs.to_owned()), ("role", role.clone())];
    args.extend(more);
    args
  };
  let connection = matches!(reference.binding.reference, BoundReference::Connection { .. });
  match &reference.standing {
    Standing::Missing if connection && here.connections_of(&role).is_empty() => (
      ctx.t_args("sharing.bindings.hint.noAccount", args(Vec::new())),
      Tone::Danger,
    ),
    Standing::Missing if connection => (
      ctx.t_args("sharing.bindings.hint.connectionMissing", args(Vec::new())),
      Tone::Danger,
    ),
    Standing::Missing => (
      ctx.t_args("sharing.bindings.hint.missing", args(Vec::new())),
      Tone::Danger,
    ),
    Standing::Found { name, .. } => (
      ctx.t_args("sharing.bindings.hint.found", args(vec![("local", name.clone())])),
      Tone::Muted,
    ),
    Standing::Bound { .. } if connection => (
      ctx.t_args("sharing.bindings.hint.connection", args(Vec::new())),
      Tone::Muted,
    ),
    Standing::Bound { .. } => match chosen_at(&reference.binding) {
      Some(at) => (
        ctx.t_args("sharing.bindings.hint.confirmed", args(vec![("date", day_of(ctx, at))])),
        Tone::Muted,
      ),
      None => (needs.into(), Tone::Muted),
    },
  }
}

/// What it is bound to, how, and *Change* for a connection.
fn bound(
  ctx: &mut Ctx,
  id: &str,
  here: &Here,
  reference: &HereReference,
  (local, by): (&str, BoundBy),
  actions: &Actions,
) -> Vec<Element> {
  let mut parts: Vec<Element> = vec![status(&format!("{id}-bound"), DotTone::Success, local).into()];
  let BoundReference::Connection { .. } = reference.binding.reference else {
    return parts;
  };
  parts.push(match by {
    BoundBy::Name => quiet(&ctx.t("sharing.bindings.byName")).into(),
    BoundBy::Role => badge(
      &format!("{id}-by-role"),
      BadgeTone::Neutral,
      &ctx.t("sharing.bindings.byRole"),
    )
    .into(),
    BoundBy::Person => quiet(&ctx.t("sharing.bindings.chosenHere")).into(),
  });
  parts.push(connection_picker(ctx, id, here, reference, Opener::Change, actions));
  parts
}

/// *Waits for you* and *Confirm…*: an MCP server found here by its name, `local`, which the person confirms.
fn found(
  ctx: &Ctx,
  id: &str,
  reference: &HereReference,
  local: ConnectionId,
  (needs, actions): (&str, &Actions),
) -> Vec<Element> {
  let ask = Ask {
    reference: reference.binding.reference,
    name: reference.binding.name.clone(),
    needs: needs.to_owned(),
  };
  let (asks, local) = (actions.clone(), local.to_string());
  let confirm = ActionButton::new(
    format!("{id}-confirm"),
    ButtonKind::Secondary,
    ctx.t("sharing.bindings.confirm"),
  )
  .icon(Icon::ShieldCheck)
  .in_form()
  .on_click(move || asks.ask(ask.clone(), local.clone()));
  let confirm = if actions.busy { confirm.disabled() } else { confirm };
  let waits = badge(
    &format!("{id}-waits"),
    BadgeTone::Warning,
    &ctx.t("sharing.bindings.waits"),
  );
  vec![waits.into(), confirm.into()]
}

/// The picker that binds it, or *Add an account…* when no connection of its role is here.
fn missing(
  ctx: &mut Ctx,
  id: &str,
  here: &Here,
  reference: &HereReference,
  needs: &str,
  actions: &Actions,
) -> Vec<Element> {
  let select = |key: &str| Opener::Select {
    words: ctx.t(key),
    width: SELECT_WIDTH,
  };
  match reference.binding.reference {
    BoundReference::Connection { .. } => {
      let role = reference.binding.role.clone().unwrap_or_default();
      if here.connections_of(&role).is_empty() {
        return vec![picker::add_account(ctx, &format!("{id}-add-account"))];
      }
      let opener = select("sharing.bindings.chooseConnection");
      vec![connection_picker(ctx, id, here, reference, opener, actions)]
    }
    BoundReference::McpServer { .. } => {
      let opener = select("sharing.bindings.chooseServer");
      let items: Vec<(String, Arc<str>)> = here
        .mcp_servers
        .iter()
        .map(|server| (server.id().to_string(), server.name().as_str().into()))
        .collect();
      vec![asking_picker(
        ctx,
        id,
        (reference, needs),
        (opener, items, Icon::Server),
        actions,
      )]
    }
    BoundReference::Secret { .. } => {
      let opener = select("sharing.bindings.chooseSecret");
      let items: Vec<(String, Arc<str>)> = here
        .secrets_of(reference.binding.role.as_deref())
        .into_iter()
        .map(|secret| (secret.id().to_string(), secret.name().as_str().into()))
        .collect();
      vec![asking_picker(
        ctx,
        id,
        (reference, needs),
        (opener, items, Icon::KeyRound),
        actions,
      )]
    }
  }
}

/// The person's connections, the ones of another role unavailable, then *Add an account…*: a choice binds it.
fn connection_picker(
  ctx: &mut Ctx,
  id: &str,
  here: &Here,
  reference: &HereReference,
  opener: Opener,
  actions: &Actions,
) -> Element {
  let role = reference.binding.role.clone().unwrap_or_default();
  let current = match reference.binding.reference {
    BoundReference::Connection { bound, .. } => bound.map(|bound| bound.local),
    _ => None,
  };
  let another = ctx.t("sharing.bindings.anotherRole");
  let items = here
    .connections
    .iter()
    .map(|connection| Item {
      label: connection.name().as_str().into(),
      glyph: Icon::User,
      detail: None,
      chosen: current == Some(connection.id()),
      unavailable: (ConnectionRole::of(connection.kind()).as_str() != role).then(|| another.clone()),
    })
    .collect();
  let ids: Vec<ConnectionId> = here.connections.iter().map(|connection| connection.id()).collect();
  let (actions, target) = (actions.clone(), reference.binding.reference);
  picker::picker(
    ctx,
    picker::Props {
      id: format!("{id}-picker").into(),
      opener,
      items,
      add_account: true,
      choose: Arc::new(move |index| {
        let (BoundReference::Connection { reference, .. }, Some(local)) = (target, ids.get(index)) else {
          return;
        };
        let bound = Some(Bound {
          local: *local,
          by: BoundBy::Person,
        });
        bind_picked(&actions, BoundReference::Connection { reference, bound });
      }),
    },
  )
}

/// The MCP servers or secrets here: a choice opens the modal that binds it, with that one chosen.
fn asking_picker(
  ctx: &mut Ctx,
  id: &str,
  (reference, needs): (&HereReference, &str),
  (opener, choices, glyph): (Opener, Vec<(String, Arc<str>)>, Icon),
  actions: &Actions,
) -> Element {
  let mut items: Vec<Item> = choices
    .iter()
    .map(|(_, label)| Item {
      label: label.clone(),
      glyph,
      detail: None,
      chosen: false,
      unavailable: None,
    })
    .collect();
  if items.is_empty() {
    items.push(Item {
      label: ctx.t("sharing.bindings.noneHere"),
      glyph,
      detail: None,
      chosen: false,
      unavailable: Some(ctx.t("sharing.bindings.noneHereReason")),
    });
  }
  let ask = Ask {
    reference: reference.binding.reference,
    name: reference.binding.name.clone(),
    needs: needs.to_owned(),
  };
  let actions = actions.clone();
  picker::picker(
    ctx,
    picker::Props {
      id: format!("{id}-picker").into(),
      opener,
      items,
      add_account: false,
      choose: Arc::new(move |index| {
        if let Some((local, _)) = choices.get(index) {
          actions.ask(ask.clone(), local.clone());
        }
      }),
    },
  )
}
