//! Services and stacks (`docs/architecture/agent-testing.md`, "Service" and "Stack"): the programs Orchester starts
//! for a run, and named sets of them. A service's program, arguments and texts keep their `{{…}}` placeholders as
//! written; they are filled per run when it starts. No type here holds a secret value: a variable or header names
//! a project secret, and `Debug` leaves out plain values and request bodies.
//!
//! Both are versioned resources ([`crate::resource::ResourceBody`]) with drafts and published versions; publishing
//! checks their needs against the published services ([`PublishNeeds`]).

mod definition;
mod error;
mod folder;
mod name;
mod needs;
mod placeholder;
mod publish;
mod readiness;
mod seed;
mod stack;
mod stored;
mod value;

pub use self::{
  definition::{RunData, Service, UiDriver, UiStart},
  error::ServiceError,
  folder::{OwnedPath, RelativeFolder, RunFolder},
  name::{ServiceName, StackName},
  needs::{NeedsError, StartPlan},
  placeholder::{FilledSeedRequest, Placeholder, PlaceholderError, PlaceholderPlace, RunValues, check_placeholders},
  publish::PublishNeeds,
  readiness::{HttpPath, LogPattern, Readiness, ReadyCheck, ReadyTimeout},
  seed::{Capture, CapturePath, Seed, SeedHeader, SeedRequest, SeedRequests},
  stack::Stack,
  value::{ServiceValue, ServiceVariables},
};
