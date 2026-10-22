//! Eight runs at once on one project database, with what reads and writes it beside them (ORC-74): the app
//! polling the task list and each run's transcript through a connection opened per read, the Orchestrator chat
//! reading runs and their history through one it keeps, the base-move follower reading the runs and recording the
//! base branch's head, the plan-limit reads, settings saved now and then, and the agents' builds writing and
//! flushing files on the same disk. Each run's agents write over a thousand transcript lines, two step results and
//! their checkpoints. No run is interrupted for the runner's failure and no reader fails; the longest write-lock
//! holds and the largest write-ahead log are printed. And runs whose writes find the database locked by another
//! connection for seconds: they wait for the lock and go on. Both run on a runtime with the runner's own worker
//! count, and the runner keeps serving its other work meanwhile, as a statement that waits for a lock pauses off the
//! async workers. Needs `git`. Run with `cargo test -p orchester-runner --features test-agent --test db_contention
//! db_contention -- --nocapture`.

mod support;

use std::{
  fs::File,
  io::Write,
  num::NonZeroU32,
  path::{Path, PathBuf},
  sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
  },
  thread::{self, JoinHandle},
  time::{Duration, Instant},
};

use orchester_core::{
  base_move::BaseHead,
  code_index::CodeIndexSetting,
  id::{IdempotencyKey, RunId},
  revision::Revision,
  run::{BranchName, RunStatus},
  store::{ActivityStore, RunStore, SettingsStore, TaskStore},
};
use orchester_runner::WORKER_THREADS;
use orchester_store::ProjectDb;
use tokio::task::JoinHandle as TaskHandle;

use self::support::{Setup, git, project, queue_another_task, scheduler, wait_for, wait_for_idle};

const RUNS: u32 = 8;
/// How often each reader polls, as the app and the chat do while runs work.
const POLL: Duration = Duration::from_millis(20);
/// A hold of the write lock this long is logged as a warning; none comes near it. While each commit flushed the
/// disk, the longest took up to 135 ms, and a writer waited up to a second.
const LONG_HOLD: Duration = Duration::from_secs(1);
/// How often the stand-in for the runner's other work (a timer, the desktop's requests) wants to run.
const TICK: Duration = Duration::from_millis(50);
/// The longest it may wait for a worker while the runner's writes wait for the lock.
const RESPONSIVE: Duration = Duration::from_secs(1);
/// How long the other connection holds the write lock in the slow-writer case: seconds, as a commit that waited for
/// a disk flushing the agents' builds did.
const SLOW_WRITE: Duration = Duration::from_secs(9);

/// One of the readers and writers beside the runs: it works until told to stop and gives how often it did, or its
/// first error.
type Beside = JoinHandle<Result<u64, String>>;

fn beside(stop: &Arc<AtomicBool>, work: impl Fn() -> Result<(), String> + Send + 'static) -> Beside {
  let stop = Arc::clone(stop);
  thread::spawn(move || {
    let mut times = 0;
    while !stop.load(Ordering::Acquire) {
      work()?;
      times += 1;
      thread::sleep(POLL);
    }
    Ok(times)
  })
}

/// The app: a connection opened for each read of the task list, the runs and each run's transcript.
fn app_reads(root: PathBuf) -> impl Fn() -> Result<(), String> + Send + 'static {
  move || {
    let db = ProjectDb::open(&root).map_err(|error| format!("app open: {error}"))?;
    for task in db.tasks().map_err(|error| format!("app tasks: {error}"))? {
      for run in db
        .runs_of_task(task.id())
        .map_err(|error| format!("app runs: {error}"))?
      {
        let run = run.map_err(|unreadable| format!("app run: {}", unreadable.error))?;
        db.activity(run.id(), 0)
          .map_err(|error| format!("app activity: {error}"))?;
        db.invocations(run.id())
          .map_err(|error| format!("app invocations: {error}"))?;
      }
    }
    Ok(())
  }
}

/// The Orchestrator chat: `list_runs`, `get_run` and the history of each, through the connection it keeps.
fn chat_reads(
  root: &Path,
  runs: Arc<parking_lot::Mutex<Vec<RunId>>>,
) -> impl Fn() -> Result<(), String> + Send + 'static {
  let db = parking_lot::Mutex::new(ProjectDb::open(root).expect("the chat's connection"));
  move || {
    let db = db.lock();
    db.unfinished_runs()
      .map_err(|error| format!("chat list_runs: {error}"))?;
    for run in runs.lock().iter() {
      db.run(*run).map_err(|error| format!("chat get_run: {error}"))?;
      db.events(*run).map_err(|error| format!("chat history: {error}"))?;
    }
    Ok(())
  }
}

/// The base-move follower and the plan-limit reads: the unfinished runs, the base branch's head recorded, the runs
/// at a plan limit.
fn follower(root: PathBuf, head: Revision) -> impl Fn() -> Result<(), String> + Send + 'static {
  let base = BranchName::new("main").expect("branch");
  move || {
    let mut db = ProjectDb::open(&root).map_err(|error| format!("follower open: {error}"))?;
    db.unfinished_runs()
      .map_err(|error| format!("follower runs: {error}"))?;
    db.tells_base_moves()
      .map_err(|error| format!("follower setting: {error}"))?;
    db.base_head(&base).map_err(|error| format!("follower head: {error}"))?;
    let seen = BaseHead {
      head: head.clone(),
      moved_at: None,
    };
    db.save_base_head(&base, &seen)
      .map_err(|error| format!("follower record: {error}"))?;
    db.runs_at_plan_limit()
      .map_err(|error| format!("plan limits: {error}"))?;
    Ok(())
  }
}

/// A person changing a setting in the app.
fn settings_writes(root: PathBuf) -> impl Fn() -> Result<(), String> + Send + 'static {
  move || {
    let mut db = ProjectDb::open(&root).map_err(|error| format!("settings open: {error}"))?;
    db.save_code_index(CodeIndexSetting::Off)
      .map_err(|error| format!("settings save: {error}"))
  }
}

/// The agents' builds: a file written and flushed to the disk the database is on, as compilers in the worktrees
/// do. A flush waits for what the drive still has to write, the database's own commits included.
fn builds(root: &Path) -> impl Fn() -> Result<(), String> + Send + 'static {
  let output = root.join(".orchester").join("build-output.bin");
  let chunk = vec![7_u8; 256 * 1024];
  move || {
    let mut file = File::create(&output).map_err(|error| format!("build output: {error}"))?;
    file
      .write_all(&chunk)
      .map_err(|error| format!("build write: {error}"))?;
    file.sync_all().map_err(|error| format!("build flush: {error}"))
  }
}

/// Keeps the largest size the project's write-ahead log reached in `peak`: checkpoints keep it small unless
/// readers that never end hold them back.
fn wal_size(root: &Path, peak: Arc<AtomicU64>) -> impl Fn() -> Result<(), String> + Send + 'static {
  let wal = root.join(".orchester").join("orchester.db-wal");
  move || {
    if let Ok(metadata) = std::fs::metadata(&wal) {
      peak.fetch_max(metadata.len(), Ordering::Relaxed);
    }
    Ok(())
  }
}

/// Runs `test` on a runtime like the runner's: multi-threaded, with its worker count.
fn on_runners_runtime(test: impl Future<Output = ()>) {
  tokio::runtime::Builder::new_multi_thread()
    .worker_threads(WORKER_THREADS)
    .enable_all()
    .build()
    .expect("the runner's runtime")
    .block_on(test);
}

/// Stands in for the runner's other work (a timer, the desktop's requests): it wants a worker every [`TICK`] and keeps
/// the longest it had to wait for one.
struct Ticker {
  started: Instant,
  /// When it last ran and the longest gap between two runs, in milliseconds since `started`.
  last: Arc<AtomicU64>,
  longest: Arc<AtomicU64>,
  task: TaskHandle<()>,
}

impl Ticker {
  fn start() -> Self {
    let started = Instant::now();
    let (last, longest) = (Arc::new(AtomicU64::new(0)), Arc::new(AtomicU64::new(0)));
    let task = tokio::spawn({
      let (last, longest) = (Arc::clone(&last), Arc::clone(&longest));
      async move {
        loop {
          tokio::time::sleep(TICK).await;
          let now = millis(started.elapsed());
          longest.fetch_max(now - last.load(Ordering::SeqCst), Ordering::SeqCst);
          last.store(now, Ordering::SeqCst);
        }
      }
    });
    Self {
      started,
      last,
      longest,
      task,
    }
  }

  /// Stops it and gives the longest gap, the one still open now included: a runtime that never ran it again since
  /// it froze fails here too.
  fn stop(self) -> Duration {
    self.task.abort();
    let open = millis(self.started.elapsed()) - self.last.load(Ordering::SeqCst);
    Duration::from_millis(self.longest.load(Ordering::SeqCst).max(open))
  }
}

fn millis(duration: Duration) -> u64 {
  u64::try_from(duration.as_millis()).expect("a test's milliseconds fit in u64")
}

#[test]
fn db_contention_eight_runs_with_readers_settle_without_runner_failures() {
  on_runners_runtime(eight_runs_with_readers());
}

#[allow(
  clippy::print_stderr,
  reason = "prints the measured write-lock holds, which the test is run to report"
)]
async fn eight_runs_with_readers() {
  let project = project(&Setup {
    review: true,
    ..Setup::new("Flood standard error. Append one line.")
  });
  let root = project.repo.path();
  let tasks: Vec<_> = std::iter::once(project.task)
    .chain((1..RUNS).map(|_| queue_another_task(root)))
    .collect();
  let head = Revision::new(&git(root, &["rev-parse", "HEAD"])).expect("revision");
  let (scheduler, mut events) = scheduler(root);
  scheduler
    .set_agent_slots(NonZeroU32::new(RUNS).expect("above zero"))
    .await
    .expect("a slot for each run");
  orchester_store::reset_lock_holds();
  let ticker = Ticker::start();
  let started = Instant::now();
  let stop = Arc::new(AtomicBool::new(false));
  let runs = Arc::new(parking_lot::Mutex::new(Vec::new()));
  let wal_peak = Arc::new(AtomicU64::new(0));
  let besides = [
    beside(&stop, app_reads(root.to_path_buf())),
    beside(&stop, chat_reads(root, Arc::clone(&runs))),
    beside(&stop, follower(root.to_path_buf(), head)),
    beside(&stop, settings_writes(root.to_path_buf())),
    beside(&stop, builds(root)),
    beside(&stop, wal_size(root, Arc::clone(&wal_peak))),
  ];
  for task in tasks {
    let run = scheduler
      .start_run(IdempotencyKey::generate(), task)
      .await
      .expect("the run starts");
    runs.lock().push(run);
  }
  let ids = runs.lock().clone();
  for run in ids {
    let ended = wait_for(root, &mut events, run, |run| run.status() != &RunStatus::Running).await;
    assert_eq!(ended.status(), &RunStatus::Succeeded, "run {run}");
  }
  wait_for_idle(&scheduler).await;
  let longest_gap = ticker.stop();
  stop.store(true, Ordering::Release);
  for beside in besides {
    let times = beside.join().expect("the reader ends").expect("the reader never fails");
    assert!(times > 0, "the reader worked beside the runs");
  }
  let elapsed = started.elapsed();
  let holds = orchester_store::lock_holds();
  eprintln!(
    "{RUNS} runs settled in {elapsed:?}; {} write transactions held the write lock for {:?} together ({:.0} % of \
     the time); the longest wait for it {:?}; largest write-ahead log {} KiB; longest holds: {:#?}",
    holds.count,
    holds.total,
    holds.total.as_secs_f64() / elapsed.as_secs_f64() * 100.0,
    holds.longest_wait,
    wal_peak.load(Ordering::Relaxed) / 1024,
    holds.longest
  );
  assert!(
    holds.longest.first().is_none_or(|longest| longest.held < LONG_HOLD),
    "no write transaction holds the lock long: {:?}",
    holds.longest
  );
  assert!(
    longest_gap < RESPONSIVE,
    "the runner's other work waited {longest_gap:?} for a worker"
  );
}

#[test]
fn db_contention_runs_wait_out_a_slow_writer_and_the_runner_stays_responsive() {
  on_runners_runtime(runs_meet_a_slow_writer());
}

async fn runs_meet_a_slow_writer() {
  let project = project(&Setup::new("Append one line."));
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let others = [queue_another_task(root), queue_another_task(root)];
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  wait_for(root, &mut events, id, |run| !run.running_executions().is_empty()).await;

  // Its agent works now, and the run records what it does: every write of the run waits for this lock, and so does
  // recording each of the two runs started meanwhile, more waiting writes than the runtime has workers.
  let slow = rusqlite::Connection::open(root.join(".orchester").join("orchester.db")).expect("open");
  slow
    .execute_batch("BEGIN IMMEDIATE; UPDATE project SET name = 'Kontur Two';")
    .expect("hold the write lock");
  let ticker = Ticker::start();
  let starts = others.map(|task| {
    let scheduler = scheduler.clone();
    tokio::spawn(async move { scheduler.start_run(IdempotencyKey::generate(), task).await })
  });
  tokio::time::sleep(SLOW_WRITE).await;
  let longest_gap = ticker.stop();
  assert!(
    longest_gap < RESPONSIVE,
    "the runner's other work waited {longest_gap:?} for a worker while its writes waited for the lock"
  );
  assert!(
    starts.iter().all(|start| !start.is_finished()),
    "the starts wait for the lock"
  );
  slow.execute_batch("COMMIT").expect("end the slow write");

  let mut runs = vec![id];
  for start in starts {
    runs.push(start.await.expect("the start does not panic").expect("the run starts"));
  }
  for run in runs {
    let ended = wait_for(root, &mut events, run, |run| run.status().is_terminal()).await;
    assert_eq!(ended.status(), &RunStatus::Succeeded, "run {run}");
    assert_eq!(
      ProjectDb::open(root).expect("open").runner_failure(run).expect("read"),
      None,
      "run {run}"
    );
  }
  wait_for_idle(&scheduler).await;
}
