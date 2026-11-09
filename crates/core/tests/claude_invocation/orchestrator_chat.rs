//! The Orchestrator chat's turn (`ClaudeChat`): read-only and only inside the project folder, with its own MCP
//! server's tools allowed and the folders that hold secrets denied by rule; a turn that loads the skill its message
//! carries (ORC-634); and what its system prompt tells it about links.

use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use orchester_core::{
  command::Command,
  connection::{
    AgentProvider, ApiProvider, BaseUrl,
    invocation::{ChatSkill, ClaudeChat, ClaudeSession, OnProvider},
  },
  id::{ProjectId, SecretId},
  orchestrator::{
    MAX_TURNS,
    instructions::{ProjectReads, system_prompt},
  },
  resource::{CommandEnvironment, ModelId, Slug},
  secret::SecretValue,
};

use super::SESSION;

fn chat_args(denied: &[PathBuf]) -> Vec<String> {
  chat_args_with(denied, &[])
}

fn chat_args_with(denied: &[PathBuf], denied_files: &[PathBuf]) -> Vec<String> {
  chat(denied, denied_files, None).args()
}

fn chat<'a>(denied: &'a [PathBuf], denied_files: &'a [PathBuf], skill: Option<ChatSkill<'a>>) -> ClaudeChat<'a> {
  ClaudeChat {
    executable: Path::new("C:/tools/claude.exe"),
    model: "haiku",
    effort: None,
    folder: Path::new("C:/work/kontur"),
    session: ClaudeSession::New(SESSION),
    system_prompt: Path::new("C:/tmp/prompt.md"),
    mcp_config: Path::new("C:/tmp/mcp.json"),
    denied,
    denied_files,
    skill,
  }
}

/// The values after `flag`, up to the next flag.
fn after<'a>(args: &'a [String], flag: &str) -> Vec<&'a str> {
  args
    .iter()
    .skip_while(|arg| *arg != flag)
    .skip(1)
    .take_while(|arg| !arg.starts_with("--"))
    .map(String::as_str)
    .collect()
}

/// The text of the first user message `command` gives the CLI.
fn first_message(command: &Command) -> String {
  let line = command.stdin().expect("the prompt goes on the input");
  let message: serde_json::Value = serde_json::from_str(line.trim()).expect("one stream-json line");
  message["message"]["content"][0]["text"]
    .as_str()
    .expect("a text block")
    .to_owned()
}

/// ORC-634: a message that carries a skill loads it as the CLI's own, from a plugin of that skill alone, and keeps
/// every other part of the isolation: no setting source (the person's own skills, plugins and hooks), no MCP server but
/// its own. Slash commands stay on, since `--disable-slash-commands` turns the plugin's skill off too.
#[test]
fn orchestrator_chat_loads_a_chosen_skill_from_its_plugin_and_keeps_the_isolation() {
  let slug = Slug::new("release-notes").expect("a slug");
  let skill = ChatSkill {
    plugin: Path::new("C:/work/kontur/.orchester/chat-skills/m1"),
    slug: &slug,
  };
  let with = chat(&[], &[], Some(skill));
  let args = with.args();
  assert_eq!(
    after(&args, "--plugin-dir"),
    ["C:/work/kontur/.orchester/chat-skills/m1"]
  );
  assert!(!args.iter().any(|arg| arg == "--disable-slash-commands"), "{args:?}");
  let at = args
    .iter()
    .position(|arg| arg == "--setting-sources")
    .expect("setting sources");
  assert_eq!(
    args[at + 1],
    "",
    "no settings file, so none of the person's own skills: {args:?}"
  );
  assert!(args.iter().any(|arg| arg == "--strict-mcp-config"));
  let command = with
    .command(("for v0.4", &[]), Vec::new(), &CommandEnvironment::default())
    .expect("a command");
  assert_eq!(first_message(&command), "/orchester:release-notes for v0.4");

  let without = chat(&[], &[], None);
  let args = without.args();
  assert!(args.iter().any(|arg| arg == "--disable-slash-commands"));
  assert!(!args.iter().any(|arg| arg == "--plugin-dir"));
  let command = without
    .command(("for v0.4", &[]), Vec::new(), &CommandEnvironment::default())
    .expect("a command");
  assert_eq!(first_message(&command), "for v0.4");
}

/// A compaction of a turn whose message carried a skill loads none: `/compact` is its only prompt.
#[test]
fn orchestrator_compaction_loads_no_skill() {
  let slug = Slug::new("release-notes").expect("a slug");
  let skill = ChatSkill {
    plugin: Path::new("C:/work/kontur/.orchester/chat-skills/m1"),
    slug: &slug,
  };
  let args = chat(&[], &[], Some(skill)).compaction_args();
  assert!(!args.iter().any(|arg| arg == "--plugin-dir"), "{args:?}");
  assert!(!args.iter().any(|arg| arg == "--disable-slash-commands"), "{args:?}");
}

#[test]
fn orchestrator_chat_reads_only_in_the_project_folder_and_never_in_secret_folders() {
  // The private folder as Windows may name it, short and long, and the Default account's own file.
  let denied = [
    PathBuf::from(r"C:\Users\ME~1\AppData\Local\Temp\orchester-mcp"),
    PathBuf::from(r"\\?\C:\Users\me\AppData\Local\Temp\orchester-mcp"),
    PathBuf::from(r"C:\Users\me\.claude"),
  ];
  let args = chat_args_with(&denied, &[PathBuf::from(r"C:\Users\me\.claude.json")]);
  assert_eq!(
    after(&args, "--settings"),
    [r#"{"permissions":{"blockReadsOutsideWorkingDirectories":true}}"#],
    "the CLI's own block on reads outside the working folder"
  );
  assert_eq!(after(&args, "--tools"), ["Read,Glob,Grep"]);
  // A bare `Read` would allow every path the person can read; without a rule a read outside the working folder
  // asks, which print mode denies.
  assert_eq!(after(&args, "--allowedTools"), ["mcp__orchester"]);
  assert_eq!(
    after(&args, "--disallowedTools"),
    [
      "Read(//c/Users/ME~1/AppData/Local/Temp/orchester-mcp/**)",
      "Read(//c/Users/me/AppData/Local/Temp/orchester-mcp/**)",
      "Read(//c/Users/me/.claude/**)",
      "Read(//c/Users/me/.claude.json)"
    ]
  );
  assert_eq!(after(&args, "--permission-mode"), ["default"]);
  assert_eq!(
    after(&args, "--max-turns"),
    [MAX_TURNS.to_string().as_str()],
    "one answer cannot run away"
  );
}

#[test]
fn orchestrator_chat_denies_unix_folders_and_skips_what_it_cannot_name() {
  let denied = [
    PathBuf::from("/Users/me/Library/Application Support/claude"),
    PathBuf::from(r"\\server\share\accounts"),
  ];
  let args = chat_args(&denied);
  assert_eq!(
    after(&args, "--disallowedTools"),
    ["Read(//Users/me/Library/Application Support/claude/**)"]
  );
  assert!(!chat_args(&[]).iter().any(|arg| arg == "--disallowedTools"));
}

#[test]
fn orchestrator_links_only_with_the_links_its_tools_returned() {
  let prompt = system_prompt("Kontur", ProjectId::generate(), ProjectReads::FileTools);
  assert!(
    prompt.contains("[KON-12](orchester:/project/<id>/tasks/KON-12)"),
    "it shows how a `link` becomes an `orchester:` link: {prompt}"
  );
  assert!(
    prompt.contains("Never write or change a route yourself"),
    "it never makes up a route: {prompt}"
  );
  assert!(
    prompt.contains("only with the `url` a tool returned"),
    "it links a web page only with a returned `url`: {prompt}"
  );
  assert!(
    prompt.contains(
      "A link is always the Markdown form [words](orchester:<link>): never put a link, a route or a key you link \
       in backticks or a code block, and never show a route bare."
    ),
    "it never writes a link as code or a route bare: {prompt}"
  );
  assert!(
    prompt.contains("`- Label: [words](orchester:<link>) — detail`"),
    "it writes how several things stand as labelled links: {prompt}"
  );
}

/// ORC-89: the chat adds and changes MCP servers at once while it drives and as cards otherwise, names secrets only,
/// and says where a program it suggests came from.
#[test]
fn orchestrator_prompt_proposes_mcp_servers_and_names_secrets_only() {
  for reads in [ProjectReads::FileTools, ProjectReads::GitTools] {
    let prompt = system_prompt("Kontur", ProjectId::generate(), reads);
    for rule in [
      "mcp_add_server, mcp_update_server, mcp_set_server_enabled and mcp_remove_server",
      "While Let the Orchestrator drive is on they are made at once by those rules",
      "is refused (person_only) unless a person turned on Let automation change agent permissions",
      "While it is off, each is a card the person applies",
      "never type, ask for or repeat a secret's value",
      "tell the person where it came from; never guess a path",
      "not on this computer's search path",
      "save_profile_draft, then publish_profile",
    ] {
      assert!(prompt.contains(rule), "{rule}: {prompt}");
    }
  }
}

/// ORC-96: the chat applies a proposal the person's message approves, asks when it does not say which, proposes anew
/// when the message changes it, and leaves what only a click applies to the card.
#[test]
fn orchestrator_prompt_applies_what_the_person_approves_in_the_chat() {
  for reads in [ProjectReads::FileTools, ProjectReads::GitTools] {
    let prompt = system_prompt("Kontur", ProjectId::generate(), reads);
    for rule in [
      "apply it for them with apply_proposal",
      "ask which one; never guess",
      "do not apply the old one: make a new proposal",
      "never apply it in the turn you made it",
      "(click_only): tell the person to apply it there",
      "In a turn Orchester started by itself nothing is approved",
    ] {
      assert!(prompt.contains(rule), "{rule}: {prompt}");
    }
  }
}

/// ORC-709: on an API provider, which lists no efforts, the effort the person typed in the runs-on picker reaches the
/// CLI as it is, and starting it on the provider keeps it.
#[test]
fn orchestrator_chat_on_an_api_provider_passes_a_typed_effort() {
  let mut typed = chat(&[], &[], None);
  typed.effort = Some("think-hard");
  let command = typed
    .command(("plan", &[]), Vec::new(), &CommandEnvironment::default())
    .expect("a command");
  let provider = ApiProvider::new(
    BaseUrl::new("https://api.deepseek.com/anthropic").expect("a URL"),
    SecretId::generate(),
    vec![ModelId::new("deepseek-v4-pro").expect("a model")],
  )
  .expect("a provider");
  let model = ModelId::new("deepseek-v4-pro").expect("a model");
  let on = OnProvider {
    provider: &provider,
    model: &model,
    key: Arc::new(SecretValue::new("sk-test-provider".to_owned()).expect("not blank")),
  };
  let started = on.apply(command, AgentProvider::ClaudeCodeCli);
  assert_eq!(after(started.args(), "--effort"), ["think-hard"]);
}
