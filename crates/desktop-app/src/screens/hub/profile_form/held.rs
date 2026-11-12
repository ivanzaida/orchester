//! The fields of what a local profile runs with in its panel (L1 `fFxoM`), where the panel holds the profile and the
//! workspace makes each choice on it ([`local_edit::hold`]): a choice in a select is an [`Edit`] handed to the panel,
//! and the selects show what the panel holds once no choice is being checked, so a choice that was refused, or that
//! *Cancel* dropped, is put back.
//!
//! [`local_edit::hold`]: orchester_workspace::agents::local_edit::hold

use std::sync::Arc;

use lurq::{
  app::ctx::Ctx,
  components::Row,
  core::{Ref, Signal},
};
use orchester_core::{connection::AgentProvider, id::ConnectionId};
use orchester_workspace::agents::{local_edit::Edit, local_hub::LocalProfile};

use super::{Form, ProfileFields, runs::cli_value};

/// What a choice does: the panel makes the edit on what it holds.
pub(in crate::screens) type OnEdit = Arc<dyn Fn(Edit) + Send + Sync>;

/// What the panel holds, as the selects hold values.
#[derive(Clone, Debug, Default)]
struct Values {
  clis: Vec<AgentProvider>,
  cli: String,
  connection: String,
  model: String,
  effort: String,
}

impl Values {
  fn of(profile: &LocalProfile) -> Self {
    Self {
      clis: profile.options.clis.clone(),
      cli: profile.cli.map(cli_value).unwrap_or_default(),
      connection: profile.current.map(|id| id.to_string()).unwrap_or_default(),
      model: profile.model.clone(),
      effort: profile.effort.clone().unwrap_or_default(),
    }
  }
}

/// The fields of a local profile's panel. Cloning shares them.
#[derive(Clone)]
pub(in crate::screens) struct HeldFields {
  fields: ProfileFields,
  /// What the panel holds, as the selects last heard it: a value other than it is the person's choice.
  held: Ref<Values>,
}

impl HeldFields {
  /// The fields, each choice in them made by `on_edit`.
  pub(in crate::screens) fn new(ctx: &mut Ctx, on_edit: &OnEdit) -> Self {
    let fields = ProfileFields::new(ctx);
    let held: Ref<Values> = ctx.create_ref(Values::default());
    let follow = |ctx: &mut Ctx, signal: &Signal<String>, edit: fn(&Values, &str) -> Option<Edit>| {
      let (held, on_edit) = (held.clone(), on_edit.clone());
      ctx.watch(signal, move |value: &String| {
        let now = held.get();
        if let Some(edit) = edit(&now, value) {
          on_edit(edit);
        }
      });
    };
    follow(ctx, &fields.cli, |held, value| {
      (value != held.cli)
        .then(|| held.clis.iter().find(|cli| cli_value(**cli) == value))
        .flatten()
        .map(|cli| Edit::Cli(*cli))
    });
    follow(ctx, &fields.connection, |held, value| {
      (value != held.connection)
        .then(|| value.parse::<ConnectionId>().ok())
        .flatten()
        .map(Edit::Connection)
    });
    follow(ctx, &fields.model, |held, value| {
      (value != held.model && !value.is_empty()).then(|| Edit::Model(value.to_owned()))
    });
    follow(ctx, &fields.effort, |held, value| {
      (value != held.effort).then(|| Edit::Effort(Some(value.to_owned()).filter(|effort| !effort.is_empty())))
    });
    Self { fields, held }
  }

  /// The selects follow `shown`, what the panel holds, unless a choice is being checked (`choosing`): they keep
  /// showing what the person chose until the panel holds it.
  pub(in crate::screens) fn keep(&self, shown: &LocalProfile, choosing: bool) {
    let values = Values::of(shown);
    self.held.set(values.clone());
    if choosing {
      return;
    }
    let fields = &self.fields;
    for (signal, value) in [
      (&fields.cli, values.cli),
      (&fields.connection, values.connection),
      (&fields.model, values.model),
      (&fields.effort, values.effort),
    ] {
      if signal.get_untracked() != value {
        signal.set(value);
      }
    }
  }

  /// The rows of what it runs with, as `form` draws them; `form.held` is what the panel holds.
  pub(in crate::screens) fn rows(&self, ctx: &mut Ctx, form: &Form<'_>) -> Vec<Row> {
    self.fields.runs_rows(ctx, form)
  }
}
