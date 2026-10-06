//! The transcript rows of a native agent (04.H10 `IcGvI`): file tools by their own glyph with the tool
//! first in the meta, a command with the shell it used, an approval as an entry of its own, a denial by
//! a person told apart from Orchester's refusal with the result the agent was given, and a compaction;
//! the invocation panel and header name the endpoint, vendor and effort the invocation recorded. The
//! transcript is seeded as the runner records it.

use orchester_core::{
  connection::{
    BaseUrl, Effort, EngineVendor,
    stream::{AgentEvent, CallDetail, CommandStatus, Declined, DeclinedBy, FileChange, FileChangeKind, NativeCall},
  },
  native::{session::DENIED, tools::ToolName},
  run::{CommandShell, DecidedBy, InvocationEngine, TokenUsage, ToolDecision},
  store::ActivityStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use crate::{github::shows, harness::Desktop, native_run::native_run, unreadable_run::at};

const REFUSED: &str = "the command cannot be split: use shell: true for `|`";

fn call(item: &str, tool: ToolName, detail: CallDetail, declined: Option<Declined>) -> AgentEvent {
  AgentEvent::NativeCall {
    item: item.to_owned(),
    call: NativeCall {
      tool,
      detail: Some(detail),
      declined,
    },
  }
}

fn started(item: &str, label: &str) -> AgentEvent {
  AgentEvent::CommandStarted {
    item: item.to_owned(),
    command: label.to_owned(),
  }
}

fn finished(item: &str, label: &str, output: &str, exit_code: Option<i64>) -> AgentEvent {
  AgentEvent::CommandFinished {
    item: item.to_owned(),
    command: label.to_owned(),
    output: output.to_owned(),
    exit_code,
    status: CommandStatus::Completed,
  }
}

fn decided(item: &str, decided_by: DecidedBy, told: Option<&str>, waited_ms: Option<u64>) -> AgentEvent {
  AgentEvent::ToolDecided {
    item: item.to_owned(),
    server: "command".to_owned(),
    action: "pnpm vitest run".to_owned(),
    decision: ToolDecision {
      decided_by,
      category: None,
      secrets: false,
      told: told.map(str::to_owned),
      waited_ms,
    },
  }
}

/// One group of tool calls, then a denial, a refusal and a compaction.
fn transcript() -> Vec<AgentEvent> {
  let shell = CallDetail::Command {
    shell: Some(CommandShell::Pwsh),
  };
  let read = CallDetail::Read {
    first: 1,
    last: 140,
    bytes: 4_915,
  };
  let searched = CallDetail::Searched {
    scope: Some("src/**".to_owned()),
    matches: 9,
    files: 4,
  };
  let changed = CallDetail::Changed {
    change: FileChangeKind::Modified,
    added: 38,
    removed: 6,
  };
  vec![
    AgentEvent::TurnStarted,
    started("call_1", "read_file src/switcher.ts:1-140"),
    call("call_1", ToolName::ReadFile, read, None),
    finished("call_1", "read_file src/switcher.ts:1-140", "lines 1–140 of 300", None),
    started("call_2", "search useKeymap src/**"),
    call("call_2", ToolName::Search, searched, None),
    finished("call_2", "search useKeymap src/**", "9 matches in 4 files", None),
    started("call_3", "edit_file src/switcher.ts"),
    call("call_3", ToolName::EditFile, changed, None),
    finished("call_3", "edit_file src/switcher.ts", "edited src/switcher.ts", None),
    AgentEvent::FileChanged {
      item: "call_3".to_owned(),
      changes: vec![FileChange {
        path: "src/switcher.ts".to_owned(),
        kind: FileChangeKind::Modified,
      }],
    },
    // A native agent reports usage after each model request.
    AgentEvent::TurnCompleted {
      usage: TokenUsage {
        input: 1_200,
        cached_input: 0,
        output: 80,
      },
    },
    decided("call_4", DecidedBy::Person, None, Some(48_000)),
    started("call_4", "pnpm vitest run"),
    call("call_4", ToolName::RunCommand, shell.clone(), None),
    finished("call_4", "pnpm vitest run", "Tests 12 passed", Some(0)),
    decided("call_5", DecidedBy::PersonDeny, Some(DENIED), Some(9_000)),
    call(
      "call_5",
      ToolName::RunCommand,
      shell,
      Some(Declined {
        by: DeclinedBy::Person,
        result: DENIED.to_owned(),
      }),
    ),
    AgentEvent::PermissionDenied {
      item: "call_5".to_owned(),
      command: "Remove-Item .vitest-cache -Recurse -Force".to_owned(),
    },
    call(
      "call_6",
      ToolName::RunCommand,
      CallDetail::Command { shell: None },
      Some(Declined {
        by: DeclinedBy::Orchester,
        result: REFUSED.to_owned(),
      }),
    ),
    AgentEvent::PermissionDenied {
      item: "call_6".to_owned(),
      command: "ls | wc".to_owned(),
    },
    AgentEvent::RunnerProblem {
      problem: orchester_core::connection::stream::RunnerProblem::ContextCompacted {
        before: 164_000,
        after: 12_000,
      },
    },
  ]
}

fn seeded() -> Desktop {
  Desktop::with_seeded_project(|repo| {
    let (_, invocation) = native_run(repo);
    let mut db = ProjectDb::open(repo).expect("open the project");
    let engine = InvocationEngine {
      endpoint: BaseUrl::new("https://gateway.kontur.lan/v1").expect("base URL"),
      vendor: EngineVendor::Host("gateway.kontur.lan".to_owned()),
      effort: Some(Effort::High),
    };
    db.save_invocation_engine(invocation.id, &engine).expect("engine");
    let events: Vec<(Timestamp, AgentEvent)> = transcript()
      .into_iter()
      .zip(13..)
      .map(|(event, second)| (at(second * 1_000), event))
      .collect();
    db.append_activity(&invocation, &events).expect("transcript");
  })
}

#[test]
fn native_rows_show_tool_calls_approvals_and_denials() {
  let mut desktop = seeded();
  desktop.wait_for("nav-tasks");
  desktop.press("nav-tasks");
  desktop.wait_for("task-row-KON-1");
  desktop.click("task-row-KON-1");
  desktop.wait_for("task-page");
  desktop.click("task-tab-conversation");
  desktop.wait_for("conversation-group-toggle");
  // The open group's rows push the step's header out of the virtualized list.
  assert!(shows(
    &mut desktop,
    "activity-step-header",
    "gateway.kontur.lan / deepseek-v4-pro · native"
  ));
  desktop.click("conversation-group-toggle");
  desktop.wait_until("the group opens", |desktop| {
    shows(desktop, "conversation", "read_file · lines 1–140 · 4.7 KiB")
  });
  for text in [
    "src/switcher.ts",
    "“useKeymap”",
    "search · src/** · 9 matches in 4 files",
    "edit_file · M · +38 −6 · open in Changes",
    "Approved by you · pnpm vitest run",
    "approve once · waited 48 s",
    "shell · pwsh",
    "Denied by a person · the agent continues",
    "run_command · shell · pwsh",
    "Given to the agent as the command’s result:",
    "refused by Orchester · the agent continues",
    "The conversation was shortened to fit the model’s context window",
    "about 164k tokens became about 12k",
  ] {
    assert!(shows(&mut desktop, "conversation", text), "{text}");
  }
  desktop.click("conversation-details");
  desktop.wait_for("activity-invocation");
  let panel = "activity-invocation";
  assert!(shows(&mut desktop, panel, "https://gateway.kontur.lan/v1"));
  assert!(!shows(&mut desktop, panel, "Endpoint now"), "the endpoint the run used");
  assert!(shows(&mut desktop, panel, "Effort"));
  assert!(shows(&mut desktop, panel, "high"));
}

#[test]
fn native_rows_leave_usage_out_of_the_tools_list() {
  let mut desktop = seeded();
  desktop.wait_for("nav-tasks");
  desktop.press("nav-tasks");
  desktop.wait_for("task-row-KON-1");
  desktop.click("task-row-KON-1");
  desktop.wait_for("task-page");
  desktop.click("task-tab-conversation");
  desktop.wait_for("conversation-show-2");
  desktop.click("conversation-show-2");
  desktop.wait_until("the tools", |desktop| {
    shows(desktop, "conversation", "edit_file · M · +38 −6 · open in Changes")
  });
  assert!(
    !shows(&mut desktop, "conversation", "Usage reported"),
    "the step's header and panel carry a native agent's usage"
  );
}
