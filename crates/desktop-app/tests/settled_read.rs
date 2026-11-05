//! A read that settles before it is read again (`project::settled_read`): deps that move faster than a read takes make
//! one read at a time, each published, and one more read of the newest deps once they stop.

use std::{
  sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
  },
  thread,
  time::{Duration, Instant},
};

use lurq::{
  app::{
    App, Tree,
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureState},
  },
  components::Column,
  core::Signal,
  node::Element,
};
use orchester_app::project::settled_read::settled_read;
use parking_lot::Mutex;

/// How long each read takes.
const READ: Duration = Duration::from_millis(200);

/// What the reader saw: the deps it renders with, the reads it made and how many ran at once at most, and what each
/// render got.
#[derive(Clone)]
struct Probe {
  deps: Signal<u64>,
  reads: Arc<AtomicUsize>,
  running: Arc<AtomicUsize>,
  most_at_once: Arc<AtomicUsize>,
  shown: Arc<Mutex<Vec<FutureState<u64, ()>>>>,
}

impl Probe {
  fn new() -> Self {
    Self {
      deps: Signal::new(0),
      reads: Arc::default(),
      running: Arc::default(),
      most_at_once: Arc::default(),
      shown: Arc::default(),
    }
  }

  fn last(&self) -> FutureState<u64, ()> {
    self.shown.lock().last().cloned().expect("the reader rendered")
  }
}

impl PartialEq for Probe {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.reads, &other.reads)
  }
}

impl DevtoolsInspectable for Probe {}

struct Reader;

impl Component for Reader {
  type Props = Probe;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let probe = ctx.props::<Probe>().clone();
    let deps = probe.deps.get();
    let reading = probe.clone();
    let state = settled_read(ctx, "test", deps, move |deps| {
      let probe = reading.clone();
      async move {
        probe.reads.fetch_add(1, Ordering::SeqCst);
        let now = probe.running.fetch_add(1, Ordering::SeqCst) + 1;
        probe.most_at_once.fetch_max(now, Ordering::SeqCst);
        tokio::time::sleep(READ).await;
        probe.running.fetch_sub(1, Ordering::SeqCst);
        Ok::<_, ()>(deps)
      }
    });
    probe.shown.lock().push(state);
    Column::new()
  }
}

fn pass(tree: &mut Tree, app: &mut App) {
  tree.tick_futures();
  tree.pass_headless(app);
}

#[test]
fn settled_read_reads_one_at_a_time_while_deps_move_faster() {
  let runtime = tokio::runtime::Runtime::new().expect("a runtime");
  let mut app = App::new().with_tokio_handle(runtime.handle().clone());
  let mut tree = Tree::new();
  let probe = Probe::new();
  tree.mount_root::<Reader>(&mut app, probe.clone());
  pass(&mut tree, &mut app);
  assert!(probe.last().is_pending(), "the first read is under way");
  assert_eq!(probe.last().data, None, "nothing is read yet");

  let started = Instant::now();
  while started.elapsed() < Duration::from_secs(2) {
    probe.deps.update(|deps| *deps += 1);
    pass(&mut tree, &mut app);
    thread::sleep(Duration::from_millis(20));
  }
  let newest = probe.deps.get_untracked();
  let published = probe.shown.lock().iter().filter(|state| state.is_fulfilled()).count();
  assert!(published >= 5, "reads were published while deps moved: {published}");

  let deadline = Instant::now() + Duration::from_secs(5);
  while probe.last().data != Some(newest) || probe.last().is_pending() {
    assert!(
      Instant::now() < deadline,
      "the newest deps are read: {:?}",
      probe.last()
    );
    pass(&mut tree, &mut app);
    thread::sleep(Duration::from_millis(10));
  }
  assert_eq!(probe.most_at_once.load(Ordering::SeqCst), 1, "one read at a time");
  let reads = probe.reads.load(Ordering::SeqCst);
  assert!(
    reads <= 11,
    "about one read per {READ:?} for 2 s, not one per change: {reads}"
  );
}

#[test]
fn settled_read_change_during_a_read_makes_exactly_one_more_read() {
  let runtime = tokio::runtime::Runtime::new().expect("a runtime");
  let mut app = App::new().with_tokio_handle(runtime.handle().clone());
  let mut tree = Tree::new();
  let probe = Probe::new();
  tree.mount_root::<Reader>(&mut app, probe.clone());
  pass(&mut tree, &mut app);
  for _ in 0..3 {
    probe.deps.update(|deps| *deps += 1);
    pass(&mut tree, &mut app);
  }
  let deadline = Instant::now() + Duration::from_secs(5);
  while probe.last().data != Some(3) || probe.last().is_pending() {
    assert!(
      Instant::now() < deadline,
      "the newest deps are read: {:?}",
      probe.last()
    );
    pass(&mut tree, &mut app);
    thread::sleep(Duration::from_millis(10));
  }
  assert!(
    probe.shown.lock().iter().any(|state| state.data == Some(0)),
    "the first read is published"
  );
  for _ in 0..20 {
    pass(&mut tree, &mut app);
    thread::sleep(Duration::from_millis(10));
  }
  assert_eq!(
    probe.reads.load(Ordering::SeqCst),
    2,
    "the first read and one of the newest deps"
  );
}
