//! A tool step running a command tool (M10a): what of the program's output is kept, how its end
//! decides the step, what the transcript records and what the step leaves for the steps after it.
//! The program itself is started and ended by the runner.

use std::{collections::VecDeque, time::Duration};

use super::{
  result::{StepOutcome, StepOutput},
  status::StepFailure,
};
use crate::{
  connection::stream::{AgentEvent, InvocationOutcome, ToolCommandResult},
  id::{RunId, StepExecutionId},
  resource::{CommandAction, OutputLimit, StepKey},
  time::Timestamp,
};

/// Output lines a step's summary repeats for the steps after it, and the longest line it repeats.
const SUMMARY_LINES: usize = 20;
const SUMMARY_LINE_CHARS: usize = 300;
/// What comes between how the program ended and the output's last lines in a step's summary.
const LAST_LINES: &str = "\n\nThe last lines of its output:\n\n";

/// The end of a program's output, both streams in the order the lines arrived, within the tool's
/// [`OutputLimit`]: once more arrives, the oldest lines are dropped and counted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutputCapture {
  limit: usize,
  lines: VecDeque<String>,
  /// Bytes of the kept lines, each with its line break.
  kept: usize,
  omitted: u64,
}

/// What an [`OutputCapture`] kept.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CapturedOutput {
  /// The kept lines, one per line.
  pub text: String,
  /// Bytes of output before `text` that were not kept.
  pub omitted_bytes: u64,
}

impl OutputCapture {
  pub fn new(limit: OutputLimit) -> Self {
    Self {
      limit: limit.bytes(),
      lines: VecDeque::new(),
      kept: 0,
      omitted: 0,
    }
  }

  /// Keeps `line` (without its line break). A line longer than the limit keeps its end.
  pub fn push(&mut self, line: &str) {
    let mut line = line;
    let whole = line.len() + 1;
    if whole > self.limit {
      let mut start = whole - self.limit;
      while !line.is_char_boundary(start) {
        start += 1;
      }
      self.omitted += start as u64;
      line = &line[start..];
    }
    let size = line.len() + 1;
    while self.kept + size > self.limit {
      let Some(dropped) = self.lines.pop_front() else {
        break;
      };
      self.kept -= dropped.len() + 1;
      self.omitted += dropped.len() as u64 + 1;
    }
    self.kept += size;
    self.lines.push_back(line.to_owned());
  }

  pub fn finish(self) -> CapturedOutput {
    CapturedOutput {
      text: self.lines.into_iter().collect::<Vec<_>>().join("\n"),
      omitted_bytes: self.omitted,
    }
  }
}

/// How a command tool's program ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandEnd {
  /// It exited on its own; `None` when the system reported no exit code.
  Exited { code: Option<i32> },
  /// It did not finish within the tool's timeout and was ended with everything it started.
  TimedOut,
  /// The run ended it.
  Stopped,
}

/// One run of a command tool's program for a tool step, from its start to its end.
#[derive(Clone, Copy, Debug)]
pub struct ToolCommand<'a> {
  /// The tool's name.
  pub tool: &'a str,
  pub action: &'a CommandAction,
}

impl ToolCommand<'_> {
  pub fn started(&self) -> AgentEvent {
    AgentEvent::ToolCommandStarted {
      tool: self.tool.to_owned(),
      command: self.action.command_line(),
    }
  }

  pub fn result(&self, end: CommandEnd) -> ToolCommandResult {
    match end {
      CommandEnd::Exited { code: Some(code) } if self.action.exit_codes().contains(i64::from(code)) => {
        ToolCommandResult::Passed
      }
      CommandEnd::Exited { .. } => ToolCommandResult::Failed,
      CommandEnd::TimedOut => ToolCommandResult::TimedOut,
      CommandEnd::Stopped => ToolCommandResult::Stopped,
    }
  }

  pub fn finished(&self, end: CommandEnd, output: &CapturedOutput) -> AgentEvent {
    AgentEvent::ToolCommandFinished {
      tool: self.tool.to_owned(),
      command: self.action.command_line(),
      result: self.result(end),
      exit_code: exit_code(end),
      output: output.text.clone(),
      omitted_bytes: output.omitted_bytes,
    }
  }

  /// How the invocation that ran the program ended.
  pub fn outcome(&self, end: CommandEnd) -> InvocationOutcome {
    match (self.result(end), end) {
      (ToolCommandResult::Passed, _) => InvocationOutcome::Completed,
      (ToolCommandResult::TimedOut, _) => InvocationOutcome::TimedOut,
      (ToolCommandResult::Stopped, _) => InvocationOutcome::Terminated,
      (ToolCommandResult::Failed, CommandEnd::Exited { code }) => InvocationOutcome::ExitFailed { code },
      (ToolCommandResult::Failed, _) => InvocationOutcome::ExitFailed { code: None },
    }
  }

  /// Why the step fails; `None` when it passes, or when the run ended the program and decides.
  pub fn failure(&self, end: CommandEnd) -> Option<StepFailure> {
    match self.result(end) {
      ToolCommandResult::Failed | ToolCommandResult::TimedOut => Some(StepFailure::CommandFailed),
      ToolCommandResult::Passed | ToolCommandResult::Stopped => None,
    }
  }

  /// What the step leaves for the steps after it: how the program ended and the last lines of its
  /// output. `None` for a program the run ended, whose step did not finish.
  pub fn output(&self, at: StepAt, end: CommandEnd, elapsed: Duration, output: &CapturedOutput) -> Option<StepOutput> {
    let outcome = match self.result(end) {
      ToolCommandResult::Passed => StepOutcome::Done,
      ToolCommandResult::Failed | ToolCommandResult::TimedOut => StepOutcome::Failed,
      ToolCommandResult::Stopped => return None,
    };
    let (tool, command) = (self.tool, self.action.command_line());
    let seconds = elapsed.as_secs_f64();
    let mut text = match (end, exit_code(end)) {
      (CommandEnd::TimedOut, _) => format!(
        "{tool} ran `{command}`, which did not finish within its timeout of {} s and was ended.",
        self.action.timeout().secs()
      ),
      (_, Some(code)) => {
        let expected = if outcome == StepOutcome::Done {
          "a code the tool expects"
        } else {
          "a code the tool does not expect"
        };
        format!("{tool} ran `{command}`, which exited with code {code} after {seconds:.1} s, {expected}.")
      }
      (_, None) => format!("{tool} ran `{command}`, which ended after {seconds:.1} s without an exit code."),
    };
    text.push_str(&last_lines(output));
    Some(StepOutput {
      run: at.run,
      execution: at.execution,
      step: at.step,
      outcome: Some(outcome),
      text,
      unverified: Vec::new(),
      created_at: at.now,
    })
  }
}

/// Where a step output belongs.
#[derive(Clone, Debug)]
pub struct StepAt {
  pub run: RunId,
  pub execution: StepExecutionId,
  pub step: StepKey,
  pub now: Timestamp,
}

fn exit_code(end: CommandEnd) -> Option<i64> {
  match end {
    CommandEnd::Exited { code: Some(code) } => Some(i64::from(code)),
    _ => None,
  }
}

fn last_lines(output: &CapturedOutput) -> String {
  let lines: Vec<&str> = output.text.lines().collect();
  if lines.iter().all(|line| line.trim().is_empty()) {
    return "\n\nIt printed nothing.".to_owned();
  }
  let start = lines.len().saturating_sub(SUMMARY_LINES);
  let shown: Vec<String> = lines[start..]
    .iter()
    .map(|line| line.chars().take(SUMMARY_LINE_CHARS).collect())
    .collect();
  format!("{LAST_LINES}{}", shown.join("\n"))
}

/// A tool step's report `text` read as Markdown: how its program ended as written, and the last lines of its output
/// in a code block, since a program's output is not Markdown. A report without output lines is returned as it is.
pub fn tool_report_markdown(text: &str) -> String {
  let Some((ended, output)) = text.split_once(LAST_LINES) else {
    return text.to_owned();
  };
  // A fence longer than any run of backticks in the output, so no line of it closes the block.
  let longest = output
    .split(|character| character != '`')
    .map(str::len)
    .max()
    .unwrap_or_default();
  let fence = "`".repeat(longest.max(2) + 1);
  format!("{ended}{LAST_LINES}{fence}\n{output}\n{fence}")
}
