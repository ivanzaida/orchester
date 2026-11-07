//! probe
use crate::harness::Form;

#[test]
fn form_readiness_probe() {
  let mut form = Form::seeded("live");
  form.new_task();
  form.open("task-form-pipeline");
  for i in 0..6 {
    let id = format!("task-form-pipeline-option-{i}");
    eprintln!("{i}: {:?} | {:?}", form.window.texts(&format!("{id}-title")), form.window.texts(&format!("{id}-status")));
  }
  eprintln!("{:?}", form.reasons());
}
