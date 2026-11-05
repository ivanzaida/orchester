//! The steps of a test pipeline graph.

use super::tools;

/// A step of a test pipeline graph; what its agent does is in its instructions.
#[derive(Clone, Debug)]
pub struct StepSpec {
  pub key: &'static str,
  pub name: &'static str,
  pub instructions: &'static str,
  pub after: &'static [&'static str],
  /// Where a failure returns to, and how often.
  pub retry: Option<(&'static str, u32)>,
  pub budget: Option<u64>,
  /// Its result waits for a plan decision.
  pub gate: bool,
  /// A tool step running this command tool instead of an agent step.
  pub tool: Option<tools::ToolSpec>,
  /// A Test step, which runs the project's test suites, instead of an agent step.
  pub tests: bool,
  /// The agent step's own choice of whether it merges the base when it moves; `None` follows the default rule.
  pub takes_base_moves: Option<bool>,
}

impl StepSpec {
  pub const fn new(
    key: &'static str,
    name: &'static str,
    instructions: &'static str,
    after: &'static [&'static str],
  ) -> Self {
    Self {
      key,
      name,
      instructions,
      after,
      retry: None,
      budget: None,
      gate: false,
      tool: None,
      tests: false,
      takes_base_moves: None,
    }
  }

  pub const fn gate(mut self) -> Self {
    self.gate = true;
    self
  }

  /// A tool step running `tool`.
  pub const fn command(mut self, tool: tools::ToolSpec) -> Self {
    self.tool = Some(tool);
    self
  }

  /// A Test step.
  pub const fn tests(mut self) -> Self {
    self.tests = true;
    self
  }

  pub const fn retry(mut self, to: &'static str, times: u32) -> Self {
    self.retry = Some((to, times));
    self
  }

  pub const fn budget(mut self, tokens: u64) -> Self {
    self.budget = Some(tokens);
    self
  }

  pub const fn takes_base_moves(mut self, takes: bool) -> Self {
    self.takes_base_moves = Some(takes);
    self
  }
}
