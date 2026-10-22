//! Opening a chat measured frame by frame (ORC-71): how long each frame's main-thread work took (delivering what
//! the window's futures finished, which re-renders what waited for them, and the pass that lays the window out),
//! whether something in the window changed for it, and how many rows the chat had built, from the frame that showed
//! the chat until it settles; written under `.tmp/orc-71/profile/`, with lurq's `perf_profile` with a lurq profile
//! session's samples of the same frames. Reading the window re-renders what its last pass left dirty, as the next
//! pass would: until the chat shows, that reading is part of the frame it follows; while it settles, nothing reads it
//! until it settled, so each frame is the pass's own.

use std::time::{Duration, Instant};

use lurq::app::PassReasons;

use super::Window;

/// Frames in a row in which nothing changed: the chat has settled.
const SETTLED: usize = 3;
/// The most frames an opening may take before it counts as never settling.
const MOST_FRAMES: usize = 200;

/// One frame of the window.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
  /// Delivering the futures that finished, the pass, and reading the window after it until the chat shows.
  pub wall: Duration,
  /// The pass alone.
  pub pass: Duration,
  /// Something in the window changed for it ([`changed`]): not a frame that only lays out or draws again.
  pub changed: bool,
  /// Why lurq ran the pass.
  pub reasons: PassReasons,
}

/// The frames of an opening: those until the chat showed what was waited for (the last of them showed it), with the
/// rows the chat had built after each, then those until it settled, the [`SETTLED`] frames in which nothing changed
/// not counted, and the rows it had built then.
#[derive(Debug)]
pub struct Opening {
  pub until_shown: Vec<(Frame, usize)>,
  pub settling: Vec<Frame>,
  pub settled_built: usize,
}

impl Opening {
  /// The frame that first showed the chat.
  pub fn first_shown(&self) -> Frame {
    self.until_shown.last().expect("a frame showed the chat").0
  }

  /// The time from the first frame that built a row, the one that had the data read, through the frame that showed
  /// the chat.
  pub fn to_first_frame(&self) -> Duration {
    self
      .until_shown
      .iter()
      .skip_while(|(_, built)| *built == 0)
      .map(|(frame, _)| frame.wall)
      .sum()
  }

  /// The most rows built after any frame until the chat showed, and once it settled.
  pub fn most_built(&self) -> usize {
    self
      .until_shown
      .iter()
      .map(|(_, built)| *built)
      .chain([self.settled_built])
      .max()
      .unwrap_or_default()
  }

  /// The frames in which something changed after the one that showed the chat.
  pub fn layouts_to_settle(&self) -> usize {
    self.settling.iter().filter(|frame| frame.changed).count()
  }

  /// The longest frame from the one that showed the chat on.
  pub fn longest_settling(&self) -> Duration {
    self
      .settling
      .iter()
      .map(|frame| frame.wall)
      .chain([self.first_shown().wall])
      .max()
      .unwrap_or_default()
  }
}

/// Runs `window`'s frames until `shown` holds after one, then until it settles; `built` counts the chat's rows.
/// Panics when it is not shown or does not settle within [`MOST_FRAMES`].
pub fn open(
  window: &mut Window,
  built: impl Fn(&mut Window) -> usize,
  mut shown: impl FnMut(&mut Window) -> bool,
) -> Opening {
  let mut until_shown = Vec::new();
  loop {
    let began = Instant::now();
    let (pass, reasons) = pass(window);
    let (rows, done) = (built(window), shown(window));
    let frame = Frame {
      wall: began.elapsed(),
      pass,
      changed: changed(reasons),
      reasons,
    };
    until_shown.push((frame, rows));
    if done {
      break;
    }
    assert!(until_shown.len() < MOST_FRAMES, "the chat is shown");
    std::thread::sleep(Duration::from_millis(5));
  }
  let mut settling: Vec<Frame> = Vec::new();
  let mut quiet = 0;
  while quiet < SETTLED {
    let began = Instant::now();
    let (pass, reasons) = pass(window);
    let frame = Frame {
      wall: began.elapsed(),
      pass,
      changed: changed(reasons),
      reasons,
    };
    quiet = if frame.changed { 0 } else { quiet + 1 };
    settling.push(frame);
    assert!(
      settling.len() < MOST_FRAMES,
      "the chat settles; its last frames: {:?}",
      &settling[settling.len() - SETTLED..]
    );
    std::thread::sleep(Duration::from_millis(5));
  }
  settling.truncate(settling.len() - SETTLED);
  Opening {
    until_shown,
    settling,
    settled_built: built(window),
  }
}

/// Delivers what `window`'s futures finished and runs a pass: how long the pass took, and why it ran.
fn pass(window: &mut Window) -> (Duration, PassReasons) {
  window.tree.tick_futures();
  let passing = Instant::now();
  let report = window.tree.pass_headless(&mut window.app);
  (passing.elapsed(), report.reasons)
}

/// Whether something in the window changed for a pass: a component re-rendered, a measured element changed or a
/// future finished. A layout left dirty alone does not count: lurq 0.49.1 leaves it dirty after every pass while a
/// Markdown link shows (`tests/ui_kit/markdown_link_layout.rs`), so a chat with a link would never settle; a scroll
/// the chat still moves re-renders its list, which counts.
const fn changed(reasons: PassReasons) -> bool {
  reasons.component_dirty || reasons.element_ref_dirty || reasons.future_completed
}

/// Writes what `opening` measured, and `more` after it, as `name` and the build's profile under
/// `.tmp/orc-71/profile/`.
pub fn write(name: &str, opening: &Opening, more: &str) {
  use std::fmt::Write as _;
  let mut text = format!(
    "{name}: first shown after {} frames, {:?} from the first frame that built a row through the one that showed \
     it, {} frames that changed something to settle, longest frame from then {:?}, at most {} rows built\n",
    opening.until_shown.len(),
    opening.to_first_frame(),
    opening.layouts_to_settle(),
    opening.longest_settling(),
    opening.most_built(),
  );
  let line = |text: &mut String, frame: &Frame, built: Option<usize>| {
    let built = built.map_or_else(String::new, |built| format!(", {built} rows built"));
    writeln!(
      text,
      "frame {:?} (pass {:?}), changed {}, layout dirty {}{built}",
      frame.wall, frame.pass, frame.changed, frame.reasons.layout_dirty
    )
    .expect("a string takes text");
  };
  for (frame, built) in &opening.until_shown {
    line(&mut text, frame, Some(*built));
  }
  writeln!(text, "shown; settling:").expect("a string takes text");
  for frame in &opening.settling {
    line(&mut text, frame, None);
  }
  text.push_str(more);
  let folder = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/orc-71/profile");
  std::fs::create_dir_all(&folder).expect("the profile folder");
  let build = if cfg!(debug_assertions) { "debug" } else { "release" };
  std::fs::write(folder.join(format!("{name}-{build}.txt")), text).expect("the profile is written");
}

/// A lurq profile session over the frames [`open`] runs.
#[cfg(feature = "perf_profile")]
pub mod session {
  use std::fmt::Write as _;

  use lurq::app::profiler::{ProfilingHandle, SampleData, SessionId, SessionOptions, UiUpdateKind};

  use super::Window;

  pub struct Session {
    handle: ProfilingHandle,
    id: SessionId,
  }

  impl Session {
    pub fn start(window: &Window) -> Self {
      let handle = window.tree.profiling_handle();
      let started = handle
        .start(SessionOptions {
          max_samples: lurq::app::profiler::MAX_SAMPLES_PER_SESSION,
          include_devtools: false,
        })
        .expect("a profile session starts");
      Self { handle, id: started.id }
    }

    /// Ends the session: its samples, one a line.
    pub fn end(self) -> String {
      let report = self.handle.end(self.id).expect("the session ends");
      let mut text = String::new();
      for sample in &report.samples {
        let line = match &sample.data {
          SampleData::Pass(pass) => format!(
            "lurq pass {:?}: total {:?}, layout update {:?} (compute {:?}), after layout {:?}, laid out {}",
            pass.frame_id,
            pass.total,
            pass.layout_update,
            pass.layout_compute,
            pass.component_after_layout,
            pass.layout_recalculated
          ),
          SampleData::UiUpdate(update) => format!(
            "lurq ui update {:?} {}: total {:?}, commit {:?}",
            update.frame_id,
            if update.kind == UiUpdateKind::RootRebuild {
              "root"
            } else {
              "subtree"
            },
            update.total,
            update.commit
          ),
          SampleData::InputDispatch(input) => format!("lurq input {:?}: {:?}", input.frame_id, input.total),
        };
        writeln!(text, "{line}").expect("a string takes text");
      }
      text
    }
  }
}
