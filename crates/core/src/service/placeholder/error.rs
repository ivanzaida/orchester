use thiserror::Error;

use super::{Placeholder, PlaceholderPlace};
use crate::service::ServiceName;

/// Why a placeholder of a service cannot be used or filled, with the place it is written in. The text around a
/// placeholder is never repeated: a body or a plain value may hold a password by mistake.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum PlaceholderError {
  #[error("{place} opens a placeholder with `{{{{` but never closes it with `}}}}`")]
  Unclosed { place: PlaceholderPlace },
  #[error("{place} has a placeholder with nothing or a brace between `{{{{` and `}}}}`")]
  Malformed { place: PlaceholderPlace },
  /// Such as `{{host}}` or `{{services.web.host}}`.
  #[error(
    "{place}: `{placeholder}` is not a placeholder of a service; they are `{{{{port}}}}`, `{{{{url}}}}`, \
     `{{{{run}}}}`, `{{{{data}}}}`, `{{{{services.<name>.url}}}}`, `{{{{services.<name>.port}}}}` and \
     `{{{{seed.<name>}}}}`"
  )]
  UnknownField {
    place: PlaceholderPlace,
    placeholder: String,
  },
  #[error("{place} uses the service `{service}`, which is not defined")]
  UnknownService { place: PlaceholderPlace, service: String },
  /// Only a service it needs is started and ready before it.
  #[error("{place} uses the service `{service}`, which this service does not need")]
  NotNeeded {
    place: PlaceholderPlace,
    service: ServiceName,
  },
  /// Captures exist only after the seed request that makes them, so arguments, variables and readiness have none.
  #[error("{place} uses `seed.{capture}`, which no earlier seed request captures")]
  UnknownCapture { place: PlaceholderPlace, capture: String },
  /// `{{data}}` is the folder of the run's own files, which only a service whose per-run data is a run folder has.
  #[error("{place} uses `{{{{data}}}}`, but this service's per-run data is not a run folder")]
  NoRunFolder { place: PlaceholderPlace },
  /// The run's values lack what the placeholder stands for.
  #[error("{place}: the run has no value for `{placeholder}`")]
  NoValue {
    place: PlaceholderPlace,
    placeholder: Placeholder,
  },
}
