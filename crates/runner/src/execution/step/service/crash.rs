//! Crashes of ready services (`docs/architecture/agent-testing.md`, "Service"; card `FwHrv`): once an invocation keeps
//! a service, a watcher of its own looks at the services it keeps every [`LOOK`] until the invocation ends. A kept
//! service whose program exited on its own crashed: the kept services that need it, also through others, are stopped,
//! as they cannot work without it; its output is read to its end; and the crash ([`ServiceCrash`]) is queued as a
//! notice that the answer of the agent's next Orchester tool call carries ([`StepServices::take_notices`]), as well as
//! recorded in the run's conversation for the person ([`CrashReport`]).

use std::{
  collections::BTreeMap,
  sync::{Arc, Weak, atomic::Ordering},
  thread,
  time::{Duration, Instant},
};

use orchester_core::service::{ServiceCrash, ServiceName, ServicePhase};

use super::{InvocationServices, Running, StepServices, readiness::SharedLog, stop_aside};

/// How often the watcher looks at the invocation's services.
const LOOK: Duration = Duration::from_millis(200);

/// Records a crash in the run's conversation.
pub(in crate::execution::step) type CrashReport = Arc<dyn Fn(&ServiceCrash) + Send + Sync>;

/// A service seen to have exited, taken out of the services the invocation keeps.
struct Exited {
  name: ServiceName,
  code: Option<i32>,
  service: Running,
  log: SharedLog,
  started: Instant,
  /// The services stopped because they need it.
  stopped: Vec<ServiceName>,
}

impl StepServices {
  /// Starts the watcher of the invocation's crashes, unless it runs already.
  pub(super) fn watch_crashes(&self) {
    if self.state.watching.swap(true, Ordering::AcqRel) {
      return;
    }
    let state = Arc::downgrade(&self.state);
    let report = Arc::clone(&self.crashed);
    thread::spawn(move || watch(&state, report.as_ref()));
  }
}

/// Looks for crashes of `state`'s services until its invocation ended or was forgotten, and tells each one.
fn watch(state: &Weak<InvocationServices>, report: &(dyn Fn(&ServiceCrash) + Send + Sync)) {
  loop {
    thread::sleep(LOOK);
    let Some(state) = state.upgrade() else {
      return;
    };
    if state.ended() {
      return;
    }
    for crash in state.crashes() {
      state.notices.lock().push(crash.notice());
      report(&crash);
    }
  }
}

impl InvocationServices {
  /// The kept services that exited on their own since the last look, each with what its output said last.
  fn crashes(&self) -> Vec<ServiceCrash> {
    self
      .take_exited()
      .into_iter()
      .map(|exited| {
        let uptime = exited.started.elapsed();
        exited.service.output.settle();
        // Stops what its program left running and frees its port, off the async workers as the watcher is.
        drop(exited.service);
        let log = exited.log.lock();
        ServiceCrash::new(&exited.name, (exited.code, uptime), &log, &exited.stopped)
      })
      .collect()
  }

  /// Takes the kept services that exited out of those the invocation keeps, and stops the ones that need them.
  fn take_exited(&self) -> Vec<Exited> {
    let mut running = self.running.lock();
    let exited: Vec<(ServiceName, Option<i32>)> = running
      .iter_mut()
      .filter_map(|(name, service)| match service.exit() {
        Ok(exit) => exit.map(|code| (name.clone(), code)),
        Err(why) => {
          tracing::warn!(service = %name, why, "a service cannot be watched for a crash");
          None
        }
      })
      .collect();
    let mut taken = Vec::new();
    for (name, code) in exited {
      // A service that needs one that crashed in the same look was stopped with it.
      let Some(service) = running.remove(&name) else {
        continue;
      };
      let stopped = needing(&running, &name);
      let needs_it: BTreeMap<ServiceName, Running> =
        stopped.iter().filter_map(|other| running.remove_entry(other)).collect();
      self.note(&name, ServicePhase::Stopped, Some(code));
      for other in &stopped {
        self.note_stopped_for(other, &name);
      }
      stop_aside(needs_it);
      let watched = self.watched.lock();
      let Some(watch) = watched.get(&name) else {
        continue;
      };
      taken.push(Exited {
        name,
        code,
        service,
        log: Arc::clone(&watch.log),
        started: watch.started,
        stopped,
      });
    }
    taken
  }
}

/// The services of `running` that need `crashed`, also through others, in the order they are found.
fn needing(running: &BTreeMap<ServiceName, Running>, crashed: &ServiceName) -> Vec<ServiceName> {
  let mut gone = vec![crashed.clone()];
  loop {
    let more: Vec<ServiceName> = running
      .iter()
      .filter(|(name, service)| !gone.contains(name) && service.needs.iter().any(|need| gone.contains(need)))
      .map(|(name, _)| name.clone())
      .collect();
    if more.is_empty() {
      break;
    }
    gone.extend(more);
  }
  gone.split_off(1)
}
