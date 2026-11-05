//! The QA app as the built-in service `desktop-qa` (`orchester_core::orchester_tools::QA_SERVICE`), and the `ui_*`
//! tools: `start_service("desktop-qa")` asks and starts as `start_qa_app` on the tasks scenario does, its approval
//! naming the QA script, so it takes one of the service slots for QA apps the project's *QA apps at once* allows;
//! `stop_service`, `service_status` and `service_logs` of it stop it, say where it is and read its log; the `ui_*`
//! calls reach it through the `mcp` driver ([`super::super::ui`]).

use orchester_core::{
  orchester_tools::{MAX_ANSWER_BYTES, QaAppStart, ToolResult, ToolWork, UiCall, is_qa_service, ui::MAX_LOG_LINES},
  service::ServiceName,
};

use super::ToolHost;
use crate::execution::step::{qa_app::QaVia, ui::StepUi};

impl ToolHost {
  /// Whether `name` is the built-in `desktop-qa` this step is offered.
  pub(super) fn is_qa(&self, name: &ServiceName) -> bool {
    is_qa_service(name) && self.tools.offers_qa_service()
  }

  /// What the approval of a QA app's start names, `start` being the start looked at, which is kept so the start runs
  /// only the script this request names ([`Self::start_qa`]).
  pub(super) fn qa_target(&self, start: &QaAppStart) -> String {
    let approval = self.qa_approval(start);
    self.qa_scripts.lock().push((start.clone(), approval.script.clone()));
    approval.target()
  }

  /// Starts a QA app for `start`, asked for `via` a tool, running only the script its latest look named.
  pub(super) fn start_qa(&self, start: &QaAppStart, via: QaVia) -> ToolResult {
    let Some(qa) = &self.qa else {
      return ToolResult::unavailable("this step cannot start a QA app");
    };
    // The latest look at this start is what was asked about; earlier ones were denied or never run.
    let mut asked = self.qa_scripts.lock();
    let script = asked
      .iter()
      .rposition(|(looked, _)| looked == start)
      .map(|at| asked[at].1.clone());
    asked.retain(|(looked, _)| looked != start);
    drop(asked);
    qa.start(start, script.as_deref(), via)
  }

  /// The answer to `work` when it is a service tool's call of `desktop-qa` or a `ui_*` call; `None` for any other.
  pub(super) fn qa_work(&self, work: &ToolWork) -> Option<ToolResult> {
    Some(match work {
      ToolWork::QaApp(start) => self.start_qa(start, QaVia::Tool),
      ToolWork::StartService(name) if self.is_qa(name) => self.start_qa(&QaAppStart::of_service(), QaVia::Service),
      ToolWork::StopService(name) if self.is_qa(name) => self.qa.as_ref().map_or_else(no_qa, |qa| qa.stop()),
      ToolWork::ServiceStatus(name) if self.is_qa(name) => self.qa.as_ref().map_or_else(no_qa, |qa| qa.status()),
      ToolWork::ServiceLogs { name, tail, .. } if self.is_qa(name) => self.ui().qa_logs((*tail).min(MAX_LOG_LINES)),
      ToolWork::Ui(call) => self.ui_call(call),
      _ => return None,
    })
  }

  fn ui_call(&self, call: &UiCall) -> ToolResult {
    let answer = self.ui().run(call);
    if answer.text().len() > MAX_ANSWER_BYTES {
      return ToolResult::unavailable(
        "the UI's answer exceeds the relay's answer limit; narrow it with ui_snapshot(query) or fewer log lines",
      );
    }
    answer
  }

  fn ui(&self) -> StepUi<'_> {
    StepUi {
      processes: &self.processes,
      folders: (&self.worktree, &self.artifacts),
      qa: self.qa.as_ref(),
      root: &self.repository,
      opened: &self.ui_opened,
    }
  }
}

fn no_qa() -> ToolResult {
  ToolResult::unavailable("this step cannot start a QA app")
}
