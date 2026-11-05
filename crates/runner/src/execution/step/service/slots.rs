//! *Services at once* (`docs/architecture/agent-testing.md`, decision 3 and "Limits and load"): every running service
//! takes one of the computer's service slots, a stack as many as it starts services, and a test suite's own process one
//! while it runs. A start takes all the slots it needs before it starts anything, or none.
//!
//! Two limits hold at once, both read at each try: the computer's (`orchester_core::limits::ComputerServiceSlots`, in
//! the app's settings) and the project's share of it (`orchester_core::limits::ServiceSlots`, with the project's
//! limits). As with the QA slots (`super::super::qa_app`), a slot is an OS lock on a file of the user's data folder,
//! which every runner takes the same way and the OS frees when its holder closes the file or ends, so a runner that
//! died keeps none: `slot-<n>.lock` for the computer's, `projects/<project>/slot-<n>.lock` for a project's share. A
//! slot is both: a start takes one lock of each per service. Locks no limit reaches are never made.
//!
//! A start that cannot get every slot it needs waits, and says so once ([`Waiting`]). The starts of one project's
//! runner that wait are served in the order they began to: only the earliest tries, so a later one that needs fewer
//! slots does not pass it. Between projects, whoever tries first when slots are freed takes them. A start that needs
//! more slots than its project may ever use is refused at once rather than waiting for ever.

use std::{
  collections::VecDeque,
  fs::{self, File, OpenOptions, TryLockError},
  io,
  num::NonZeroU32,
  path::{Path, PathBuf},
  thread,
  time::{Duration, Instant},
};

use orchester_core::{
  id::ProjectId,
  limits::{ComputerServiceSlots, ServiceSlots as Share},
  store::SettingsStore,
};
use orchester_store::{AppDb, ProjectDb};
use parking_lot::Mutex;

/// How often a start that waits for service slots tries again.
const SLOT_POLL: Duration = Duration::from_millis(200);

/// Where the computer's service slots are, where its limit is read, and the starts of this runner that wait for them.
pub struct ServiceSlots {
  /// `None` on a platform without a user data folder: no service can start there yet.
  folder: Option<PathBuf>,
  /// The application database that holds the computer's *Services at once*; `None`, or a file not made yet, is the
  /// default.
  settings: Option<PathBuf>,
  /// The starts that wait, earliest first, by their tickets.
  queue: Mutex<Queue>,
}

#[derive(Default)]
struct Queue {
  next: u64,
  waiting: VecDeque<u64>,
}

/// A held service slot: one lock of the computer's and one of its project's share. Dropping it frees both.
pub(in crate::execution::step) struct ServiceSlot {
  _computer: File,
  _project: File,
}

/// What a start that waits says once: how many slots it needs, and the limits it waits under.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::execution::step) struct Waiting {
  pub(in crate::execution::step) slots: u32,
  pub(in crate::execution::step) share: u32,
  pub(in crate::execution::step) limit: u32,
}

/// What a start that got its slots waited for, when it waited.
#[derive(Clone, Copy, Debug)]
pub(in crate::execution::step) struct Waited {
  pub(in crate::execution::step) slots: u32,
  pub(in crate::execution::step) took: Duration,
}

impl Waited {
  /// What a start's answer says of the wait.
  pub(in crate::execution::step) fn answer(self) -> serde_json::Value {
    serde_json::json!({
      "slots": self.slots,
      "secs": (self.took.as_secs_f64() * 10.0).round() / 10.0,
    })
  }
}

/// Who asks for slots: the project, its repository whose database holds its share, how many slots it holds already,
/// why it may not take them any more (its invocation ended or waits for a person), and what it says once it waits.
pub(in crate::execution::step) struct Asker<'a> {
  pub(in crate::execution::step) project: ProjectId,
  pub(in crate::execution::step) root: &'a Path,
  pub(in crate::execution::step) holding: u32,
  pub(in crate::execution::step) gate: &'a dyn Fn() -> Result<(), String>,
  pub(in crate::execution::step) waiting: &'a dyn Fn(Waiting),
}

impl ServiceSlots {
  /// The computer's slots, in the user's data folder, under the limit of the application database beside the runner,
  /// which the app that started it keeps.
  pub fn machine() -> Self {
    let settings = orchester_store::location::app_data_location()
      .map(|location| location.database)
      .inspect_err(|error| tracing::warn!(%error, "the application database is unknown; Services at once is 8"))
      .ok();
    Self {
      folder: super::super::run_app::user_data().map(|data| data.join("orchester-service-slots")),
      settings,
      queue: Mutex::default(),
    }
  }

  /// Slots in `folder`, shared only by the runners given the same folder, under the limit the application database
  /// `settings` keeps.
  pub fn in_folder(folder: &Path, settings: &Path) -> Self {
    Self {
      folder: Some(folder.to_path_buf()),
      settings: Some(settings.to_path_buf()),
      queue: Mutex::default(),
    }
  }

  /// `count` slots for `asker`, waiting in order while too few are free; refused when the asker's gate refuses
  /// meanwhile, or when its project may never use as many. How long it waited, when it did.
  pub(in crate::execution::step) fn take(
    &self,
    count: u32,
    asker: &Asker<'_>,
  ) -> Result<(Vec<ServiceSlot>, Option<Waited>), String> {
    if count == 0 {
      return Ok((Vec::new(), None));
    }
    let folder = self.folder.as_deref().ok_or_else(|| {
      "services are not supported on this platform yet: it has no user data folder for the service slots".to_owned()
    })?;
    let ticket = self.enqueue();
    let began = Instant::now();
    let taken = self.wait_for(folder, (count, ticket), asker);
    self.queue.lock().waiting.retain(|waiting| *waiting != ticket);
    let (slots, said) = taken?;
    Ok((
      slots,
      said.then(|| Waited {
        slots: count,
        took: began.elapsed(),
      }),
    ))
  }

  fn enqueue(&self) -> u64 {
    let mut queue = self.queue.lock();
    let ticket = queue.next;
    queue.next += 1;
    queue.waiting.push_back(ticket);
    ticket
  }

  /// Tries for `count` slots each [`SLOT_POLL`] while `ticket` is the earliest waiting; the slots, and whether it said
  /// it waits.
  fn wait_for(
    &self,
    folder: &Path,
    (count, ticket): (u32, u64),
    asker: &Asker<'_>,
  ) -> Result<(Vec<ServiceSlot>, bool), String> {
    let mut said = false;
    loop {
      (asker.gate)()?;
      let (share, limit) = self.limits(asker.root);
      let usable = share.within(limit).get();
      // What the asker holds is freed only once it goes on, so it never gets more than the rest.
      if count.saturating_add(asker.holding) > usable {
        return Err(format!(
          "this start needs {count} service slots and this step holds {} already, but the project may run only \
           {usable} services at once (its share is {}, the computer's Services at once {}); stop services, start \
           fewer, or raise the limits",
          asker.holding,
          share.get(),
          limit.get()
        ));
      }
      let first = self.queue.lock().waiting.front() == Some(&ticket);
      if first
        && let Some(slots) =
          try_take(folder, &asker.project, count, (usable, limit.get())).map_err(|error| cannot(&error))?
      {
        return Ok((slots, said));
      }
      if !said {
        (asker.waiting)(Waiting {
          slots: count,
          share: share.get().get(),
          limit: limit.get().get(),
        });
        said = true;
      }
      thread::sleep(SLOT_POLL);
    }
  }

  /// The project's share as stored now and the computer's limit; the fewest, logged, when either cannot be read.
  fn limits(&self, root: &Path) -> (Share, ComputerServiceSlots) {
    let share = match ProjectDb::open_read_only(root).and_then(|db| db.project_limits()) {
      Ok(limits) => limits.service_slots(),
      Err(error) => {
        tracing::warn!(%error, "the project's share of Services at once cannot be read; this start uses one slot");
        Share::new(Share::MIN.into()).expect("the fewest service slots are in range")
      }
    };
    (share, self.computer_limit())
  }

  /// The computer's *Services at once*: the default without an application database, the fewest, logged, when it
  /// cannot be read.
  fn computer_limit(&self) -> ComputerServiceSlots {
    let Some(settings) = self.settings.as_deref().filter(|settings| settings.is_file()) else {
      return ComputerServiceSlots::DEFAULT;
    };
    match AppDb::open_read_only(settings).and_then(|db| db.service_slots()) {
      Ok(limit) => limit,
      Err(error) => {
        tracing::warn!(%error, "the computer's Services at once cannot be read; this start uses one slot");
        ComputerServiceSlots::new(ComputerServiceSlots::MIN.into()).expect("the fewest service slots are in range")
      }
    }
  }
}

fn cannot(error: &io::Error) -> String {
  format!("a service slot cannot be taken: {error}")
}

/// `count` free slots of the project's lowest `share` and the computer's lowest `limit`, or `None`, holding nothing,
/// while fewer are free.
fn try_take(
  folder: &Path,
  project: &ProjectId,
  count: u32,
  (share, limit): (u32, NonZeroU32),
) -> io::Result<Option<Vec<ServiceSlot>>> {
  let own = folder.join("projects").join(project.to_string());
  let Some(projects) = free_locks(&own, share, count)? else {
    return Ok(None);
  };
  let Some(computers) = free_locks(folder, limit.get(), count)? else {
    return Ok(None);
  };
  Ok(Some(
    computers
      .into_iter()
      .zip(projects)
      .map(|(computer, project)| ServiceSlot {
        _computer: computer,
        _project: project,
      })
      .collect(),
  ))
}

/// `count` locks of the lowest `upto` slots in `folder`, held; `None`, freeing those it took, while fewer are free.
fn free_locks(folder: &Path, upto: u32, count: u32) -> io::Result<Option<Vec<File>>> {
  fs::create_dir_all(folder)?;
  let mut held = Vec::new();
  for slot in 0..upto {
    if held.len() == count as usize {
      break;
    }
    let file = OpenOptions::new()
      .create(true)
      .truncate(false)
      .read(true)
      .write(true)
      .open(folder.join(format!("slot-{slot}.lock")))?;
    match file.try_lock() {
      Ok(()) => held.push(file),
      Err(TryLockError::WouldBlock) => {}
      Err(TryLockError::Error(error)) => return Err(error),
    }
  }
  Ok((held.len() == count as usize).then_some(held))
}
