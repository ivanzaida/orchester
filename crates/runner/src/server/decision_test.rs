use orchester_ipc::{Command, Reply};

use super::{Answer, Later, Scheduler};

pub(super) fn answer(later: &Later<'_>, handle: Scheduler, command: &Command) -> Answer {
  let Command::TestDecisionConnection {
    test,
    connection,
    model,
    automation,
  } = command
  else {
    unreachable!("the server routes only decision tests here");
  };
  let (test, connection, model, automation) = (*test, connection.clone(), model.clone(), *automation);
  later.reply(async move {
    match handle
      .test_decision_connection(test, connection, model, automation)
      .await
    {
      Ok(_) => Reply::DecisionTested { test },
      Err(rejection) => Reply::Rejected(rejection),
    }
  })
}
