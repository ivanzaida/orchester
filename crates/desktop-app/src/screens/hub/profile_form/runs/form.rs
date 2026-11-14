//! The field in a form of a profile (a new one's, and an admin's edit of a team one): what the picker hands back goes
//! to the form's fields. A model is chosen with what lists it, so choosing one chooses the account, or the CLI, it is
//! shown under; an item of the rail is only shown until then, and afterwards takes the model with it where it lists
//! that model too. An account the project has no connection on yet gets one when it is looked at
//! ([`Connect`](crate::screens::hub::profile_form::Connect)), so that its models can be listed.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::Row,
  core::Signal,
};

use super::{
  Runs,
  field::Field,
  picker::{self, Held, Item, Who},
};
use crate::{
  screens::{
    hub::profile_form::{Connect, Form, ProfileFields, Scope},
    team_hub::rows::Sub,
  },
  ui::{
    icon::Icon,
    message::message_text,
    runs_on_picker::{Notice, NoticeTone, Pick, RunsOnPicker},
  },
};

impl Runs {
  /// The row of the field.
  pub(in crate::screens::hub::profile_form) fn rows(
    &self,
    ctx: &mut Ctx,
    fields: &ProfileFields,
    (form, who): (&Form<'_>, &Who<'_>),
  ) -> Vec<Row> {
    let team = form.scope == Scope::Team;
    let held = Held {
      on: self.on.map(|at| &self.items[at]),
      model: self.model.as_ref().map(|model| model.as_str()),
      effort: self.effort.as_deref(),
      team,
    };
    let said = picker::label(ctx, held, who);
    let missing = form.tried && !said.chosen;
    let sub = if missing {
      Some(Sub {
        words: ctx.t("team_new.errors.required"),
        color: PaletteColor::Danger,
      })
    } else {
      team.then(|| Sub::quiet(ctx.t("team_hub.runsOn.sub")))
    };
    let field = Field {
      ids: form.ids.rows,
      label: ctx.t(if team {
        "team_hub.runsOn.label"
      } else {
        "local_hub.runsOn.label"
      }),
      sub,
      said,
      add: !team,
      locked: form.locked,
    };
    let id = field.menu_id();
    let chooser = Chooser {
      items: self.items.clone().into(),
      on: fields.on.clone(),
      model: fields.model.clone(),
      effort: fields.effort.clone(),
      connect: form.connect.cloned(),
    };
    if let (true, None, Some(connect), Some(account)) = (
      fields.menu.open.get() && !form.locked,
      held.on,
      form.connect,
      picker::opens_unconnected(&self.items),
    ) {
      connect.look(account);
    }
    let refused = form.connect.and_then(Connect::refused);
    let close = fields.menu.closer();
    vec![field.row(ctx, &fields.menu, |ctx| {
      let rail = picker::connections(ctx, &self.items, who);
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

/// What a pick of the picker does to the form's fields.
struct Chooser {
  items: Arc<[Item]>,
  on: Signal<String>,
  model: Signal<String>,
  effort: Signal<String>,
  connect: Option<Connect>,
}

impl Chooser {
  fn take(&self, pick: Pick) {
    match pick {
      Pick::Connection(id) => {
        let Some(item) = self.item(&id) else {
          return;
        };
        if let (Some(on), Some(connect)) = (item.target.unconnected(), &self.connect) {
          connect.run(on);
          return;
        }
        let model = self.model.get_untracked();
        if item.target.models().iter().any(|listed| listed.id == model) {
          self.choose(item, &model);
        }
      }
      Pick::Model { connection, model } => {
        let Some(item) = self.item(&connection) else {
          return;
        };
        // An API account lists its models before the project has a connection on it: the choice is kept, and the
        // connection it needs is made.
        self.choose(item, &model);
        if let (Some(on), Some(connect)) = (item.target.unconnected(), &self.connect) {
          connect.run(on);
        }
      }
      Pick::Effort(effort) => self
        .effort
        .set(effort.map(|effort| effort.to_string()).unwrap_or_default()),
    }
  }

  fn item(&self, id: &str) -> Option<&Item> {
    self.items.iter().find(|item| *item.id == *id)
  }

  /// The fields run on `item` with `model`; the effort they hold stays where that model takes it there, and is the
  /// CLI's default otherwise.
  fn choose(&self, item: &Item, model: &str) {
    self.on.set(item.id.to_string());
    self.model.set(model.to_owned());
    let effort = self.effort.get_untracked();
    let takes = item.target.types_effort()
      || item
        .target
        .models()
        .iter()
        .any(|listed| listed.id == model && listed.efforts.contains(&effort));
    if !effort.is_empty() && !takes {
      self.effort.set(String::new());
    }
  }
}
