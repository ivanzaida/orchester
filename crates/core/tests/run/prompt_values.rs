//! Where each value a prompt section was given is in its text: the viewer marks exactly those ranges, even
//! when the same words are written elsewhere in the section.

use orchester_core::{
  name::Name,
  resource::{AgentProfile, InjectionEvent, PromptText, ProviderSettings},
  run::PromptSource,
};

use super::{
  fixture::{model, task},
  prompt::{Prompt, step_with},
};

#[test]
fn prompt_values_are_marked_where_they_were_put_in_not_where_the_words_repeat() {
  let task = task("Users sign in with GitHub.", &[]);
  let title = task.details().title.as_str().to_owned();
  let profile = AgentProfile::new(
    Name::new("Builder").expect("valid role"),
    format!("  {title}  "),
    PromptText::new("\n  {{task.title}} before {{run.revision}}, then {{ task.title }}  \n").expect("valid prompt"),
    orchester_core::id::ConnectionId::generate(),
    model("gpt-5.5"),
    ProviderSettings::default(),
  );
  let prompt = Prompt::new(profile, step_with("Work on {{task.title}}."), task);
  let composition = prompt.input(&[], &[], false).compose(InjectionEvent::Start);
  let role = composition
    .sections()
    .iter()
    .find(|section| matches!(section.source, PromptSource::AgentProfile { .. }))
    .expect("the role's section");
  assert_eq!(
    role.text,
    format!("{title}\n\n{title} before {{{{run.revision}}}}, then {title}")
  );
  let after_responsibilities = title.len() + 2;
  assert_eq!(
    role.values,
    [
      after_responsibilities..after_responsibilities + title.len(),
      role.text.len() - title.len()..role.text.len(),
    ]
  );
  let step = composition
    .sections()
    .iter()
    .find(|section| matches!(section.source, PromptSource::Step { .. }))
    .expect("the step's section");
  let marked: Vec<&str> = step.values.iter().map(|range| &step.text[range.clone()]).collect();
  assert_eq!(marked, [title.as_str()]);
  assert_eq!(step.values[0].start, "Work on ".len());
}
