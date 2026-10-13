//! Signing an account in and checking it: the command a person runs in a terminal of their own, and what the CLI's
//! status command says of who is signed in, never a credential.

use std::path::{Path, PathBuf};

use orchester_core::{
  account::{
    AccountCheck, AccountCli, AccountIdentity, AccountState, Shell, SignInMethod, parse_claude_identity,
    sign_in_command,
  },
  command::{Command, CommandError, CommandOutput, CommandRunner},
  time::Timestamp,
};

fn home() -> PathBuf {
  if cfg!(windows) {
    PathBuf::from(r"C:\Users\ana")
  } else {
    PathBuf::from("/Users/ana")
  }
}

#[test]
fn accounts_sign_in_commands_set_the_folder_for_one_terminal() {
  let base = home().join("AppData").join("Local");
  let folder = base.join("Orchester").join("accounts").join("claude-freelance");
  let text = sign_in_command(
    AccountCli::ClaudeCode,
    Some(&folder),
    Shell::PowerShell,
    Some((&base, "$env:LOCALAPPDATA")),
  );
  let separator = std::path::MAIN_SEPARATOR;
  assert_eq!(
    text,
    format!(
      "$env:CLAUDE_CONFIG_DIR=\"$env:LOCALAPPDATA\\Orchester{separator}accounts{separator}claude-freelance\"; claude auth login"
    )
  );
  assert_eq!(
    sign_in_command(AccountCli::Codex, None, Shell::Zsh, None),
    "codex login"
  );
  let odd = sign_in_command(AccountCli::Codex, Some(Path::new("/a$b\"c")), Shell::Zsh, None);
  assert_eq!(odd, "CODEX_HOME='/a$b\"c' codex login");
  // PowerShell takes typographic quotes as quotes: none of them ends the literal.
  let hostile = sign_in_command(
    AccountCli::Codex,
    Some(Path::new("C:\\x\u{201D}; calc; \u{201C}\u{2019}$(calc)")),
    Shell::PowerShell,
    None,
  );
  assert_eq!(
    hostile,
    "$env:CODEX_HOME='C:\\x\u{201D}; calc; \u{201C}\u{2019}\u{2019}$(calc)'; codex login"
  );
  let based = sign_in_command(
    AccountCli::ClaudeCode,
    Some(&base.join("it's")),
    Shell::PowerShell,
    Some((&base, "$env:LOCALAPPDATA")),
  );
  assert_eq!(
    based,
    "$env:CLAUDE_CONFIG_DIR=$env:LOCALAPPDATA + '\\it''s'; claude auth login"
  );
  let bang = sign_in_command(
    AccountCli::Codex,
    Some(Path::new("/Users/ana/.codex-!!'x")),
    Shell::Zsh,
    None,
  );
  assert_eq!(bang, "CODEX_HOME='/Users/ana/.codex-!!'\\''x' codex login");
  let home_based = sign_in_command(
    AccountCli::Codex,
    Some(Path::new("/Users/ana/.codex-$work")),
    Shell::Zsh,
    Some((Path::new("/Users/ana"), "$HOME")),
  );
  assert_eq!(home_based, "CODEX_HOME=\"$HOME\"'/.codex-$work' codex login");
}

#[test]
fn accounts_claude_identity_is_read_where_reported_and_never_guessed() {
  let identity = parse_claude_identity(
    r#"{"loggedIn":true,"authMethod":"claude.ai","email":"ana@example.com","orgName":"Ana","subscriptionType":"max"}"#,
  );
  assert_eq!(identity.email.as_deref(), Some("ana@example.com"));
  assert_eq!(identity.plan.as_deref(), Some("max"));
  assert_eq!(identity.method, Some(SignInMethod::ClaudeAi));
  let signed_out = parse_claude_identity(r#"{"loggedIn":false,"authMethod":"none"}"#);
  assert_eq!(signed_out, AccountIdentity::default());
}

/// Answers `--version` and the status command as the CLI with the given status output would.
struct Cli {
  status: &'static str,
  code: i32,
}

impl CommandRunner for Cli {
  fn run(&self, command: &Command) -> Result<CommandOutput, CommandError> {
    let stdout = if command.args().first().map(String::as_str) == Some("--version") {
      "2.1.296 (Claude Code)".to_owned()
    } else {
      self.status.to_owned()
    };
    Ok(CommandOutput {
      code: Some(if stdout.contains("Claude Code") { 0 } else { self.code }),
      stdout,
      stderr: String::new(),
    })
  }
}

#[test]
fn accounts_a_check_reads_who_is_signed_in_with_the_clis_status_command() {
  let program = Path::new("claude");
  let signed = AccountCheck::run(
    AccountCli::ClaudeCode,
    Some(program),
    &Cli {
      status: r#"{"loggedIn":true,"email":"ana@example.com"}"#,
      code: 0,
    },
    &[],
    Timestamp::now(),
  );
  assert_eq!(signed.state(), AccountState::SignedIn);
  assert_eq!(
    signed.known_identity().and_then(|identity| identity.email.as_deref()),
    Some("ana@example.com")
  );
  let out = AccountCheck::run(
    AccountCli::ClaudeCode,
    Some(program),
    &Cli {
      status: r#"{"loggedIn":false}"#,
      code: 1,
    },
    &[],
    Timestamp::now(),
  );
  assert_eq!(out.state(), AccountState::NotSignedIn);
  let missing = AccountCheck::run(
    AccountCli::ClaudeCode,
    None,
    &Cli { status: "", code: 0 },
    &[],
    Timestamp::now(),
  );
  assert_eq!(missing.state(), AccountState::CliMissing);
}

/// A stand-in `codex` that says `status` to `codex login status`.
struct Codex(&'static str);

impl CommandRunner for Codex {
  fn run(&self, command: &Command) -> Result<CommandOutput, CommandError> {
    let version = command.args().first().map(String::as_str) == Some("--version");
    Ok(CommandOutput {
      code: Some(0),
      stdout: if version { "codex-cli 0.159.2" } else { self.0 }.to_owned(),
      stderr: String::new(),
    })
  }
}

#[test]
fn accounts_a_codex_api_key_login_keeps_only_that_it_is_an_api_key() {
  let check = |status| {
    AccountCheck::run(
      AccountCli::Codex,
      Some(Path::new("codex")),
      &Codex(status),
      &[],
      Timestamp::now(),
    )
  };
  let key = check("Logged in using an API key - sk-proj-***ABCD");
  assert_eq!(key.state(), AccountState::SignedIn);
  assert_eq!(
    key.known_identity().and_then(|identity| identity.method),
    Some(SignInMethod::ApiKey)
  );
  let stored = serde_json::to_string(&key.result).expect("serialized");
  assert!(!stored.contains("sk-") && !stored.contains("ABCD"), "{stored}");
  let unknown = check("Logged in using a token sk-abc");
  assert_eq!(unknown.known_identity().and_then(|identity| identity.method), None);
  assert_eq!(
    check("Logged in using ChatGPT")
      .known_identity()
      .and_then(|identity| identity.method),
    Some(SignInMethod::ChatGpt)
  );
  let old: AccountIdentity =
    serde_json::from_str(r#"{"email":null,"organization":null,"plan":null,"method":"an API key - sk-proj-***ABCD"}"#)
      .expect("an identity stored before");
  assert_eq!(
    old.method,
    Some(SignInMethod::ApiKey),
    "stored words are read as a method, never kept"
  );
}
