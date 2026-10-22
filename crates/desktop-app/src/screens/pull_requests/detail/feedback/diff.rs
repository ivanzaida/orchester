//! What feedback draws into a file's diff in Files changed (`Lines` `Opg3p`): the lines of its open threads and the
//! chosen lines marked (`LhyWc`), each thread under its last line (`Thread wrap` `p6wOsh`), several ending on one line
//! under a line that says how many (`BHXTq`), and the composer of a new thread under the chosen lines. A press on a
//! line chooses it for a new thread; Shift extends the choice. Where a thread's lines are in the diff shown follows
//! the diff's head: the lines it was written on at its own revision, where the same code is at the pull request's
//! head, nowhere when the code is gone.

use std::{
  collections::{BTreeMap, BTreeSet},
  hash::{DefaultHasher, Hash, Hasher},
  sync::Arc,
};

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::{
  diff::{Diff, FileDiff},
  pull_request::feedback::{DiffSide, LineSpan, Located},
  revision::Revision,
};
use orchester_workspace::pull_requests::feedback::ThreadView;

use super::{
  Composing,
  composer::{NewProps, new_thread, open},
  thread::{Env, several, wrapped},
};
use crate::screens::task::changes::diff::{Annotations, LineKey, LinePress};

/// The key of the composer's block.
const COMPOSER: &str = "composer";

/// The annotations of `file`, at `path` in the diff of head `shown`; `latest`: the pull request's diff at its head.
pub fn annotations(
  env: &Env,
  (shown, file, path): (&Revision, &FileDiff, &str),
  latest: Option<Arc<Diff>>,
) -> Annotations {
  let placed: Vec<(&ThreadView, DiffSide, LineSpan)> = env
    .board
    .threads
    .iter()
    .filter(|thread| thread.path() == Some(path))
    .filter_map(|thread| place(env, thread, shown).map(|(side, span)| (thread, side, span)))
    .collect();
  let mut marked = BTreeSet::new();
  let mut ends: BTreeMap<LineKey, Vec<&ThreadView>> = BTreeMap::new();
  for (thread, side, span) in &placed {
    if thread.state.is_open() {
      marked.extend((span.start()..=span.end()).map(|number| LineKey { side: *side, number }));
    }
    ends
      .entry(LineKey {
        side: *side,
        number: span.end(),
      })
      .or_default()
      .push(thread);
  }
  let composing = chosen(env, path, shown);
  let mut blocks: Vec<(LineKey, Arc<str>)> = ends
    .iter()
    .map(|(at, threads)| {
      let key = match threads.as_slice() {
        [one] => format!("thread:{}", one.id),
        _ => format!("several:{}", at.number),
      };
      (*at, Arc::from(key))
    })
    .collect();
  if let Some((side, span)) = composing {
    marked.extend((span.start()..=span.end()).map(|number| LineKey { side, number }));
    let at = LineKey {
      side,
      number: span.end(),
    };
    // The composer stands under the chosen lines, before a thread that ends on the same line.
    let before = blocks.iter().position(|(line, _)| *line == at).unwrap_or(blocks.len());
    blocks.insert(before, (at, Arc::from(COMPOSER)));
  }
  let reveal = env.state.reveal.get().and_then(|id| {
    blocks
      .iter()
      .find(|(at, key)| {
        **key == *format!("thread:{id}")
          || ends
            .get(at)
            .is_some_and(|threads| threads.iter().any(|thread| thread.id == id))
      })
      .map(|(_, key)| key.clone())
  });
  let version = version(env, (shown, path), (&blocks, &marked));
  Annotations {
    marked: Arc::new(marked),
    blocks: Arc::new(blocks),
    block: block_view(env, (file, path, shown), latest),
    press: env.board.task.is_some().then(|| press(env, (path, shown))),
    reveal,
    version,
  }
}

/// Where `thread`'s lines are in the diff of head `shown`: at its own revision, the lines it was written on; at the
/// pull request's head, where the same code is; elsewhere, nowhere.
fn place(env: &Env, thread: &ThreadView, shown: &Revision) -> Option<(DiffSide, LineSpan)> {
  let lines = thread.anchor.lines()?;
  if *shown == thread.revision {
    return Some((lines.side(), lines.span()));
  }
  if env.head.as_ref() != Some(shown) {
    return None;
  }
  match thread.at_head {
    None => Some((lines.side(), lines.span())),
    Some(Located::At(span)) => Some((lines.side(), span)),
    Some(Located::Outdated) => None,
  }
}

/// The lines chosen for a new thread in this file of this diff, when its composer is open.
fn chosen(env: &Env, path: &str, shown: &Revision) -> Option<(DiffSide, LineSpan)> {
  match env.state.composing.get()? {
    Composing::Lines {
      path: chosen,
      side,
      span,
      revision,
    } if &*chosen == path && revision == *shown => Some((side, span)),
    _ => None,
  }
}

/// What tells these annotations apart: the threads as read, what is open and chosen, and where.
fn version(
  env: &Env,
  (shown, path): (&Revision, &str),
  (blocks, marked): (&[(LineKey, Arc<str>)], &BTreeSet<LineKey>),
) -> u64 {
  let mut hasher = DefaultHasher::new();
  (Arc::as_ptr(&env.board) as usize).hash(&mut hasher);
  shown.hash(&mut hasher);
  path.hash(&mut hasher);
  blocks.hash(&mut hasher);
  marked.hash(&mut hasher);
  env.state.composing.get().hash(&mut hasher);
  env.state.written.get().hash(&mut hasher);
  hasher.finish()
}

/// Draws a block by its key: a thread, the threads ending on one line, or the composer.
fn block_view(
  env: &Env,
  (file, path, shown): (&FileDiff, &str, &Revision),
  latest: Option<Arc<Diff>>,
) -> crate::screens::task::changes::diff::BlockView {
  let env = env.clone();
  let file = file.clone();
  let (path, shown) = (Arc::<str>::from(path), shown.clone());
  Arc::new(move |ctx: &mut Ctx, key: &Arc<str>| {
    if let Some(id) = key.strip_prefix("thread:") {
      return env
        .board
        .threads
        .iter()
        .find(|thread| thread.id.to_string() == id)
        .map_or_else(Element::new, |thread| wrapped(ctx, &env, thread));
    }
    if let Some(line) = key.strip_prefix("several:").and_then(|line| line.parse::<u32>().ok()) {
      let threads: Vec<&ThreadView> = env
        .board
        .threads
        .iter()
        .filter(|thread| thread.path() == Some(&*path))
        .filter(|thread| place(&env, thread, &shown).is_some_and(|(_, span)| span.end() == line))
        .collect();
      let add = add_on_line(&env, (&path, &shown), line);
      return several(ctx, &env, (line, &threads), add);
    }
    let Some(composing) = env.state.composing.get() else {
      return Element::new();
    };
    let excerpt = match &composing {
      Composing::Lines { side, span, .. } => excerpt(&file, *side, *span),
      _ => Vec::new(),
    };
    super::thread::wrap(new_thread(
      ctx,
      &NewProps {
        env: env.clone(),
        composing,
        excerpt,
        latest: latest.clone(),
      },
    ))
  })
}

/// *Add feedback on this line*: a new thread's composer on line `line` of the new side.
fn add_on_line(env: &Env, (path, shown): (&Arc<str>, &Revision), line: u32) -> Option<Arc<dyn Fn() + Send + Sync>> {
  let task = env.board.task.as_ref()?;
  let (state, default, path, revision) = (env.state.clone(), task.default, path.clone(), shown.clone());
  let span = LineSpan::line(std::num::NonZeroU32::new(line)?);
  Some(Arc::new(move || {
    open(
      &state,
      Composing::Lines {
        path: path.clone(),
        side: DiffSide::New,
        span,
        revision: revision.clone(),
      },
      default,
    );
  }))
}

/// The code of lines `span` on `side` of `file`.
fn excerpt(file: &FileDiff, side: DiffSide, span: LineSpan) -> Vec<String> {
  file
    .hunks()
    .iter()
    .flat_map(|hunk| hunk.lines())
    .filter_map(|line| {
      let number = match side {
        DiffSide::Old => line.old_number(),
        DiffSide::New => line.new_number(),
      }?;
      span.contains(number).then(|| line.text().to_owned())
    })
    .collect()
}

/// A press on a line opens a new thread's composer on it; with Shift, on the lines from the chosen ones to it, on
/// the same side.
fn press(env: &Env, (path, shown): (&str, &Revision)) -> LinePress {
  let (state, default) = (env.state.clone(), env.board.task.as_ref().and_then(|task| task.default));
  let (path, revision) = (Arc::<str>::from(path), shown.clone());
  Arc::new(move |key: LineKey, extend: bool| {
    // Diff lines are numbered from 1: a line 0 names nothing to choose.
    let Some(line) = std::num::NonZeroU32::new(key.number) else {
      return;
    };
    let extended = match state.composing.get_untracked() {
      Some(Composing::Lines {
        path: chosen,
        side,
        span,
        ..
      }) if extend && chosen == path && side == key.side => std::num::NonZeroU32::new(span.start().min(key.number))
        .zip(std::num::NonZeroU32::new(span.end().max(key.number)))
        .and_then(|(start, end)| LineSpan::new(start, end).ok()),
      _ => None,
    };
    let span = extended.unwrap_or_else(|| LineSpan::line(line));
    let next = Composing::Lines {
      path: path.clone(),
      side: key.side,
      span,
      revision: revision.clone(),
    };
    if let Some(current @ Composing::Lines { .. }) = state.composing.get_untracked() {
      state.drafts.moved(&current, next.clone());
    }
    open(&state, next, default);
  })
}
