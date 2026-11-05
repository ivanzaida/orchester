//! ORC-663: a skill the person chose for a message, as the Orchestrator's turn on Codex takes it, at App Server's
//! boundary: the folder `skills/extraRoots/set` registers, the turn's input (`$<slug> <words>` and the skill named by
//! its `SKILL.md`), the read that lists Codex's skills before the turn, and the person's own skills it finds, which the
//! turn's arguments disable while Codex's bundled ones stay. The results are shaped as Codex 0.166.0 answered them
//! (`docs/evidence/orc-663-codex-skill-2026-11-05.md`).
//! `cargo test -p orchester-core --test codex_chat_skill codex_chat_skill`.

use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use orchester_core::{
  command::{Command, CommandError, LineExchange},
  connection::{
    app_server_config::{ChatConfiguration, ConfigReadError, chat_read_command, own_skills, read_chat_configuration},
    invocation::{CodexChat, CodexChatSkill, CodexMcp, EXTRA_ROOTS_METHOD, HttpAuthorization, McpLaunch, Platform},
    mcp::{McpServerName, McpUrl},
  },
  conversation::skill::codex_prompt,
  orchestrator::instructions::SERVER,
  resource::Slug,
  secret::SecretValue,
};
use serde_json::{Value, json};

fn slug() -> Slug {
  Slug::new("release-notes").expect("a slug")
}

fn chosen() -> CodexChatSkill {
  CodexChatSkill {
    root: PathBuf::from(r"H:\Projects\Kontur\.orchester\chat-skills\m1"),
    slug: slug(),
  }
}

#[test]
fn codex_chat_skill_is_mentioned_by_its_slug_before_the_words() {
  assert_eq!(codex_prompt(&slug(), "for v0.4"), "$release-notes for v0.4");
}

#[test]
fn codex_chat_skill_registers_its_folder_alone_and_names_the_skill_in_the_input() {
  let skill = chosen();
  assert_eq!(EXTRA_ROOTS_METHOD, "skills/extraRoots/set");
  assert_eq!(
    skill.extra_roots(),
    json!({"extraRoots": [r"H:\Projects\Kontur\.orchester\chat-skills\m1"]})
  );
  let file = Path::new(r"H:\Projects\Kontur\.orchester\chat-skills\m1")
    .join("release-notes")
    .join("SKILL.md");
  assert_eq!(skill.file(), file);
  let input = skill.input("for v0.4", &[]).expect("no file to read");
  assert_eq!(
    input,
    [
      json!({"type": "text", "text": "$release-notes for v0.4"}),
      json!({"type": "skill", "name": "release-notes", "path": file.to_string_lossy()}),
    ]
  );
}

/// A `skills/list` result as Codex 0.166.0 gives it: its bundled skills, the person's own, one an administrator's, a
/// repository skill the arguments disabled already, and the chosen one when registered.
fn listed() -> Value {
  let skill = |name: &str, scope: &str, enabled: bool, path: &str| json!({"name": name, "description": "A skill.", "scope": scope, "enabled": enabled, "path": path});
  json!({"data": [{"cwd": "/p", "errors": [], "skills": [
    skill("imagegen", "system", true, "/home/me/.codex/skills/.system/imagegen/SKILL.md"),
    skill("my-own", "user", true, "/home/me/.codex/skills/my-own/SKILL.md"),
    skill("agents-own", "user", true, "/home/me/.agents/skills/agents-own/SKILL.md"),
    skill("company", "admin", true, "/etc/codex/skills/company/SKILL.md"),
    skill("deploy", "repo", false, "/p/.agents/skills/deploy/SKILL.md"),
    skill("my-own", "user", true, "/home/me/.codex/skills/my-own/SKILL.md"),
  ]}]})
}

#[test]
fn codex_chat_skill_read_keeps_every_enabled_skill_but_codexs_bundled_ones_to_disable() {
  assert_eq!(
    own_skills(&listed()).expect("listed"),
    [
      PathBuf::from("/home/me/.codex/skills/my-own/SKILL.md"),
      PathBuf::from("/home/me/.agents/skills/agents-own/SKILL.md"),
      PathBuf::from("/etc/codex/skills/company/SKILL.md"),
    ]
  );
  assert_eq!(own_skills(&json!({})), Err(ConfigReadError::MissingSkills));
}

/// Answers an exchange with the lines it was made with, and keeps the requests it was given.
struct Answers(Vec<String>);

impl LineExchange for Answers {
  fn exchange(&self, _: &Command, done: &mut dyn FnMut(&str) -> bool) -> Result<Vec<String>, CommandError> {
    let mut lines = Vec::new();
    for line in &self.0 {
      lines.push(line.clone());
      if done(line) {
        break;
      }
    }
    Ok(lines)
  }
}

fn config_answer() -> String {
  json!({"id": 6, "result": {"config": {"mcp_servers": {"pencil": {"enabled": true}}}}}).to_string()
}

#[test]
fn codex_chat_skill_read_asks_the_configuration_and_the_skills_and_waits_for_both() {
  let command = chat_read_command(Command::new(Path::new("codex"), Vec::<String>::new()), Path::new("/p"));
  let asked: Vec<Value> = command
    .stdin()
    .expect("requests")
    .lines()
    .map(|line| serde_json::from_str(line).expect("JSON"))
    .collect();
  assert_eq!(asked[2]["method"], "config/read");
  assert_eq!(
    asked[3],
    json!({"id": 7, "method": "skills/list", "params": {"cwds": ["/p"], "forceReload": true}})
  );
  assert!(
    command.interactive_stdin(),
    "the input stays open until both answers are in"
  );

  let skills = json!({"id": 7, "result": listed()}).to_string();
  let notice = json!({"method": "skills/changed", "params": {}}).to_string();
  let read = read_chat_configuration(&Answers(vec![skills, notice, config_answer()]), &command).expect("read");
  assert_eq!(read.names.into_iter().collect::<Vec<_>>(), ["pencil"]);
  assert_eq!(read.own_skills.len(), 3, "{:?}", read.own_skills);

  let refused = json!({"id": 7, "error": {"code": -32601, "message": "method\nnot found"}}).to_string();
  assert_eq!(
    read_chat_configuration(&Answers(vec![config_answer(), refused]), &command),
    Err(ConfigReadError::SkillsRefused("method not found".to_owned()))
  );
  assert_eq!(
    read_chat_configuration(&Answers(vec![config_answer()]), &command),
    Err(ConfigReadError::NoSkillsAnswer)
  );
  assert_eq!(ChatConfiguration::default().own_skills, Vec::<PathBuf>::new());
}

#[test]
fn codex_chat_skill_turn_disables_the_persons_own_skills_by_path_with_the_projects() {
  let launch = McpLaunch::http(
    McpServerName::new(SERVER).expect("a name"),
    McpUrl::new("http://127.0.0.1:9/mcp").expect("a URL"),
    HttpAuthorization::Bearer(Arc::new(SecretValue::new("token".to_owned()).expect("not blank"))),
  );
  let (mcp, _) = CodexMcp::new(&[launch]);
  let mut skills = vec![PathBuf::from("/p/.agents/skills/deploy/SKILL.md")];
  skills.extend(own_skills(&listed()).expect("listed"));
  let args = CodexChat {
    executable: Path::new("/bin/codex"),
    folder: Path::new("/p"),
    platform: Platform::MacOs,
    mcp: &mcp,
    skills: &skills,
  }
  .args()
  .expect("Unicode paths");
  let disabled = args
    .iter()
    .find(|arg| arg.starts_with("skills.config=["))
    .expect("skills disabled");
  for path in [
    "/p/.agents/skills/deploy/SKILL.md",
    "/home/me/.codex/skills/my-own/SKILL.md",
    "/home/me/.agents/skills/agents-own/SKILL.md",
    "/etc/codex/skills/company/SKILL.md",
  ] {
    assert!(
      disabled.contains(&format!("{{path=\"{path}\",enabled=false}}")),
      "{path}: {disabled}"
    );
  }
  assert!(
    !disabled.contains("imagegen"),
    "Codex's bundled skills stay: {disabled}"
  );
}
