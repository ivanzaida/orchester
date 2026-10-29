use super::{NeedsError, Service, Stack, StartPlan};

/// What publishing a service or a stack checks against the project's published services: every service its
/// needs or its list name is published, and the needs form no cycle. Every
/// [`crate::store::ResourceStore::publish_draft`] implementation checks it before publishing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublishNeeds<'a> {
  Service(&'a Service),
  Stack(&'a Stack),
}

impl PublishNeeds<'_> {
  /// `published` is the latest published version of every other service of the project, the one published
  /// left out: a service is checked as it would be published, with what the others need of it.
  pub fn check(self, published: &[Service]) -> Result<(), NeedsError> {
    match self {
      Self::Service(service) => {
        let services: Vec<Service> = published.iter().chain([service]).cloned().collect();
        StartPlan::for_service(&services, service.name()).map(drop)
      }
      Self::Stack(stack) => StartPlan::for_stack(published, stack).map(drop),
    }
  }
}
