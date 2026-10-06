//! How a request is answered (`orchester_core::run::ApprovalAnswer`, "Approval choices and waiting
//! requests"): Orchester answers it itself when its server lists the tool to run without asking since
//! the agent started (Always allow), the step's own permissions run every part of the command on its
//! own (`orchester_core::run::permitted`), a rule of the run covers it (Approve similar, or a command Always
//! allowed during the run), or the run auto-approves and the request is not one auto-approval leaves to
//! a person. Otherwise a person answers, and a wider answer first leaves what it promises: a rule, a
//! profile version or a server setting, or the run's auto-approval. Each answer is recorded with who
//! gave it, delivered to the one call that asked, and marked in the activity; Deny tells the agent what
//! [`denial_message`] says and lets it continue.

use orchester_core::{
  connection::{mcp::InvocationExemptions, stream::AgentEvent},
  native::session::DENIED,
  orchester_tools::auto_approvable,
  permission::category::CommandCategory,
  run::{
    Answerer, ApprovalAnswer, ApprovalRuleId, ApprovalSubject, AutoVerdict, AutomationAnswerError, DecidedBy,
    PendingToolApproval, RuleScope, ToolApprovalDecision, ToolDecision, allowing_tool, answering, auto_approves,
    auto_verdict, codex_denial, denial_message, permitted, review::Review,
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

/// An answer about to be given: who gives it, the rule that does, a command's category for its mark,
/// and what a denial tells the agent.
pub(super) struct Settled {
  pub(super) decided_by: DecidedBy,
  pub(super) rule: Option<ApprovalRuleId>,
  pub(super) category: Option<CommandCategory>,
  /// A command that reads a credential location, which its mark names; only a person answers one.
  pub(super) secrets: bool,
  pub(super) told: Option<String>,
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
      review: None,
    }
  }
}

impl Worker {
  /// Orchester's own answer to `approval`, when something the person decided before covers it.
  pub(super) fn answer_itself(
    &self,
    watch: &Watch,
    approval: &PendingToolApproval,
  ) -> Result<Option<Settled>, StoreError> {
    let subject = ApprovalSubject::of(approval, &self.scope);
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
        rule: Some(rule.id),
        ..Settled::by(decided_by, category)
      }));
    }
    let profile = watch
      .invocation
      .permissions
      .as_ref()
      .is_some_and(|launch| launch.permissions().auto_approve());
    if auto_approves(watch.db.run_auto_approve(approval.run)?, profile)
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

  /// Leaves what `answer`, given `by` a person or automation, promises beyond this request, and returns
  /// how the request itself is answered; a rejection when the answer is not available for it.
  pub(super) fn widen(
    &self,
    watch: &mut Watch,
    approval: &PendingToolApproval,
    (answer, by): (&ApprovalAnswer, Answerer),
  ) -> Result<Result<Settled, Rejection>, StoreError> {
    let subject = ApprovalSubject::of(approval, &self.scope);
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
      ApprovalAnswer::Once => {}
      ApprovalAnswer::Deny => {
        // Claude Code hands the agent Orchester's message; Codex tells it in its own words; a native
        // agent gets the denial result its loop answers the call with.
        let told = match self.app_server() {
          Some(_) => codex_denial(approval.command).to_owned(),
          None if self.is_native() => DENIED.to_owned(),
          None => denial_message(approval, by),
        };
        return Ok(Ok(Settled {
          told: Some(told),
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
      ApprovalAnswer::Always { prefix } => {
        if let Err(rejection) = self.always_allow(watch, approval, &subject, prefix.as_ref())? {
          return Ok(Err(rejection));
        }
      }
    }
    Ok(Ok(settled))
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
      (ApprovalSubject::McpTool { server, tool }, None, _) => {
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
    if !watch
      .db
      .claim_tool_approval(run, &approval.id, settled.decided_by, Timestamp::now())?
    {
      return Ok(Err(refusal(&watch.db, run, &approval.id)?));
    }
    if let Some(rule) = settled.rule {
      watch.db.rule_answered(run, rule, &approval.id)?;
    }
    if settled.decided_by.answered_waiting() {
      self.notifier.send(Event::ToolApprovalChanged { run, task: self.task });
    }
    let item = awaiting.item().to_owned();
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
    Ok(Ok(()))
  }
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
