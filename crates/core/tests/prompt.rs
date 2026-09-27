use std::collections::HashSet;

use orchester_core::resource::{PromptError, PromptText, Variable, VariableGroup, VariableType};

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

#[test]
fn prompt_escaped_braces_are_literal_text() {
  let prompt = PromptText::new(r"Use \{{name}} in Handlebars, not {{task.title}}; \{{ alone is fine too.")
    .expect("escaped placeholders are not parsed");
  assert_eq!(prompt.variables(), [Variable::TaskTitle]);
  assert!(PromptText::new(r"\{{unclosed").is_ok());
}

#[test]
fn prompt_marked_rendering_keeps_placeholders_without_values() {
  let prompt = PromptText::new(r"On {{ run.branch }} for {{task.title}}, \{{literal}} and {{run.branch}}.").unwrap();
  let rendered = prompt.render_marked(|variable| (variable == Variable::TaskTitle).then(|| "Sign-in".to_owned()));
  assert_eq!(
    rendered.text,
    "On {{ run.branch }} for Sign-in, {{literal}} and {{run.branch}}."
  );
  assert_eq!(rendered.unavailable, [Variable::RunBranch]);
  assert_eq!(prompt.render(|_| None), Err(Variable::RunBranch));
}

#[test]
fn prompt_catalog_groups_match_the_design() {
  let count = |group| {
    Variable::ALL
      .iter()
      .filter(|variable| variable.group() == group)
      .count()
  };
  let counts: Vec<usize> = VariableGroup::ALL.into_iter().map(count).collect();
  assert_eq!(counts, [6, 4, 4, 3, 5, 2]);
  assert_eq!(Variable::RunBudget.value_type(), VariableType::Tokens);
  assert_eq!(Variable::TaskAcceptanceCriteria.value_type(), VariableType::StringList);
}
