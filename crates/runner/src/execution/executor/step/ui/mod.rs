//! The `ui_*` tools of one invocation (`orchester_core::orchester_tools::ui`): `ui_open(service)` picks a service the
//! step started, and the other calls reach its UI through the service's driver until another is opened. The `mcp`
//! driver reaches the built-in `desktop-qa` service, the invocation's QA app ([`super::qa_app`]), over the run's relay
//! to the app (`super::run_app`), as `call_run_app` without a pid reaches it, so the same rules hold: only the app this
//! run started, through its exact discovery file, its token kept private and its PNGs saved in the run's own storage.
//! The `native` driver ([`native`]) reaches a project's service through the operating system's accessibility
//! interface (UI Automation, macOS Accessibility), only in the windows of its processes the run may reach; on macOS
//! each of its calls first answers a missing Accessibility or Screen Recording permission. The `browser` driver
//! ([`browser`]) opens a web service in a tab of the run's own in Orchester's Browser, which reaches only the origins
//! of the services the step started; the `cdp` driver attaches to an Electron or WebView2 app's own windows through
//! its debugging port, only when a process of the service's own tree listens on it, and not on macOS. A project's
//! service with another driver, or none, answers why it cannot be opened.

mod browser;
mod native;

use std::{
  path::Path,
  thread,
  time::{Duration, Instant},
};

use orchester_core::{
  orchester_tools::{
    AppCall, QA_SERVICE, QA_SERVICE_DRIVER, ToolResult, UiCall, is_qa_service,
    ui::{
      UiCondition,
      mcp::{self, UiAnswer, WAIT_POLL_MS},
    },
  },
  service::{ServiceName, UiDriver},
};
use orchester_exec::{native_ui, run_processes::RunProcesses};
use parking_lot::Mutex;
use serde_json::{Value, json};

pub(in crate::execution::executor::step) use self::browser::{AppTabs, WebTabs};
use self::{
  browser::{AppUi, WebUi},
  native::NativeUi,
};
use super::{
  qa_app::StepQa,
  run_app,
  service::{StepServices, published_services},
};

/// What the `ui_*` calls of one invocation reach.
pub(in crate::execution::executor::step) struct StepUi<'a> {
  pub(in crate::execution::executor::step) processes: &'a RunProcesses,
  /// The worktree, and the run's own storage the screenshots go to.
  pub(in crate::execution::executor::step) folders: (&'a Path, &'a Path),
  /// The invocation's QA app, the `desktop-qa` service.
  pub(in crate::execution::executor::step) qa: Option<&'a StepQa>,
  /// The invocation's services, which the `native`, `browser` and `cdp` drivers reach.
  pub(in crate::execution::executor::step) services: Option<&'a StepServices>,
  /// The `browser` driver's tabs of the invocation.
  pub(in crate::execution::executor::step) web: &'a WebTabs,
  /// The apps the `cdp` driver attached to in the invocation.
  pub(in crate::execution::executor::step) apps: &'a AppTabs,
  /// The project's repository, whose published services say their drivers.
  pub(in crate::execution::executor::step) root: &'a Path,
  /// The service `ui_open` opened last, with its driver.
  pub(in crate::execution::executor::step) opened: &'a Mutex<Option<(ServiceName, UiDriver)>>,
}

impl StepUi<'_> {
  /// The answer to `call`.
  pub(in crate::execution::executor::step) fn run(&self, call: &UiCall) -> ToolResult {
    let answered = match call {
      UiCall::Open(name) => self.open(name, call),
      _ => self.on_opened(call),
    };
    match answered {
      Ok(answer) => ToolResult::shown(answer.to_string()),
      Err(why) => ToolResult::unavailable(&why),
    }
  }

  /// The latest `lines` log lines of the QA app, as `service_logs("desktop-qa")` answers them.
  pub(in crate::execution::executor::step) fn qa_logs(&self, lines: u32) -> ToolResult {
    let call = UiCall::Logs { lines, filter: None };
    match self.once(&call) {
      Ok(UiAnswer::Done(answer)) => ToolResult::shown(answer.to_string()),
      Ok(UiAnswer::NotYet) => ToolResult::shown(json!({"lines": []}).to_string()),
      Err(why) => ToolResult::unavailable(&why),
    }
  }

  /// The `native` driver of the project's services; why it cannot work on this computer now, such as the macOS
  /// permission it misses, which every call of it answers.
  fn native(&self) -> Result<NativeUi<'_>, String> {
    native_ui::ready().map_err(|error| error.to_string())?;
    Ok(NativeUi {
      processes: self.processes,
      services: self.services()?,
      artifacts: self.folders.1,
    })
  }

  /// The `browser` driver of the project's services.
  fn web(&self) -> Result<WebUi<'_>, String> {
    Ok(WebUi {
      tabs: self.web,
      services: self.services()?,
      artifacts: self.folders.1,
    })
  }

  /// The `cdp` driver of the project's services.
  fn app(&self) -> Result<AppUi<'_>, String> {
    Ok(AppUi {
      apps: self.apps,
      processes: self.processes,
      services: self.services()?,
      artifacts: self.folders.1,
    })
  }

  fn services(&self) -> Result<&StepServices, String> {
    self.services.ok_or_else(|| "this step has no services".to_owned())
  }

  /// `ui_open(name)`: the service's windows or page once it is the one the later calls reach.
  fn open(&self, name: &ServiceName, call: &UiCall) -> Result<Value, String> {
    if !is_qa_service(name) {
      let driver = self.driver(name)?;
      let answer = match driver {
        UiDriver::Native => self.native()?.open(name)?,
        UiDriver::Browser => self.web()?.open(name)?,
        UiDriver::Cdp => self.app()?.open(name)?,
        UiDriver::None | UiDriver::Mcp => return Err(Self::not_driven(name, driver)),
      };
      *self.opened.lock() = Some((name.clone(), driver));
      return Ok(answer);
    }
    let pid = self
      .qa
      .ok_or_else(|| "this step cannot start a QA app".to_owned())?
      .target(None)
      .map_err(|_| {
        format!(
          "the service `{QA_SERVICE}` is not running in this step: start it with start_service(\"{QA_SERVICE}\") first"
        )
      })?
      .map(|target| target.pid);
    let UiAnswer::Done(mut answer) = self.once(call)? else {
      return Err("the app answered no windows".to_owned());
    };
    *self.opened.lock() = Some((name.clone(), QA_SERVICE_DRIVER));
    answer["service"] = json!(name.as_str());
    answer["driver"] = json!(driver_name(QA_SERVICE_DRIVER));
    answer["pid"] = json!(pid);
    Ok(answer)
  }

  /// A call on the service `ui_open` opened, through its driver; refused before one was. Only web content — a web
  /// page, an Electron or WebView2 app's window — waits for a selector, a text or an address.
  fn on_opened(&self, call: &UiCall) -> Result<Value, String> {
    let opened = self.opened.lock().clone();
    let Some((opened, driver)) = opened else {
      return Err("no service's UI is open in this step: call ui_open(service) after starting it".to_owned());
    };
    if let UiCall::Wait { condition, .. } = call
      && condition.is_page()
      && !matches!(driver, UiDriver::Browser | UiDriver::Cdp)
    {
      return Err(format!(
        "the {} driver waits for idle or element: {} waits in web content, which the browser and cdp drivers open",
        driver_name(driver),
        condition.as_str()
      ));
    }
    match driver {
      UiDriver::Browser => return self.web()?.run(&opened, call),
      UiDriver::Cdp => return self.app()?.run(&opened, call),
      UiDriver::Native => return self.native()?.run(&opened, call),
      UiDriver::Mcp | UiDriver::None => {}
    }
    match call {
      UiCall::Wait {
        condition: UiCondition::Element(query),
        timeout_ms,
      } => self.wait_for(call, query, *timeout_ms),
      _ => match self.once(call)? {
        UiAnswer::Done(answer) => Ok(answer),
        UiAnswer::NotYet => Err("the app's answer says nothing yet".to_owned()),
      },
    }
  }

  /// Looks at the UI until the element `query` names is shown, at most `timeout_ms`.
  fn wait_for(&self, call: &UiCall, query: &str, timeout_ms: u32) -> Result<Value, String> {
    let deadline = Instant::now() + Duration::from_millis(u64::from(timeout_ms));
    loop {
      if let UiAnswer::Done(answer) = self.once(call)? {
        return Ok(answer);
      }
      if Instant::now() >= deadline {
        return Err(format!("no element holding `{query}` was shown within {timeout_ms} ms"));
      }
      thread::sleep(Duration::from_millis(WAIT_POLL_MS));
    }
  }

  /// One request of `call` to the QA app through the `mcp` driver, and what its answer comes to.
  fn once(&self, call: &UiCall) -> Result<UiAnswer, String> {
    let request = mcp::request(call);
    let app = AppCall::of_qa_app(request.tool, request.arguments);
    let result = run_app::result(self.processes, self.folders, &app, self.qa)?;
    mcp::answer(call, &result)
  }

  /// The driver the project's published service `name` has; why it cannot be told.
  fn driver(&self, name: &ServiceName) -> Result<UiDriver, String> {
    published_services(self.root)
      .map_err(|error| format!("the project's services cannot be read: {error}"))?
      .iter()
      .find(|service| service.name() == name)
      .map(|service| service.ui_driver())
      .ok_or_else(|| format!("the project publishes no service named `{name}`"))
  }

  /// Why the project's service `name`, with `driver`, cannot be opened here: it has no UI, or it is not one the step
  /// can reach.
  fn not_driven(name: &ServiceName, driver: UiDriver) -> String {
    match driver {
      UiDriver::None => format!("the service `{name}` has no UI driver: read it with service_logs and its URL"),
      UiDriver::Mcp => format!(
        "the mcp driver reaches only the built-in `{QA_SERVICE}` service yet, not the project's service `{name}`"
      ),
      UiDriver::Native | UiDriver::Browser | UiDriver::Cdp => {
        format!("the service `{name}` uses the {} UI driver", driver_name(driver))
      }
    }
  }
}

/// A driver's name, as the answers and the project's services say it.
const fn driver_name(driver: UiDriver) -> &'static str {
  match driver {
    UiDriver::None => "none",
    UiDriver::Native => "native",
    UiDriver::Mcp => "mcp",
    UiDriver::Browser => "browser",
    UiDriver::Cdp => "cdp",
  }
}
