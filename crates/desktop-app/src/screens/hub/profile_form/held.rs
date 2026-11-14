//! What a local profile runs with in its panel (`Agents on accounts` C1 `RwzH2`, C2 `t7s3I`): one field, *Runs on*,
//! which says *Claude Code · Work · claude-opus-5-5 · High* and opens the compact *Runs on* picker under it
//! (`ui::runs_on_picker`): this computer's accounts along the rail ([`picker`]), the models of the one shown and the
//! efforts of the chosen model. The picker applies nothing itself; what a choice does depends on what it changes:
//!
//! - another account of the profile's CLI that serves its model is this computer's choice, made at once with no new
//!   version ([`OnHere`]), while the panel holds no change; held with the rest once it holds one;
//! - a model, or an effort, is the profile's: the panel holds it ([`OnEdit`]) and *Save* publishes it;
//! - an account that does not serve the profile's model is only shown: choosing one of its models holds the account
//!   with the model, and *Save* publishes both.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::Row,
};
use orchester_core::{
  connection::AgentProvider,
  id::{AgentAccountId, ConnectionId},
};
use orchester_workspace::agents::{local_edit::Edit, local_hub::LocalProfile};

use super::{
  Connect,
  connect::On,
  runs::{
    field::{Field, Menu},
    picker::{self, Held, Item, OnAccount, Target},
  },
};
use crate::{
  mcp::input_gate::{Guarded, InputGate, may_press},
  screens::team_hub::rows::Sub,
  services::{accounts, plan_limits},
  ui::{
    icon::Icon,
    message::message_text,
    runs_on_picker::{Notice, NoticeTone, Pick, RunsOnPicker},
  },
};

/// What a choice does: the panel makes the edit on what it holds.
pub(in crate::screens) type OnEdit = Arc<dyn Fn(Edit) + Send + Sync>;

/// Where the profile runs from now on, as this computer's choice: on a connection the project has, or on an account
/// of the person's, whose connection the project gets when it has none.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::screens) enum Here {
  Connection(ConnectionId),
  Account(AgentAccountId),
}

/// What choosing where the profile runs as it is does: this computer's choice, made at once.
pub(in crate::screens) type OnHere = Arc<dyn Fn(Here) + Send + Sync>;

/// How the panel stands when the field is drawn.
#[derive(Clone, Copy)]
pub(in crate::screens) struct Standing {
  /// The ids of the field's parts start with it.
  pub ids: &'static str,
  /// A save is under way: nothing is chosen now.
  pub locked: bool,
  /// The panel holds a change: an account chosen now is held with it.
  pub changed: bool,
}

/// The field of a local profile's panel. Cloning shares it.
#[derive(Clone)]
pub(in crate::screens) struct HeldFields {
  menu: Menu,
  on_edit: OnEdit,
  on_here: OnHere,
  connect: Connect,
  gate: Option<InputGate>,
}

impl HeldFields {
  /// The field, a choice in it made by `on_edit` or, when it is this computer's alone, by `on_here`; an account of
  /// another CLI that the project has no connection on gets one through `connect` when it is looked at.
  pub(in crate::screens) fn new(ctx: &mut Ctx, (on_edit, on_here): (&OnEdit, &OnHere), connect: Connect) -> Self {
    Self {
      menu: Menu::new(ctx),
      on_edit: on_edit.clone(),
      on_here: on_here.clone(),
      connect,
      gate: ctx.use_context::<InputGate>(),
    }
  }

  /// The row *Runs on* of `shown`, what the panel holds.
  pub(in crate::screens) fn rows(&self, ctx: &mut Ctx, standing: Standing, shown: &LocalProfile) -> Vec<Row> {
    let accounts = accounts::listed(ctx);
    let readings = plan_limits::account_readings(ctx);
    let api = accounts::api_listed(ctx);
    let who = picker::Who {
      accounts: &accounts,
      readings: &readings,
      api: &api.0,
      connecting: self.connect.connecting(),
    };
    let items = picker::items(&shown.runs_on, shown.current, &who);
    let held = Held {
      on: items.iter().find(|item| item.is(shown.current)),
      model: Some(shown.model.as_str()),
      effort: shown.effort.as_deref(),
      team: false,
    };
    let said = picker::label(ctx, held, &who);
    let sub = (!said.chosen).then(|| Sub {
      words: ctx.t("local_hub.runsOn.choose"),
      color: PaletteColor::Warning,
    });
    let field = Field {
      ids: standing.ids,
      label: ctx.t("local_hub.runsOn.label"),
      sub,
      said,
      add: true,
      locked: standing.locked,
    };
    let id = field.menu_id();
    let chooser = Chooser {
      items: items.clone().into(),
      cli: shown.cli,
      current: shown.current,
      model: shown.model.as_str().into(),
      changed: standing.changed,
      on_edit: self.on_edit.clone(),
      on_here: self.on_here.clone(),
      connect: self.connect.clone(),
      gate: self.gate.clone(),
    };
    if let (true, None, Some(account)) = (
      self.menu.open.get() && !standing.locked,
      held.on,
      picker::opens_unconnected(&items),
    ) {
      self.connect.look(account);
    }
    let refused = self.connect.refused();
    let close = self.menu.closer();
    vec![field.row(ctx, &self.menu, |ctx| {
      let rail = picker::connections(ctx, &items, &who);
      let notes = refused
        .map(|refused| Notice {
          icon: Icon::CircleX,
          tone: NoticeTone::Danger,
          text: message_text(ctx, &refused),
        })
        .into_iter()
        .collect();
      RunsOnPicker::new(id, (rail, picker::chosen(held)), move |pick| chooser.take(pick), close).notes(notes)
    })]
  }
}

/// What a pick of the picker does, by what it changes.
struct Chooser {
  items: Arc<[Item]>,
  cli: Option<AgentProvider>,
  current: Option<ConnectionId>,
  model: Arc<str>,
  changed: bool,
  on_edit: OnEdit,
  on_here: OnHere,
  connect: Connect,
  gate: Option<InputGate>,
}

impl Chooser {
  fn take(&self, pick: Pick) {
    match pick {
      Pick::Connection(id) => {
        let Some(item) = self.item(&id) else {
          return;
        };
        match item.target.unconnected() {
          // An API account is looked at first, and so is a sign-in of another CLI: the project gets its connection
          // on it, and the choice is made on that.
          Some(on @ On::Api(_)) => self.connect.run(on),
          Some(on) if !self.runs_as_it_is(item) => self.connect.run(on),
          _ if self.runs_as_it_is(item) => self.run_on(item),
          _ => {}
        }
      }
      Pick::Model { connection, model } => {
        let Some(item) = self.item(&connection) else {
          return;
        };
        if let Some(on @ On::Api(_)) = item.target.unconnected() {
          // Its models are the account's own, listed before the project has a connection on it: the connection is
          // made first, and the model is chosen on it.
          self.connect.run(on);
          return;
        }
        if item.is(self.current) {
          (self.on_edit)(Edit::Model(model.to_string()));
        } else if *model == *self.model && self.runs_as_it_is(item) {
          self.run_on(item);
        } else if let Some(choice) = item.target.connection() {
          (self.on_edit)(Edit::RunsOn {
            connection: choice.id,
            model: model.to_string(),
          });
        }
      }
      Pick::Effort(effort) => (self.on_edit)(Edit::Effort(effort.map(|effort| effort.to_string()))),
    }
  }

  fn item(&self, id: &str) -> Option<&Item> {
    self.items.iter().find(|item| *item.id == *id)
  }

  /// Whether the profile runs on `item` with the CLI and the model it has. An account the project has no connection
  /// on yet, or one whose connection lists nothing, is taken to serve what its CLI does.
  fn runs_as_it_is(&self, item: &Item) -> bool {
    Some(item.target.cli()) == self.cli
      && item
        .target
        .connection()
        .is_none_or(|choice| choice.models.is_empty() || choice.serves(&self.model))
  }

  /// Another item the profile runs on as it is: this computer's choice at once, or held with what the panel already
  /// holds.
  fn run_on(&self, item: &Item) {
    if item.is(self.current) {
      return;
    }
    let connection = item.target.connection().map(|choice| choice.id);
    if let (true, Some(connection)) = (self.changed, connection) {
      (self.on_edit)(Edit::Connection(connection));
      return;
    }
    // Work that goes to another account or host is a person's to send there, or automation's where they let it
    // change agent permissions: this choice has no *Save* that would ask.
    if self.moves_work(item)
      && !may_press(
        self.gate.as_ref(),
        Guarded::PermissionWidening,
        "run a profile on another account",
      )
    {
      return;
    }
    match (connection, &item.target) {
      (Some(connection), _) => (self.on_here)(Here::Connection(connection)),
      (None, Target::Account(account)) => (self.on_here)(Here::Account(account.id)),
      // Anything else on the rail is a connection.
      (None, _) => {}
    }
  }

  /// Whether running on `item` sends the project's work somewhere it did not go (`RunsOnChoice::moves_work`); for
  /// an account the project has no connection on yet, whether it is another account than the profile runs on.
  fn moves_work(&self, item: &Item) -> bool {
    let before = self
      .items
      .iter()
      .find(|other| other.is(self.current))
      .and_then(|other| other.target.connection());
    match (item.target.connection(), &item.target) {
      (Some(choice), _) => choice.moves_work(before),
      (None, Target::Account(OnAccount { id, default, .. })) => {
        before.is_some_and(|before| !default && before.account != Some(*id))
      }
      (None, _) => false,
    }
  }
}
