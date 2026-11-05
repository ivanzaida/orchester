//! What the person asks of the run's services: Stop, Restart (Start for one that stopped) and *Stop all*, each carried
//! out by the run's runner as `stop_service` and `start_service` are; and a service's address opened in a new tab of
//! the in-app Browser, which then shows it (`browser::BrowserLink`).

use std::{path::PathBuf, sync::Arc};

use lurq::app::{
  component::DevtoolsInspectable,
  ctx::{Ctx, FutureAction},
};
use orchester_core::id::RunId;
use orchester_ipc::{
  browser::{BrowserCommand, BrowserReply, BrowserTabId},
  services::ServiceCommand,
};
use orchester_workspace::{
  messages::{self, Message},
  tasks::run::control_message,
};

use crate::{browser::BrowserLink, services::Services};

/// A request about the run's services.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ServiceRequest {
  Stop(Arc<str>),
  /// Restart one that runs or crashed; Start one that stopped.
  Restart(Arc<str>),
  /// *Stop all*: each of these, one after another.
  StopAll(Arc<[Arc<str>]>),
}

impl DevtoolsInspectable for ServiceRequest {}

impl ServiceRequest {
  /// Whether it is under way for the service `name`.
  pub(super) fn touches(&self, name: &str) -> bool {
    match self {
      Self::Stop(service) | Self::Restart(service) => **service == *name,
      Self::StopAll(services) => services.iter().any(|service| **service == *name),
    }
  }
}

/// A request the runner did not carry out, the service it was about and why.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Refused {
  pub request: ServiceRequest,
  pub service: Arc<str>,
  pub why: Message,
}

impl DevtoolsInspectable for Refused {}

pub(super) type ServiceAction = FutureAction<ServiceRequest, ServiceRequest, Refused>;

/// The requests about the services of `run`, whose project is at `root`; made once, in the tab's `create`.
pub(super) fn service_action(ctx: &mut Ctx, root: PathBuf, run: RunId) -> ServiceAction {
  let runners = Services::of(ctx).runners;
  ctx.future_action(move |request: ServiceRequest| {
    let (runners, root) = (runners.clone(), root.clone());
    async move {
      let services: Vec<Arc<str>> = match &request {
        ServiceRequest::Stop(service) | ServiceRequest::Restart(service) => vec![service.clone()],
        ServiceRequest::StopAll(services) => services.to_vec(),
      };
      for service in services {
        let command = match request {
          ServiceRequest::Restart(_) => ServiceCommand::Restart {
            run,
            service: service.to_string(),
          },
          ServiceRequest::Stop(_) | ServiceRequest::StopAll(_) => ServiceCommand::Stop {
            run,
            service: service.to_string(),
          },
        };
        if let Err(error) = runners.services(&root, command).await {
          tracing::warn!(%error, %service, "the runner did not carry out a request about a service");
          return Err(Refused {
            request: request.clone(),
            service,
            why: control_message(&error),
          });
        }
      }
      Ok(request)
    }
  })
}

pub(super) type OpenAction = FutureAction<Arc<str>, BrowserTabId, Message>;

/// Opens an address in a new tab of the browser of the project at `root`, and shows it in the Browser window once the
/// tab is open; made once, in the tab's `create`.
pub(super) fn open_action(ctx: &mut Ctx, root: PathBuf) -> OpenAction {
  let runners = Services::of(ctx).runners;
  let shown = root.clone();
  let action = ctx.future_action(move |url: Arc<str>| {
    let (runners, root) = (runners.clone(), root.clone());
    async move {
      let open = BrowserCommand::OpenTab {
        url: Some(url.to_string()),
      };
      match runners.browser(&root, open).await {
        Ok(BrowserReply::TabOpened { tab }) => Ok(tab),
        Ok(other) => {
          tracing::error!(?other, "the browser answered a new tab with something else");
          Err(messages::interrupted())
        }
        Err(error) => {
          tracing::warn!(%error, "the browser did not open a service's address");
          Err(control_message(&error))
        }
      }
    }
  });
  let link = BrowserLink::of(ctx);
  ctx.watch(&action.state(), move |state| {
    if let (Some(link), Some(tab), false, None) = (&link, &state.data, state.is_pending(), &state.error) {
      link.show(&shown, Some(tab.clone()));
    }
  });
  action
}
