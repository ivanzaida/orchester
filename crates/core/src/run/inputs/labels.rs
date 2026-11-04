//! How a run's inputs name the resources they pin: the pipeline, each profile, instruction, tool and skill with the
//! slug and name its publisher gave it when the inputs were accepted. Prompts, skill folders and tool steps name them
//! so, without the resource rows this computer has now.

use serde::{Deserialize, Serialize};

use super::InputsError;
use crate::{id::ResourceId, name::Name, resource::Slug};

/// `{resource, slug, name}`: a pinned resource's slug and name when the inputs were accepted.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceLabel {
  pub resource: ResourceId,
  pub slug: Slug,
  pub name: Name,
}

/// `labels` in resource id order: one for each resource of `pinned`, and no other.
pub(super) fn checked(
  mut labels: Vec<ResourceLabel>,
  pinned: &[ResourceId],
) -> Result<Vec<ResourceLabel>, InputsError> {
  labels.sort_by_key(|label| label.resource);
  if let Some(pair) = labels.windows(2).find(|pair| pair[0].resource == pair[1].resource) {
    return Err(InputsError::UnexpectedLabel(pair[0].resource));
  }
  if let Some(extra) = labels.iter().find(|label| !pinned.contains(&label.resource)) {
    return Err(InputsError::UnexpectedLabel(extra.resource));
  }
  match pinned
    .iter()
    .find(|resource| !labels.iter().any(|label| label.resource == **resource))
  {
    Some(missing) => Err(InputsError::Unlabelled(*missing)),
    None => Ok(labels),
  }
}
