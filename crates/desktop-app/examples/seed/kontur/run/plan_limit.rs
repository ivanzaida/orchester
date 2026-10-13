//! A run at its account's plan limit, as the runner leaves it (`docs/design/agent-accounts.md`): stopped where the CLI
//! refused a turn of the step's agent, or, for a step whose account was already at its limit, before its agent
//! started.

use orchester_core::{
  account::AccountCli,
  connection::{
    AgentProvider,
    plan_limits::{Blocked, LimitBlock, LimitReading, LimitState, LimitWindow, PlanLimits},
  },
  run::RunLimit,
};

use super::{super::SeedError, Drive};

impl Drive<'_> {
  /// Stops at the plan limit of the account `step` runs on: its CLI reported the five-hour window used up, resetting
  /// two hours after the seed's now, and refused the turn, as the runner stops it.
  pub fn stop_at_plan_limit(&mut self, step: &str) -> Result<&mut Self, SeedError> {
    let execution = self.execution(step)?;
    if let Some(invocation) = self.open.get(&Self::key(step)) {
      let id = invocation.id;
      self
        .kontur
        .db
        .save_invocation_account(id, AccountCli::Codex.default_account())?;
    }
    self.stop_open()?;
    let at = self.tick();
    self.kontur.db.save_limit_snapshot(self.run.id(), &self.used_up(at))?;
    let events = self.run.stop_at_limit(RunLimit::PlanLimit { execution }, at)?;
    self.record(&events)?;
    Ok(self)
  }

  /// Opens `step` without starting its agent and stops there, as the runner does for a step whose account its CLI
  /// last reported at its plan limit (`XPjSc`): the run waits for the reset, holding no agent slot.
  pub fn wait_at_plan_limit(&mut self, step: &str) -> Result<&mut Self, SeedError> {
    let execution = self.execution(step)?;
    let at = self.tick();
    let events = self.run.start_step(execution, at)?;
    self.record(&events)?;
    self.stop_at_plan_limit(step)
  }

  /// What the CLI reported: the five-hour window used up, resetting two hours after the seed's now.
  fn used_up(&self, at: orchester_core::time::Timestamp) -> LimitReading {
    let window = LimitWindow::new(
      300,
      Some(100.0),
      Some(self.kontur.ago(-120)),
      LimitState::Reached,
      false,
    )
    .expect("a five-hour window used up is valid");
    LimitReading {
      provider: AgentProvider::CodexCli,
      account: None,
      at,
      limits: PlanLimits {
        windows: vec![window],
        plan: None,
        credits: None,
        blocked: Some(Blocked {
          reason: LimitBlock::Window,
          window: Some(300),
        }),
      },
    }
  }
}
