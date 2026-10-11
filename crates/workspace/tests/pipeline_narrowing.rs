//! Resetting a step's inherited permissions keeps approval policy and publication checks intact.
use orchester_core::{
  id::ResourceId,
  name::Name,
  permission::{AllowedCommands, CommandPrefix, StepCommands, StepNetwork, StepPermissions},
  resource::{Pipeline, PipelineEdit, StepKind},
};
use orchester_workspace::pipelines::{narrowing, widening::publish_reasons};

#[test]
fn pipeline_narrowing_reset_preserves_tool_approval_and_requires_review_on_publish() {
  let restricted = StepPermissions {
    commands: StepCommands::Only(AllowedCommands::new(vec![CommandPrefix::new("cargo check").unwrap()]).unwrap()),
    network: StepNetwork::Off,
    interactive_tool_approvals: true,
  };
  let inherited = narrowing::inherit_profile(&restricted);
  assert_eq!(inherited.commands, StepCommands::AsProfile);
  assert_eq!(inherited.network, StepNetwork::AsProfile);
  assert!(inherited.interactive_tool_approvals);

  let mut edit = PipelineEdit::new(Pipeline::new(vec![]).unwrap());
  let key = edit.add_step(Name::new("Inspect").unwrap(), None);
  edit.update(&key, |step| {
    step.approval_gate = true;
    if let StepKind::Agent {
      profile, permissions, ..
    } = &mut step.kind
    {
      *profile = Some(ResourceId::generate());
      *permissions = restricted;
    }
  });
  let published = edit.validate().unwrap();
  edit.update(&key, |step| {
    if let StepKind::Agent { permissions, .. } = &mut step.kind {
      *permissions = inherited;
    }
  });
  let reset = edit.validate().unwrap();
  assert!(reset.steps()[0].approval_gate);
  assert!(!publish_reasons(Some(&published), &reset).is_empty());
}

#[test]
fn pipeline_narrowing_reset_does_not_add_a_tool_approval_gate() {
  let restricted = StepPermissions {
    commands: StepCommands::Only(AllowedCommands::default()),
    ..StepPermissions::default()
  };
  assert_eq!(narrowing::inherit_profile(&restricted), StepPermissions::default());
}
