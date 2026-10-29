//! How a request is answered (`orchester_core::run::ApprovalAnswer`, "Approval choices and waiting
//! requests"): Orchester answers it itself when its server lists the tool to run without asking since
//! the agent started (Always allow), the step's own permissions run every part of the command on its
//! own (`orchester_core::run::permitted`), a rule of the run covers it (Approve similar, or a command Always
//! allowed during the run), a rule of the project does (Approve for every run of this project, `start_qa_app` only),
//! or the run auto-approves (its own choice, a profile, or auto mode's *approve everything in each run*) and the
//! request is not one auto-approval leaves to a person. Otherwise a person answers, and a wider answer first leaves
//! what it promises: a rule, a
//! profile version or a server setting, or the run's auto-approval. Each answer is recorded with who
//! gave it, delivered to the one call that asked, and marked in the activity; Deny tells the agent what
//! [`denial_message`] says and lets it continue. A denial with a message tells it the denier's words
//! ([`denial_with_message`]): Claude Code and Orchester's tools as the denial's reason; Codex, whose decline
//! carries no words (0.159), as the next input of its turn, sent with the decline. A native agent's denial
//! carries no words yet, so it is refused with one.

use orchester_core::{
  connection::{mcp::InvocationExemptions, stream::AgentEvent},
  native::session::DENIED,
  orchester_tools::auto_approvable,
  permission::category::CommandCategory,
  resource::ApprovalMode,
  run::{
    Answerer, ApprovalAnswer, ApprovalRuleId, ApprovalSubject, AutoVerdict, AutomationAnswerError, DecidedBy,
    DenialNote, PendingToolApproval, ProjectRuleId, ProjectScope, RuleScope, ToolApprovalDecision, ToolDecision,
    ToolReach, allowing_tool, answering, answering_in_project, auto_approves, auto_verdict, codex_denial,
    denial_message, denial_with_message, permitted, review::Review,
  },
  store::McpServerStore,
  time::Timestamp,
};
use orchester_ipc::{Event, Rejection};
use orchester_store::StoreError;

use super::{
  super::tool_approval::refusal,
  approval::Awaiting,
  worker::{Watch, Worker},
};

/// The rule that gives an answer: one of the run's, or one of the project's.
#[derive(Clone, Copy, Debug)]
pub(super) enum Answering {
  Run(ApprovalRuleId),
  Project(ProjectRuleId),
}

/// An answer about to be given: who gives it, the rule that does, a command's category for its mark,
/// and what a denial tells the agent.
pub(super) struct Settled {
  pub(super) decided_by: DecidedBy,
  pub(super) rule: Option<Answering>,
  pub(super) category: Option<CommandCategory>,
  /// A command that reads a credential location, which its mark names; only a person answers one.
  pub(super) secrets: bool,
  pub(super) told: Option<String>,
  /// The denier's words, kept with the decision.
  pub(super) note: Option<DenialNote>,
  /// What Codex gets as the next input of its turn, as its decline tells its model nothing of the note.
  pub(super) follow_up: Option<String>,
  /// What auto mode did with the request before it was answered.
  pub(super) review: Option<Review>,
}

impl Settled {
  pub(super) const fn by(decided_by: DecidedBy, category: Option<CommandCategory>) -> Self {
    Self {
      decided_by,
      rule: None,
      category,
      secrets: false,
      told: None,
      note: None,
      follow_up: None,
      review: None,
    }
  }
}

/// What a denial tells the agent through the call that asked, and what follows it as the next input of a Codex turn.
struct Told {
  told: String,
  follow_up: Option<String>,
}

impl Worker {
  /// Orchester's own answer to `approval`, when something the person decided before covers it.
  pub(super) fn answer_itself(
    &self,
    watch: &Watch,
    approval: &PendingToolApproval,
  ) -> Result<Option<Settled>, StoreError> {
    let subject = ApprovalSubject::of(approval, &self.scope).with_checks(&self.checks);
    let category = category_of(&subject);
    if self.listed_since_start(watch, approval)? {
      return Ok(Some(Settled::by(DecidedBy::AlwaysAllowed, None)));
    }
    if let Some(launch) = &watch.invocation.permissions
      && let Some(category) = permitted(&subject, launch.permissions())
    {
      return Ok(Some(Settled::by(DecidedBy::Permissions, Some(category))));
    }
    let rules = watch.db.approval_rules(approval.run)?;
    if let Some(rule) = answering(&rules, &subject) {
      let decided_by = if rule.scope.revocable() {
        DecidedBy::SimilarRule
      } else {
        DecidedBy::AlwaysAllowed
      };
      return Ok(Some(Settled {
        rule: Some(Answering::Run(rule.id)),
        ..Settled::by(decided_by, category)
      }));
    }
    if let Some(rule) = answering_in_project(&watch.db.project_approval_rules()?, &subject) {
      return Ok(Some(Settled {
        rule: Some(Answering::Project(rule.id)),
        ..Settled::by(DecidedBy::SimilarRule, category)
      }));
    }
    let profile = watch
      .invocation
      .permissions
      .as_ref()
      .is_some_and(|launch| launch.permissions().auto_approve());
    // Auto mode's *approve everything in each run* answers as Approve everything in this run would, read again at
    // each request: turning it or the run's auto mode off stops it from the next one.
    let everything = watch
      .db
      .approve_everything()?
      .covers(watch.db.run_auto_mode(approval.run)?);
    if auto_approves(watch.db.run_auto_approve(approval.run)?, profile || everything)
      && (approval.command || auto_approvable(&approval.server, &approval.action))
      && let AutoVerdict::Approve(category) = auto_verdict(&subject)
    {
      return Ok(Some(Settled::by(DecidedBy::AutoApproved, category)));
    }
    Ok(None)
  }

  /// Whether `approval`'s MCP tool is listed to run without asking now, although the agent's CLI did not
  /// know it when it started: Always allow answered for the rest of the invocation. A Claude Code tool
  /// whose name another server of the project could have is not, as its CLI would not run it unasked.
  fn listed_since_start(&self, watch: &Watch, approval: &PendingToolApproval) -> Result<bool, StoreError> {
    if approval.command {
      return Ok(false);
    }
    let servers = watch.db.mcp_servers()?;
    let exemptions = InvocationExemptions::new(
      servers
        .iter()
        .map(|server| (server.name().clone(), server.exempt_tools().clone())),
    );
    Ok(match self.app_server() {
      Some(_) => exemptions.exempts(&approval.server, &approval.action),
      None => exemptions
        .claude_tool(&format!("mcp__{}__{}", approval.server, approval.action))
        .is_some(),
    })
  }

  /// Leaves what `answer`, given `by` a person or automation, promises beyond this request, which waits in
  /// `awaiting`, and returns how the request itself is answered; a rejection when the answer is not available for it.
  pub(super) fn widen(
    &self,
    watch: &mut Watch,
    (approval, awaiting): (&PendingToolApproval, &Awaiting),
    (answer, by): (&ApprovalAnswer, Answerer),
  ) -> Result<Result<Settled, Rejection>, StoreError> {
    let subject = ApprovalSubject::of(approval, &self.scope).with_checks(&self.checks);
    let decided_by = match answer.given_by(by, &subject) {
      Ok(decided_by) => decided_by,
      Err(AutomationAnswerError::Wider) => return Ok(Err(Rejection::AutomationWider)),
      Err(AutomationAnswerError::PersonOnly(ask)) => return Ok(Err(Rejection::PersonOnly { ask })),
    };
    let settled = Settled {
      secrets: matches!(&subject, ApprovalSubject::Command(command) if command.reads_secrets()),
      review: watch.db.tool_approval_review(approval.run, &approval.id)?,
      ..Settled::by(decided_by, category_of(&subject))
    };
    let now = Timestamp::now();
    match answer {
      // A check its access rules approve once per run: this approval covers its later calls in the run.
      ApprovalAnswer::Once if self.once_per_run(&subject) => {
        if let Ok(scope) = RuleScope::similar(&subject) {
          watch.db.add_approval_rule(approval.run, &scope, now)?;
        }
      }
      ApprovalAnswer::Once => {}
      ApprovalAnswer::Deny | ApprovalAnswer::DenyWithMessage { .. } => {
        let note = answer.denial_note();
        let Some(Told { told, follow_up }) = self.told(approval, awaiting, (by, note)) else {
          return Ok(Err(unavailable("Deny with a message")));
        };
        return Ok(Ok(Settled {
          told: Some(told),
          note: note.cloned(),
          follow_up,
          ..settled
        }));
      }
      ApprovalAnswer::ApproveRun => watch.db.set_run_auto_approve(approval.run, Some(true))?,
      ApprovalAnswer::Similar => {
        let Ok(scope) = RuleScope::similar(&subject) else {
          return Ok(Err(unavailable("Approve similar")));
        };
        watch.db.add_approval_rule(approval.run, &scope, now)?;
      }
      ApprovalAnswer::SimilarProject => {
        let Some(scope) = ProjectScope::of(&subject) else {
          return Ok(Err(unavailable("Approve for every run of this project")));
        };
        watch
          .db
          .add_project_approval_rule(&scope, (approval.run, &approval.id), now)?;
      }
      ApprovalAnswer::Always { prefix } => {
        if let Err(rejection) = self.always_allow(watch, approval, &subject, prefix.as_ref())? {
          return Ok(Err(rejection));
        }
      }
    }
    Ok(Ok(settled))
  }

  /// What denying `approval`, which waits in `awaiting`, `by` a person or automation tells the agent, with `note` when
  /// it carries the denier's words: Claude Code and Orchester's tools get Orchester's words, Codex its own and the
  /// note as its turn's next input, a native agent the denial result its loop answers the call with. `None` for a
  /// note to a native agent, whose denial carries no words yet.
  fn told(
    &self,
    approval: &PendingToolApproval,
    awaiting: &Awaiting,
    (by, note): (Answerer, Option<&DenialNote>),
  ) -> Option<Told> {
    let told = |told: String| Told { told, follow_up: None };
    match (awaiting, note) {
      (Awaiting::Native(_), Some(_)) => None,
      (Awaiting::Native(_), None) => Some(told(DENIED.to_owned())),
      (Awaiting::Codex(_), note) => Some(Told {
        told: codex_denial(approval.command).to_owned(),
        follow_up: note.map(|note| denial_with_message(approval, by, note)),
      }),
      (Awaiting::Claude { .. } | Awaiting::Tool { .. }, Some(note)) => {
        Some(told(denial_with_message(approval, by, note)))
      }
      // A call of Orchester's tools refused to Codex reads as Codex's own refusal of an MCP call.
      (Awaiting::Tool { .. }, None) if self.app_server().is_some() => {
        Some(told(codex_denial(approval.command).to_owned()))
      }
      (Awaiting::Claude { .. } | Awaiting::Tool { .. }, None) => Some(told(denial_message(approval, by))),
    }
  }

  /// Whether `subject` runs a check its access rules approve once per run.
  fn once_per_run(&self, subject: &ApprovalSubject) -> bool {
    let ApprovalSubject::McpTool {
      reach: ToolReach::Check { name, .. },
      ..
    } = subject
    else {
      return false;
    };
    self
      .checks
      .iter()
      .any(|check| check.name() == name && check.approval() == ApprovalMode::OncePerRun)
  }

  /// Always allow: a command's prefix is published in its profile and kept by the run; an MCP tool is
  /// listed on its server.
  fn always_allow(
    &self,
    watch: &mut Watch,
    approval: &PendingToolApproval,
    subject: &ApprovalSubject,
    prefix: Option<&orchester_core::permission::CommandPrefix>,
  ) -> Result<Result<(), Rejection>, StoreError> {
    match (subject, prefix, self.profile) {
      (ApprovalSubject::Command(command), Some(prefix), Some(profile)) => {
        match watch
          .db
          .publish_allowed_command(profile, command, prefix, Timestamp::now())
        {
          Ok(_) => {}
          Err(StoreError::AlwaysAllow(_)) => return Ok(Err(unavailable("Always allow"))),
          Err(error) => return Err(error),
        }
        let scope = RuleScope::AlwaysCommand { prefix: prefix.clone() };
        watch.db.add_approval_rule(approval.run, &scope, Timestamp::now())?;
      }
      (ApprovalSubject::McpTool { server, tool, .. }, None, _) => {
        let mut servers = watch.db.mcp_servers()?;
        let Some(stored) = servers.iter_mut().find(|stored| stored.name().as_str() == server) else {
          return Ok(Err(unavailable("Always allow")));
        };
        let Ok(tools) = allowing_tool(stored.exempt_tools(), tool) else {
          return Ok(Err(unavailable("Always allow")));
        };
        stored.set_exempt_tools(tools);
        watch.db.save_mcp_server(stored)?;
      }
      _ => return Ok(Err(unavailable("Always allow"))),
    }
    Ok(Ok(()))
  }

  /// Records `settled` as the answer to `approval`, hands it to the call waiting in `awaiting` and marks
  /// it in the activity.
  pub(super) async fn settle(
    &mut self,
    watch: &mut Watch,
    approval: &PendingToolApproval,
    awaiting: Awaiting,
    settled: Settled,
  ) -> Result<Result<(), Rejection>, StoreError> {
    let run = approval.run;
    // A review still under way is no longer wanted: dropping it stops the reviewer.
    self.reviewing = None;
    if !claim(watch, approval, &settled)? {
      return Ok(Err(refusal(&watch.db, run, &approval.id)?));
    }
    if settled.decided_by.answered_waiting() {
      self.notifier.send(Event::ToolApprovalChanged { run, task: self.task });
    }
    let item = awaiting.item().to_owned();
    // The answer settled: a QA start it lets run keeps its app, unless another wait for a person remains.
    self.resume_qa(watch, Some(&approval.id));
    if let Err(error) = self.deliver(watch, awaiting, &settled).await {
      watch.db.interrupt_tool_approvals(run, watch.invocation.id)?;
      self.end_approval_wait(watch)?;
      self.observe(
        watch,
        AgentEvent::TurnFailed {
          message: format!("The {} session closed before the tool decision arrived", self.cli()),
        },
      )?;
      self.driver.end();
      return Ok(Err(Rejection::Failed {
        detail: format!("cannot send the tool decision to {}: {error}", self.cli()),
      }));
    }
    let kind = if settled.decided_by.approves() {
      ToolApprovalDecision::Approved
    } else {
      ToolApprovalDecision::Denied
    };
    if !watch.db.delivered_tool_approval(run, &approval.id, kind)? {
      watch.db.interrupt_tool_approvals(run, watch.invocation.id)?;
      self.end_approval_wait(watch)?;
      self.observe(
        watch,
        AgentEvent::TurnFailed {
          message: "The tool decision could not be recorded after delivery".to_owned(),
        },
      )?;
      self.driver.end();
      return Ok(Err(Rejection::Failed {
        detail: "the tool decision could not be recorded after delivery".to_owned(),
      }));
    }
    self.end_approval_wait(watch)?;
    let waited = Timestamp::now().unix_millis() - approval.requested_at.unix_millis();
    let decision = ToolDecision {
      decided_by: settled.decided_by,
      category: settled.category,
      secrets: settled.secrets,
      told: settled.told,
      // A clock set back while it waited reads as no wait at all.
      waited_ms: settled
        .decided_by
        .by_person()
        .then(|| u64::try_from(waited).unwrap_or(0)),
      review: settled.review,
    };
    self.observe(
      watch,
      AgentEvent::ToolDecided {
        item,
        server: approval.server.clone(),
        action: approval.action.clone(),
        decision,
      },
    )?;
    // A decision is progress: a yielded cell that waited on it can now be collected.
    watch.idle_continuations = 0;
    self.continue_after_tools(watch).await?;
    Ok(Ok(()))
  }
}

/// Takes `approval` from the waiting as `settled` answers it, with the rule that answered and a denial's words;
/// whether it still waited.
fn claim(watch: &mut Watch, approval: &PendingToolApproval, settled: &Settled) -> Result<bool, StoreError> {
  let run = approval.run;
  if !watch
    .db
    .claim_tool_approval(run, &approval.id, settled.decided_by, Timestamp::now())?
  {
    return Ok(false);
  }
  match settled.rule {
    Some(Answering::Run(rule)) => watch.db.rule_answered(run, rule, &approval.id)?,
    Some(Answering::Project(rule)) => watch.db.project_rule_answered(rule)?,
    None => {}
  }
  if let Some(note) = &settled.note {
    watch.db.record_denial_note(run, &approval.id, note)?;
  }
  Ok(true)
}

fn category_of(subject: &ApprovalSubject) -> Option<CommandCategory> {
  match subject {
    ApprovalSubject::Command(command) => Some(command.category()),
    ApprovalSubject::McpTool { .. } => None,
  }
}

fn unavailable(answer: &str) -> Rejection {
  Rejection::Failed {
    detail: format!("{answer} is not available for this request"),
  }
}
