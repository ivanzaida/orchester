//! `continue_run`'s raise of a team run's limits (OA-34): the team server is asked first ([`TeamWork::raise_limits`]),
//! within the ceiling its Admin set in the workspace policy, and only a raise it keeps goes on to the runner. With no
//! ceiling the limits stay the Admin's (`decision_required` D6, as before); a raise beyond it is
//! `raise_beyond_ceiling`.

use orchester_api::v2::{AuthorityErrorCode, DecisionId, LimitRaise};
use orchester_core::id::RunId;

use super::{decision_required, of_run, on_worker, outcome::authority_refused};
use crate::{
  app_mcp::{Services, error::ToolError},
  messages::Message,
  team_work::{RaiseAnswer, TeamWork, TeamWorkError},
};

/// Asks the team server of `project` to raise team run `run`'s limits by `raise`; fine once it keeps the raise.
pub(in crate::app_mcp::tools) async fn raise_limits(
  services: &Services,
  (project, run): (&str, RunId),
  raise: LimitRaise,
) -> Result<(), ToolError> {
  let team = of_run(services, (project, &run.to_string())).await?.ok_or_else(|| {
    ToolError::refused(
      "team_action_failed",
      vec![Message::new("appMcp.errors.teamActionFailed")],
    )
  })?;
  let dispatcher = team.dispatcher(services).await?;
  let answer = on_worker(move || async move {
    TeamWork::new(dispatcher, ())
      .raise_limits(run, raise)
      .await
      .map_err(raise_failed)
  })
  .await?;
  match answer {
    RaiseAnswer::Raised { .. } => Ok(()),
    RaiseAnswer::Unreachable => Err(ToolError::refused(
      "team_unavailable",
      vec![Message::new("appMcp.errors.teamUnavailable")],
    )),
    RaiseAnswer::Refused(error) => Err(match error.code() {
      AuthorityErrorCode::DecisionRequired => decision_required(DecisionId::D6),
      AuthorityErrorCode::Invalid => beyond_ceiling(),
      _ => authority_refused(&error),
    }),
  }
}

/// A raise the team's ceiling does not allow: too many raises of the run, a limit raised by more than its share, or
/// lifted altogether. Nothing was raised.
pub(in crate::app_mcp::tools) fn beyond_ceiling() -> ToolError {
  ToolError::refused(
    "raise_beyond_ceiling",
    vec![Message::new("appMcp.errors.teamRaiseBeyondCeiling")],
  )
}

fn raise_failed(error: TeamWorkError) -> ToolError {
  match error {
    TeamWorkError::Store(error) => error.into(),
    error => {
      tracing::warn!(%error, "a team run's raise could not be asked for");
      ToolError::refused(
        "team_action_failed",
        vec![Message::new("appMcp.errors.teamActionFailed")],
      )
    }
  }
}
