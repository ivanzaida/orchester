//! The mouse cursor over the page (`docs/design/browser.md`, "What frames do not show"): CDP does not report its shape,
//! so while the pointer moves the runner is asked what is under it (`BrowserCommand::CursorAt`), one question at a time
//! and a few times a second. The last place the pointer moved to is always asked about, so the shape is right where it
//! stops; once it stands still nothing is asked. The arrow until an answer comes, on another tab and on any failure.

use std::{sync::Arc, time::Duration};

use lurq::{app::ctx::Ctx, core::Signal, node::CursorIcon};
use orchester_ipc::browser::{BrowserTabId, PageCursor};
use parking_lot::Mutex;

use crate::browser::commands::Commands;

/// The least time from one answer to the next question: at most four questions a second.
const BETWEEN_QUESTIONS: Duration = Duration::from_millis(250);

/// What the pointer showed last on the page, and what is still to be asked.
#[derive(Clone)]
pub struct PagePointer {
  /// The last answer, with the tab it is about.
  shape: Signal<Option<(BrowserTabId, PageCursor)>>,
  asking: Arc<Mutex<Asking>>,
}

#[derive(Default)]
struct Asking {
  /// Where the pointer moved to since the last question.
  next: Option<Question>,
  /// The task that asks runs: it takes `next` when its time comes.
  running: bool,
}

struct Question {
  commands: Commands,
  tab: BrowserTabId,
  x: i32,
  y: i32,
}

impl PagePointer {
  pub fn new(ctx: &mut Ctx) -> Self {
    Self {
      shape: ctx.signal(None),
      asking: Arc::new(Mutex::new(Asking::default())),
    }
  }

  /// The cursor to set on the page while it shows `tab`.
  pub fn icon(&self, tab: &BrowserTabId) -> CursorIcon {
    let shape = self.shape.get();
    match shape.as_ref().filter(|(of, _)| of == tab).map(|(_, shape)| *shape) {
      Some(PageCursor::Hand) => CursorIcon::Pointer,
      Some(PageCursor::Text) => CursorIcon::Text,
      Some(PageCursor::Arrow) | None => CursorIcon::Default,
    }
  }

  /// The pointer moved to `x`, `y` of `tab`, in its CSS pixels.
  pub fn moved(&self, commands: &Commands, tab: &BrowserTabId, x: i32, y: i32) {
    let mut asking = self.asking.lock();
    asking.next = Some(Question {
      commands: commands.clone(),
      tab: tab.clone(),
      x,
      y,
    });
    if !asking.running {
      asking.running = true;
      commands.spawn(self.clone().ask());
    }
  }

  /// Asks about where the pointer moved to, until it stands still.
  async fn ask(self) {
    while let Some(Question { commands, tab, x, y }) = self.next_question() {
      let shape = commands.cursor_at(tab.clone(), x, y).await;
      let answer = Some((tab, shape));
      if self.shape.get_untracked() != answer {
        self.shape.set(answer);
      }
      tokio::time::sleep(BETWEEN_QUESTIONS).await;
    }
  }

  /// The question to ask now; with none, the task that asks ends, in the same lock so that no move is left unasked.
  fn next_question(&self) -> Option<Question> {
    let mut asking = self.asking.lock();
    let next = asking.next.take();
    asking.running = next.is_some();
    next
  }
}
