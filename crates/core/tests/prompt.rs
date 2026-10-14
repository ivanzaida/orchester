use std::collections::HashSet;

use orchester_core::resource::{PromptError, PromptText, Variable, VariableGroup, VariableType, prompt_references};

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
fn prompt_catalog_has_the_25_designed_variables() {
  let names: HashSet<&str> = Variable::ALL.iter().map(|variable| variable.name()).collect();
  assert_eq!(names.len(), 25);
  for variable in Variable::ALL {
    assert_eq!(variable.name().parse::<Variable>(), Ok(variable));
  }
  for name in [
    "task.acceptance_criteria",
    "project.default_branch",
    "run.base_revision",
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
fn prompt_reference_ranges_preserve_unicode_whitespace_and_duplicates() {
  let raw = "é😀 {{ task.title }}\r\n{{task.title}} \\{{run.branch}}";
  let references = prompt_references(raw)
    .collect::<Result<Vec<_>, _>>()
    .expect("valid prompt grammar");
  assert_eq!(references.len(), 2);
  assert_eq!(&raw[references[0].range.clone()], "{{ task.title }}");
  assert_eq!(references[0].range.start, "é😀 ".len());
  assert_eq!(references[0].variable, Variable::TaskTitle);
  assert_eq!(&raw[references[1].range.clone()], "{{task.title}}");
}
#[test]
fn prompt_reference_scanner_retains_known_references_but_construction_rejects_errors() {
  let raw = "{{task.title}} {{secret.token}} {{run.branch}} {{unfinished";
  let scanned = prompt_references(raw).collect::<Vec<_>>();
  assert_eq!(scanned.len(), 4);
  assert_eq!(
    scanned[0].as_ref().expect("catalog variable").variable,
    Variable::TaskTitle
  );
  assert!(matches!(&scanned[1], Err(PromptError::UnknownVariable { .. })));
  assert_eq!(
    scanned[2].as_ref().expect("catalog variable").variable,
    Variable::RunBranch
  );
  assert!(matches!(&scanned[3], Err(PromptError::UnclosedPlaceholder { .. })));
  assert!(PromptText::new(raw).is_err());
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
  assert_eq!(counts, [6, 4, 4, 3, 6, 2]);
  assert_eq!(Variable::RunBudget.value_type(), VariableType::Tokens);
  assert_eq!(Variable::TaskAcceptanceCriteria.value_type(), VariableType::StringList);
}

#[test]
fn prompt_marked_rendering_records_where_each_value_went() {
  let prompt =
    PromptText::new(r"é\{{task.title}}: {{ task.title }} / {{run.revision}} / {{project.name}}").expect("valid prompt");
  let rendered = prompt.render_marked(|variable| match variable {
    Variable::TaskTitle => Some("😀 retry".to_owned()),
    Variable::ProjectName => Some("Kontur".to_owned()),
    _ => None,
  });
  assert_eq!(rendered.text, "é{{task.title}}: 😀 retry / {{run.revision}} / Kontur");
  let marked: Vec<&str> = rendered
    .values
    .iter()
    .map(|range| &rendered.text[range.clone()])
    .collect();
  assert_eq!(marked, ["😀 retry", "Kontur"]);
}

#[test]
fn prompt_variables_have_values_at_the_events_a_run_gives_them() {
  use orchester_core::resource::InjectionEvent;
  for variable in Variable::ALL {
    let never = matches!(
      variable,
      Variable::TaskAttachments
        | Variable::TaskCreatedAt
        | Variable::ProjectRepository
        | Variable::ProjectDefaultBranch
    );
    let at_start = !never && variable != Variable::RunRevision;
    assert_eq!(
      variable.has_value_at(InjectionEvent::Start),
      at_start,
      "{variable:?} at start"
    );
    assert_eq!(
      variable.has_value_at(InjectionEvent::Resume),
      !never,
      "{variable:?} at resume"
    );
  }
}
