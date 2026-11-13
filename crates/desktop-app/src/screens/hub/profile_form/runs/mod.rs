//! What a profile runs with, in one field (`Agents on accounts` C1 `RwzH2`, C2 `t7s3I`, C4 `IgFWn`): a local
//! profile's account, model and effort (*Runs on*), a team profile's CLI, model and effort (*CLI, model and effort*).
//! The field ([`field`]) opens the compact *Runs on* picker over what the profile can run on ([`picker`]): this
//! computer's accounts for a local profile, the CLIs for a team's. A form holds the choice in its fields
//! ([`Runs::of`], [`form`]); a local profile's panel holds it on the profile ([`super::held`]).

pub(super) mod field;
mod form;
pub(super) mod picker;

use orchester_core::{connection::AgentProvider, id::ConnectionId, resource::ModelId};
use orchester_workspace::agents::runs_on;

use self::picker::{Item, Who};
use super::{Form, ProfileFields, Scope};

/// What the fields come to, with what the picker offers.
pub(super) struct Runs {
  /// What the rail offers, in its order.
  items: Vec<Item>,
  /// The place on the rail of the item chosen.
  on: Option<usize>,
  pub cli: Option<AgentProvider>,
  /// The connection a local profile runs on: the project's on the account chosen.
  pub connection: Option<ConnectionId>,
  pub model: Option<ModelId>,
  /// The effort chosen among those the model offers; `None` is the CLI's default.
  pub effort: Option<String>,
}

impl Runs {
  /// What the fields come to among what this computer offers: a model the item chosen lists, and an effort that model
  /// takes there.
  pub(super) fn of(fields: &ProfileFields, form: &Form<'_>, who: &Who<'_>) -> Self {
    let base = form.base;
    let offered = runs_on::choices(&form.offer.connections, base.map(|base| base.model()), |connection| {
      base.is_some_and(|base| base.cli() == connection.provider())
    });
    let items = match form.scope {
      Scope::Local => picker::items(&offered, None, who),
      Scope::Team => picker::kinds(&offered, base),
    };
    let chosen = fields.on.get();
    let on = items.iter().position(|item| *item.id == *chosen);
    let target = on.map(|at| &items[at].target);
    let typed = fields.model.get();
    let listed = target.and_then(|target| target.models().iter().find(|model| model.id == typed));
    let effort = fields.effort.get();
    let effort = listed
      .filter(|model| {
        !effort.is_empty() && (target.is_some_and(picker::Target::types_effort) || model.efforts.contains(&effort))
      })
      .map(|_| effort);
    Self {
      on,
      cli: target.map(picker::Target::cli),
      connection: target.and_then(picker::Target::connection).map(|choice| choice.id),
      model: listed.and_then(|model| ModelId::new(&model.id).ok()),
      effort,
      items,
    }
  }
}
