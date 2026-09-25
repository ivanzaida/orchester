use std::{cell::RefCell, collections::HashMap, path::PathBuf};

use orchester_core::{
  command::{Command, CommandError, CommandOutput, CommandRunner},
  time::Timestamp,
};

/// Output captured from the real programs.
macro_rules! fixture {
  ($name:literal) => {
    include_str!(concat!("../fixtures/github/", $name))
  };
}
pub(crate) use fixture;

pub fn ok(stdout: &str) -> CommandOutput {
  CommandOutput {
    success: true,
    stdout: stdout.to_owned(),
    stderr: String::new(),
  }
}

pub fn failed(stderr: &str) -> CommandOutput {
  CommandOutput {
    success: false,
    stdout: String::new(),
    stderr: stderr.to_owned(),
  }
}

pub fn gh() -> PathBuf {
  std::env::current_dir().expect("current directory").join("gh.exe")
}

pub fn now() -> Timestamp {
  Timestamp::from_unix_millis(1_700_000_000_000)
}

/// Answers each argument list with a captured output and records every command it was asked to
/// run; anything unexpected was never allowed to run.
pub struct Replay {
  answers: HashMap<String, Result<CommandOutput, CommandError>>,
  ran: RefCell<Vec<Command>>,
}

impl Replay {
  pub fn new(answers: impl IntoIterator<Item = (&'static str, Result<CommandOutput, CommandError>)>) -> Self {
    Self {
      answers: answers
        .into_iter()
        .map(|(args, answer)| (args.to_owned(), answer))
        .collect(),
      ran: RefCell::new(Vec::new()),
    }
  }

  pub fn ran(&self) -> Vec<Command> {
    self.ran.borrow().clone()
  }
}

impl CommandRunner for Replay {
  fn run(&self, command: &Command) -> Result<CommandOutput, CommandError> {
    self.ran.borrow_mut().push(command.clone());
    let key = command.args().join(" ");
    self
      .answers
      .get(&key)
      .cloned()
      .unwrap_or_else(|| panic!("`{key}` is not one of the expected read-only commands"))
  }
}
