use orchester_core::resource::{PromptText, Variable};
use orchester_workspace::instructions::tokens::inline_references;

#[test]
fn instructions_tokens_use_exact_current_utf8_source_and_reparse_invalid_edits() {
  let raw = "é😀 {{ task.title }} {{secret.token}} {{run.branch}} {{partial";
  let refs = inline_references(raw);
  assert_eq!(refs.len(), 2);
  assert_eq!(refs[0].variable, Variable::TaskTitle);
  assert_eq!(refs[0].range.start, "é😀 ".len());
  assert_eq!(&raw[refs[0].range.clone()], "{{ task.title }}");
  assert!(PromptText::new(raw).is_err());
  let changed = raw.replacen("{{ task.title }}", "{{ task.titl }}", 1);
  let refs = inline_references(&changed);
  assert_eq!(refs.len(), 1);
  assert_eq!(refs[0].variable, Variable::RunBranch);
  assert_eq!(&changed[refs[0].range.clone()], "{{run.branch}}");
}
#[test]
fn instructions_tokens_keep_escaped_and_crlf_source_authoritative() {
  let raw = "\\{{task.title}}\r\n{{run.branch}}\r{{\ntask.title\r\n}}\n{{project.name}}\r\n";
  let refs = inline_references(raw);
  assert_eq!(refs.len(), 2);
  assert_eq!(refs[0].variable, Variable::RunBranch);
  assert_eq!(refs[1].variable, Variable::ProjectName);
  assert_eq!(
    PromptText::new(raw)
      .expect("multiline whitespace is valid grammar")
      .as_str(),
    raw
  );
  assert!(
    refs
      .iter()
      .all(|reference| !raw[reference.range.clone()].contains(['\r', '\n']))
  );
}
#[test]
fn instructions_recovery_replaces_only_actual_variable_references() {
  let raw = "\\{{run.revision}} {{ run.revision }} {{\nrun.revision\n}} {{run.base_revision}}";
  let replaced =
    orchester_workspace::instructions::tokens::replace_variable(raw, Variable::RunRevision, Variable::RunBaseRevision);
  assert_eq!(
    replaced,
    "\\{{run.revision}} {{run.base_revision}} {{run.base_revision}} {{run.base_revision}}"
  );
}
