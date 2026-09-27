//! Command tools of the test projects: `orchester-test-command` (or another program) with fixed
//! arguments, published so tool steps can run them.

use std::collections::BTreeSet;

use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{CommandAction, CommandArg, CommandProgram, CommandTimeout, ExitCodes, OutputLimit, Slug, Tool},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

pub const TEST_COMMAND: &str = env!("CARGO_BIN_EXE_orchester-test-command");

/// A command tool: `program` (the test command unless set) with `args`.
#[derive(Clone, Copy, Debug)]
pub struct ToolSpec {
  pub program: &'static str,
  pub args: &'static [&'static str],
  pub timeout_secs: u64,
  pub output_kib: u32,
  pub exit_codes: &'static [i64],
}

impl ToolSpec {
  /// The test command with `args`, a minute to finish, exit code 0 and the default output limit.
  pub const fn test_command(args: &'static [&'static str]) -> Self {
    Self {
      program: TEST_COMMAND,
      args,
      timeout_secs: 60,
      output_kib: 256,
      exit_codes: &[0],
    }
  }

  pub const fn program(mut self, program: &'static str) -> Self {
    self.program = program;
    self
  }

  pub const fn timeout(mut self, secs: u64) -> Self {
    self.timeout_secs = secs;
    self
  }

  pub const fn output_kib(mut self, kib: u32) -> Self {
    self.output_kib = kib;
    self
  }

  fn tool(&self) -> Tool {
    let action = CommandAction::new(
      CommandProgram::new(self.program).expect("valid program"),
      self
        .args
        .iter()
        .map(|arg| CommandArg::new(arg).expect("valid argument"))
        .collect(),
    )
    .with_timeout(CommandTimeout::from_secs(self.timeout_secs).expect("valid timeout"))
    .with_exit_codes(ExitCodes::new(self.exit_codes.iter().copied().collect::<BTreeSet<_>>()).expect("valid codes"))
    .with_output_limit(OutputLimit::from_kib(self.output_kib).expect("valid limit"));
    Tool::command("", action)
  }
}

/// Creates and publishes the tool of step `step`, named after it.
pub fn publish(db: &mut ProjectDb, step: &str, spec: &ToolSpec) -> ResourceId {
  let now = Timestamp::now();
  let slug = Slug::from_name(step).expect("a step name with letters");
  let id = db
    .create_resource(slug, Name::new(step).expect("valid name"), &spec.tool(), now)
    .expect("create the tool")
    .id();
  db.publish_draft::<Tool>(id, now).expect("publish the tool");
  id
}
