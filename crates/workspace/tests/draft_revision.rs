//! A draft's revision as automation reads it (`get_tool`, `get_profile`): it names the draft itself, not only
//! when it was saved, so two saves within one millisecond on the same version still differ.

use orchester_core::{
  id::ResourceId,
  name::Name,
  permission::StepPermissions,
  resource::{Draft, Pipeline, PromptText, RepositoryAccess, Step, StepKey, StepKind},
  time::Timestamp,
};
use orchester_workspace::drafts::draft_revision;

fn pipeline(step: &str) -> Pipeline {
  Pipeline::new(vec![Step {
    key: StepKey::new("research").expect("valid key"),
    name: Name::new(step).expect("valid name"),
    kind: StepKind::Agent {
      profile: Some(ResourceId::generate()),
      repository: RepositoryAccess::ReadOnly,
      model: None,
      permissions: StepPermissions::default(),
    },
    instructions: PromptText::new("").expect("valid prompt"),
    depends_on: Default::default(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  }])
  .expect("valid pipeline")
}

#[test]
fn draft_revision_tells_two_drafts_saved_in_one_millisecond_apart() {
  let at = Timestamp::from_unix_millis(1_000);
  let (mine, theirs) = (pipeline("Research"), pipeline("Research more"));
  let revision =
    |body: &Pipeline| draft_revision::<Pipeline>(Some(&Draft::new(body.clone(), None, at)), None).expect("a revision");
  assert_ne!(revision(&mine), revision(&theirs));
  assert_eq!(revision(&mine), revision(&mine), "the same draft names one revision");
  assert_eq!(
    draft_revision::<Pipeline>(None, None).expect("a revision"),
    "draft:none;latest:none"
  );
}
