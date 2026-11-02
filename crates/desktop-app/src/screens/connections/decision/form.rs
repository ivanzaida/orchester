//! Reusable Connection fields from 01 XMNUP and 01b z3G09h. Add's server choices are separate.
use lurq::{app::ctx::Ctx, core::Signal, node::Element};
use orchester_core::{
  connection::{DecisionConnection, DecisionServerUrl},
  id::DecisionConnectionId,
  name::Name,
};
use orchester_workspace::connections::{data::ProjectConnections, decision::DecisionDraft, native::KeySecret};

use crate::{
  screens::{connections::provider_form::ProviderForm, project_settings::kit::FormField},
  ui::field::FieldInput,
};

pub(in super::super) struct DecisionForm {
  pub(super) initialized: Signal<bool>,
  pub(super) name: Signal<String>,
  pub(super) url: Signal<String>,
  pub(super) key: ProviderForm,
}
impl DecisionForm {
  pub(in super::super) fn new(ctx: &mut Ctx) -> Self {
    Self {
      initialized: ctx.signal(false),
      name: ctx.signal(String::new()),
      url: ctx.signal(String::new()),
      key: ProviderForm::new(ctx),
    }
  }

  pub(in super::super) fn seed(&self, connection: &DecisionConnection) {
    self.name.set(connection.name().as_str().to_owned());
    self.url.set(connection.base_url().as_str().to_owned());
    self.key.seed_decision(connection);
  }

  pub(in super::super) fn draft(
    &self,
    id: Option<DecisionConnectionId>,
    listed: &ProjectConnections,
  ) -> Option<(DecisionDraft, Option<KeySecret>)> {
    let key = self.key.decision_key(listed)?;
    let draft = DecisionDraft {
      id,
      name: Name::new(&self.name.get()).ok()?,
      base_url: DecisionServerUrl::new(&self.url.get()).ok()?,
      key: match &key {
        Some(KeySecret::Existing(id)) => Some(*id),
        _ => None,
      },
    };
    Some((draft, key))
  }

  pub(in super::super) fn rows(&self, ctx: &mut Ctx, listed: &ProjectConnections, local: bool) -> Vec<Element> {
    let name = FieldInput::new(
      "decision-edit-name",
      ctx.t("decision_connection.name"),
      self.name.clone(),
    )
    .render(ctx);
    let mut url = FieldInput::new(
      "decision-edit-url",
      ctx.t("decision_connection.baseUrl"),
      self.url.clone(),
    )
    .mono();
    if !self.url.get().is_empty() && DecisionServerUrl::new(&self.url.get()).is_err() {
      url = url.error(ctx.t("decision_connection.invalidUrl"));
    }
    let url = url.render(ctx);
    vec![
      FormField::new("decision-name-field", ctx.t("decision_connection.name")).render(ctx, name),
      FormField::new("decision-url-field", ctx.t("decision_connection.baseUrl"))
        .hint(ctx.t(if local {
          "decision_connection.localUrlHint"
        } else {
          "decision_connection.urlHint"
        }))
        .render(ctx, url),
      self.key.decision_key_row(ctx, listed, local),
    ]
  }
}
