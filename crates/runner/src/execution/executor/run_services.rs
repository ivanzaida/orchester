//! The task page's Services tab's requests about a run's services ([`ServiceCommand`], ORC-265): read at once, stops
//! and restarts carried out off the async workers, as ending and starting a service's tree waits for its processes.

use orchester_ipc::{
  Rejection,
  services::{MAX_SHOWN_LINES, ServiceCommand, ServiceReply},
};

use super::step::ShownServiceError;
use crate::execution::Scheduler;

impl Scheduler {
  /// Carries out the Services tab's `command`.
  pub async fn services(&self, command: ServiceCommand) -> Result<ServiceReply, Rejection> {
    let services = std::sync::Arc::clone(&self.inner.services);
    match command {
      ServiceCommand::List { run } => Ok(ServiceReply::Listed {
        run,
        services: services.of_run(run),
      }),
      ServiceCommand::Log {
        run,
        service,
        since,
        tail,
      } => {
        let (lines, last) = services
          .log_of(run, &service, (since, tail.min(MAX_SHOWN_LINES)))
          .map_err(|error| refused(&error))?;
        Ok(ServiceReply::Log { lines, last })
      }
      ServiceCommand::Stop { run, service } => {
        blocking(move || services.stop_of(run, &service)).await?;
        Ok(ServiceReply::Done)
      }
      ServiceCommand::Restart { run, service } => {
        blocking(move || services.restart_of(run, &service)).await?;
        Ok(ServiceReply::Done)
      }
    }
  }
}

/// Runs `work` on a blocking thread.
async fn blocking(work: impl FnOnce() -> Result<(), ShownServiceError> + Send + 'static) -> Result<(), Rejection> {
  match tokio::task::spawn_blocking(work).await {
    Ok(done) => done.map_err(|error| refused(&error)),
    Err(error) => Err(Rejection::Failed {
      detail: format!("the request about the service ended without an answer: {error}"),
    }),
  }
}

fn refused(error: &ShownServiceError) -> Rejection {
  Rejection::Failed {
    detail: error.to_string(),
  }
}
