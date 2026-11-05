//! Decision connections reuse the provider's secret picker and masked new-secret fields.
use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::connection::DecisionConnection;
use orchester_workspace::connections::{data::ProjectConnections, native::KeySecret};

use super::{KeyChoice, ProviderForm};

impl ProviderForm {
  pub(in super::super) fn key_revision(&self) -> u64 {
    self.key_revision.get()
  }
  pub(in super::super) fn seed_decision(&self, connection: &DecisionConnection) {
    if self.seeded.get_untracked() {
      return;
    }
    self.seeded.set(true);
    self.on.set(super::OnProvider(true));
    self
      .key
      .set(connection.key().map_or(KeyChoice::None, KeyChoice::Existing));
  }

  pub(in super::super) fn decision_key(&self, listed: &ProjectConnections) -> Option<Option<KeySecret>> {
    if self.key.get() == KeyChoice::None {
      Some(None)
    } else {
      self.key_secret(listed).map(Some)
    }
  }

  pub(in super::super) fn decision_key_row(&self, ctx: &mut Ctx, listed: &ProjectConnections, local: bool) -> Element {
    self.key_row_with(ctx, listed, true, local)
  }
}
