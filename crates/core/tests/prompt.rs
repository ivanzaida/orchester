use std::collections::HashSet;

use orchester_core::resource::{PromptError, PromptText, Variable};

#[test]
fn prompt_lists_catalog_variables_in_first_use_order() {
  let prompt = PromptText::new("Implement {{task.title}} on {{ run.branch }}. Title again: {{task.title}}.").unwrap();
  assert_eq!(prompt.variables(), [Variable::TaskTitle, Variable::RunBranch]);
  assert_eq!(
    prompt.as_str(),
    "Implement {{task.title}} on {{ run.branch }}. Title again: {{task.title}}."
  );
}

#[test]
fn prompt_without_placeholders_is_plain_text() {
  let prompt = PromptText::new("Keep commits small. A lone } or { is text.").unwrap();
  assert!(prompt.variables().is_empty());
  assert!(PromptText::new("").unwrap().is_blank());
}

#[test]
fn prompt_rejects_unknown_variables() {
  assert_eq!(
    PromptText::new("Use {{task.title}} and {{secret.token}}"),
    Err(PromptError::UnknownVariable {
      name: "secret.token".into(),
      offset: 23,
    })
  );
  assert!(matches!(
    PromptText::new("{{}}"),
    Err(PromptError::UnknownVariable { name, offset: 0 }) if name.is_empty()
  ));
}

#[test]
fn prompt_rejects_unclosed_placeholders() {
  assert_eq!(
    PromptText::new("Title: {{task.title"),
    Err(PromptError::UnclosedPlaceholder { offset: 7 })
  );
}

#[test]
fn prompt_catalog_has_the_24_designed_variables() {
  let names: HashSet<&str> = Variable::ALL.iter().map(|variable| variable.name()).collect();
  assert_eq!(names.len(), 24);
  for variable in Variable::ALL {
    assert_eq!(variable.name().parse::<Variable>(), Ok(variable));
  }
  for name in [
    "task.acceptance_criteria",
    "project.default_branch",
    "run.revision",
    "step.name",
  ] {
    assert!(names.contains(name), "{name} is missing");
  }
}
