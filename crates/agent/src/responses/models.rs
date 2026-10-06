//! OpenAI's models (`docs/architecture/native-agent.md` §3.2): `GET /models` lists ids only, with no
//! context window, output cap or reasoning efforts, so those come from a small built-in table of the
//! models Orchester knows. A model the table does not know is listed without them; a person enters them
//! by hand.
//!
//! The table was checked against the model pages of developers.openai.com on 2026-10-06
//! (`/api/docs/models/<id>`): context window, max output tokens and the `reasoning.effort` values each
//! page lists. Every reasoning model there takes `reasoning.summary: "auto"` (the reasoning guide).

use orchester_core::{native::model::RemoteModel, resource::ModelId};
use serde::Deserialize;

/// What Orchester knows of a model the listing does not describe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct KnownModel {
  pub id: &'static str,
  /// The tokens a request may hold: the page's maximum input where it names one, else its context
  /// window.
  pub context_window: u64,
  pub max_output: u32,
  /// The `reasoning.effort` values the page lists, least first.
  pub efforts: &'static [&'static str],
}

/// The models of the table, flagship first.
const KNOWN: [KnownModel; 4] = [
  KnownModel {
    id: "gpt-6-astra",
    context_window: 922_000,
    max_output: 128_000,
    efforts: &["low", "medium", "high", "xhigh", "max"],
  },
  KnownModel {
    id: "gpt-6.1-sol",
    context_window: 1_050_000,
    max_output: 128_000,
    efforts: &["low", "medium", "high", "xhigh", "max"],
  },
  KnownModel {
    id: "gpt-6-luna",
    context_window: 1_050_000,
    max_output: 128_000,
    efforts: &["none", "low", "medium", "high", "xhigh", "max"],
  },
  KnownModel {
    id: "gpt-5.5",
    context_window: 1_050_000,
    max_output: 128_000,
    efforts: &["none", "low", "medium", "high", "xhigh"],
  },
];

/// The table's entry for `model`.
pub(super) fn known(model: &ModelId) -> Option<&'static KnownModel> {
  KNOWN.iter().find(|known| known.id == model.as_str())
}

/// The answer of `GET /models`.
#[derive(Debug, Deserialize)]
pub(super) struct ModelList {
  data: Vec<ListedModel>,
}

#[derive(Debug, Deserialize)]
struct ListedModel {
  id: String,
}

impl ModelList {
  /// The listed models as Orchester lists them, with the table's limits and efforts where it knows the
  /// model; an id Orchester cannot use is left out.
  pub(super) fn remote(&self) -> Vec<RemoteModel> {
    self
      .data
      .iter()
      .filter_map(|listed| ModelId::new(&listed.id).ok())
      .map(|id| match known(&id) {
        Some(known) => RemoteModel {
          context_window: Some(known.context_window),
          max_output: Some(known.max_output),
          efforts: known.efforts.iter().map(|&effort| effort.to_owned()).collect(),
          id,
        },
        None => RemoteModel {
          id,
          context_window: None,
          max_output: None,
          efforts: Vec::new(),
        },
      })
      .collect()
  }
}
