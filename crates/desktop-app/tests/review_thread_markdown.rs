//! Implement's words in a thread of a run's review page (ORC-963), in the app's real window on the QA seed's
//! `review-page` scenario: its answer is Markdown drawn as the chats draw an agent's (bold, a nested list, inline code)
//! and selects and copies like the Conversation's; while it starts, its three dots move until its first words arrive;
//! and its name reads once. Needs `git`, as the seed does.
//! `cargo test -p orchester-app --test review_thread_markdown review_thread_markdown`.

#[path = "../examples/seed/kontur/mod.rs"]
#[allow(dead_code, reason = "the seed's helpers serve every scenario; these tests seed one")]
mod kontur;
mod support;

use std::{
  thread,
  time::{Duration, Instant},
};

use lurq::{
  app::events::{MouseButton, ScrollPhase},
  layout::{
    quad::{QuadContent, RichTextSpan},
    text_style::FontWeight,
  },
};
use orchester_app::routes::{ProjectPage, Route};
use orchester_core::{project::TaskKey, time::Timestamp};

use crate::{
  kontur::{SeedRequest, scenarios},
  support::{Data, Window, stand_ins::StandIns},
};

/// How long reading a run's diff with git may take on a loaded machine.
const GIT_WORK: Duration = Duration::from_secs(60);
/// How far one turn of the wheel scrolls the diff, and how many turns look for a thread.
const WHEEL_STEP: f32 = 500.0;
const WHEEL_TURNS: usize = 16;
/// What a dot's side is (`h6EkpB`), and how often the dots are sampled.
const DOT_SIDE: f32 = 6.0;
const SAMPLE: Duration = Duration::from_millis(23);

/// The review page of the seed's "Sign webhook payloads", in a window whose project has no runner.
fn opened() -> (Window, StandIns) {
  let data = Data::new();
  let request = SeedRequest {
    data_dir: data.dir.path().to_path_buf(),
    repo: data.dir.path().join("repo-review-page"),
    scenario: scenarios::named("review-page").expect("a known scenario"),
    fake_codex: None,
    now: Timestamp::now(),
  };
  let seeded = kontur::seed(&request).expect("the scenario seeds");
  let launch = data.launch_with(data.no_runner());
  let (mut window, stand_ins) = Window::launched_with_stand_ins(data, launch);
  window.tree.resize(1440, 1400);
  let task = seeded
    .tasks
    .iter()
    .find(|task| task.title == "Sign webhook payloads")
    .expect("the scenario seeds the task");
  let key: TaskKey = task.key.parse().expect("a task key");
  window
    .navigation()
    .go(Route::Project(seeded.project, ProjectPage::TaskReview(key)));
  window.until_within(GIT_WORK, "the review page", |window| {
    window.texts("review-status-text").is_some() && window.texts("review-diff").is_some()
  });
  (window, stand_ins)
}

/// Every element id under the diff.
fn ids(window: &mut Window) -> Vec<String> {
  let mut found = Vec::new();
  let mut stack = vec![window.tree.get_element_by_id("review-diff").expect("the diff")];
  while let Some(element) = stack.pop() {
    found.extend(element.id().map(str::to_owned));
    stack.extend(element.children());
  }
  found
}

/// Turns the wheel over the diff until an element whose id `wanted` accepts is laid out, and tells its id.
fn scroll_to(window: &mut Window, what: &str, wanted: impl Fn(&mut Window, &str) -> bool) -> String {
  for _ in 0..WHEEL_TURNS {
    window.pass();
    for id in ids(window) {
      if wanted(window, &id) {
        return id;
      }
    }
    let bounds = window
      .tree
      .get_element_by_id_mut("review-diff")
      .and_then(|element| element.bounds())
      .expect("the diff is laid out");
    let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
    window.tree.mouse_move(x, y);
    window.tree.scroll(x, y, 0.0, -WHEEL_STEP, ScrollPhase::Scroll);
  }
  panic!("{what} in the diff");
}

/// The id of the words of the answer that counts the budget.
fn answer(window: &mut Window) -> String {
  scroll_to(window, "the answer about the budget", |window, id| {
    id.ends_with("-message-1-body") && window.text(id).contains("budget")
  })
}

/// Every run of text the window draws, with where its quad stands.
fn drawn(window: &mut Window) -> Vec<(f32, RichTextSpan)> {
  window.pass();
  let tree = &window.tree;
  tree
    .resolve_quads(tree.last_layout().expect("a layout"))
    .into_iter()
    .flat_map(|quad| {
      let x = quad.x;
      match quad.content {
        QuadContent::RichText { spans, .. } => spans.into_iter().map(|span| (x, span)).collect(),
        QuadContent::Text { text, style, .. } => vec![(x, RichTextSpan { text, style })],
        _ => Vec::new(),
      }
    })
    .collect()
}

/// The run that is exactly `words`, else the first that holds them: a short code run such as `5` would otherwise be
/// found inside a clock or a count drawn before the answer.
fn span_of<'a>(drawn: &'a [(f32, RichTextSpan)], words: &str) -> &'a (f32, RichTextSpan) {
  drawn
    .iter()
    .find(|(_, span)| span.text.trim() == words)
    .or_else(|| drawn.iter().find(|(_, span)| span.text.contains(words)))
    .unwrap_or_else(|| {
      panic!(
        "{words:?} is drawn in a run of its own: {:?}",
        drawn.iter().map(|(_, s)| &s.text).collect::<Vec<_>>()
      )
    })
}

/// Drags over the whole of the element `id`, from just inside its start to just inside its end.
fn drag_over(window: &mut Window, id: &str) {
  let bounds = window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"));
  let (from, to) = (
    (bounds.x + 1.0, bounds.y + 2.0),
    (bounds.x + bounds.width - 2.0, bounds.y + bounds.height - 2.0),
  );
  window.tree.mouse_move(from.0, from.1);
  window.tree.mouse_down(from.0, from.1, MouseButton::Left);
  window.tree.mouse_move(to.0, to.1);
  window.tree.mouse_up(to.0, to.1, MouseButton::Left);
  window.pass();
}

/// The opacity each dot inside `id` is drawn with, left to right.
fn dot_opacities(window: &mut Window, id: &str) -> Vec<f32> {
  let row = window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"));
  let tree = &window.tree;
  let mut dots: Vec<(f32, f32)> = tree
    .resolve_quads(tree.last_layout().expect("a layout"))
    .into_iter()
    .filter(|quad| {
      matches!(quad.content, QuadContent::Rect { .. })
        && quad.width == DOT_SIDE
        && quad.height == DOT_SIDE
        && (row.x..row.x + row.width).contains(&quad.x)
        && (row.y..row.y + row.height).contains(&quad.y)
    })
    .map(|quad| (quad.x, quad.opacity))
    .collect();
  dots.sort_by(|a, b| a.0.total_cmp(&b.0));
  dots.into_iter().map(|(_, opacity)| opacity).collect()
}

#[test]
fn review_thread_markdown_draws_bold_a_nested_list_and_inline_code_of_the_answer() {
  let (mut window, _stand_ins) = opened();
  let id = answer(&mut window);
  let texts = window.texts(&id).expect("the answer is shown");
  for markup in ["**", "`"] {
    assert!(
      texts.iter().all(|text| !text.contains(markup)),
      "{markup:?} is drawn as what it means, not as written: {texts:?}"
    );
  }
  assert!(
    texts.iter().all(|text| !text.trim_start().starts_with("- ")),
    "a bullet is drawn as a list item, not as the dash written: {texts:?}"
  );
  let drawn = drawn(&mut window);
  let (_, body) = span_of(&drawn, "The budget counts");
  let (_, bold) = span_of(&drawn, "every attempt");
  assert_ne!(bold.style.weight, FontWeight::Normal, "**every attempt** is bold");
  assert_eq!(body.style.weight, FontWeight::Normal);
  for code in ["5", "delay_for()"] {
    let (_, span) = span_of(&drawn, code);
    assert_ne!(
      span.style.font_family, body.style.font_family,
      "`{code}` is inline code, in the code's own font"
    );
  }
  let (outer, _) = span_of(&drawn, "the first try");
  let (inner, _) = span_of(&drawn, "then up to four retries");
  assert!(
    inner > outer,
    "the nested item stands in from its parent: {inner} against {outer}"
  );
}

#[test]
fn review_thread_markdown_a_drag_over_the_answer_selects_its_words_and_copies_them_without_markup() {
  let (mut window, _stand_ins) = opened();
  let id = answer(&mut window);
  drag_over(&mut window, &id);
  let copied = window
    .tree
    .selected_text()
    .expect("the drag selects the answer's words");
  for part in ["every attempt", "then up to four retries", "delay_for()"] {
    assert!(copied.contains(part), "{part:?} is copied: {copied}");
  }
  assert!(!copied.contains("**"), "the copy keeps no Markdown marks: {copied}");
  assert!(
    window.texts("review-composer-text").is_none(),
    "a drag in a thread does not start the diff's selection of lines"
  );
}

#[test]
fn review_thread_markdown_the_dots_move_while_implement_starts_and_the_words_replace_them() {
  let (mut window, _stand_ins) = opened();
  let dots = scroll_to(&mut window, "the dots of an answer that has not started", |_, id| {
    id.ends_with("-starting-dots")
  });
  assert!(window.tree.has_active_timeline(), "dots in view ask for frames");
  let mut seen: [Vec<f32>; 3] = Default::default();
  let end = Instant::now() + Duration::from_millis(1500);
  while Instant::now() < end {
    window.pass();
    let now = dot_opacities(&mut window, &dots);
    assert_eq!(now.len(), 3, "three dots");
    for (dot, opacity) in now.into_iter().enumerate() {
      seen[dot].push(opacity);
    }
    thread::sleep(SAMPLE);
  }
  for (dot, opacities) in seen.iter().enumerate() {
    let low = opacities.iter().copied().fold(f32::MAX, f32::min);
    let high = opacities.iter().copied().fold(f32::MIN, f32::max);
    assert!(high - low > 0.4, "dot {dot} fades: {low}..{high}");
  }
  let written = scroll_to(&mut window, "an answer being written", |_, id| {
    id.ends_with("-message-1-streaming")
  });
  let row = written.trim_end_matches("-message-1-streaming");
  assert!(
    ids(&mut window)
      .iter()
      .all(|id| !id.starts_with(row) || !id.ends_with("-dots")),
    "once words arrived the dots are gone"
  );
  assert!(
    ids(&mut window).contains(&format!("{row}-message-1-caret")),
    "the words come with the caret"
  );
}

#[test]
fn review_thread_markdown_the_agents_name_reads_once() {
  let (mut window, _stand_ins) = opened();
  let id = answer(&mut window);
  let name = window.text(&id.replace("-body", "-name"));
  assert_eq!(name, "Implement · Builder", "the step and its agent, once each");
}
