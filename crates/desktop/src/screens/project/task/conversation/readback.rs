//! What a filter or a search leaves out, in a line under the toolbar with the way back
//! (`LhLL8`, `a5hF4`, `OlLGD`): *Showing 01 Research only · 02 Plan and 03 Implement are hidden.*
//! with *Show all steps*; *4 matches for “cancel” in 03 Implement · items without a match are
//! hidden.* with *Clear search*; *31 tool calls in 4 groups hidden* with *Show all*. A search that
//! finds nothing is a state of its own, centred (`gLVQQ`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};

use super::{super::tab_state, ShowChoice, StepFilter};
use crate::{
  tasks::{
    detail::RunView,
    timeline::{
      StepHead,
      view::{Choice, Show, Shown},
    },
  },
  theme::ExtraSpacing,
  ui::{button::ActionButton, icon::Icon},
};

/// The lines for what `choice` leaves out of `shown`, or `None` when it leaves nothing out.
pub(super) fn readback(
  ctx: &Ctx,
  run: &RunView,
  (choice, shown): (&Choice, &Shown),
  (step, query, show): (&Signal<StepFilter>, &Signal<String>, &Signal<ShowChoice>),
) -> Option<Element> {
  let mut lines = Vec::new();
  let searching = !choice.query.trim().is_empty();
  if searching && shown.matches > 0 {
    let key = if shown.matches == 1 {
      "runs.conversation.matches.one"
    } else {
      "runs.conversation.matches.other"
    };
    let text = ctx.t_args(
      key,
      [
        ("count", shown.matches.to_string().as_str()),
        ("query", choice.query.trim()),
        ("steps", &steps(ctx, &shown.matched_steps)),
      ],
    );
    lines.push(line("conversation-search-readback", &text, clear_search(ctx, query)));
  }
  if let Some(picked) = choice.step.and_then(|picked| run.step_of(picked)) {
    let picked = ctx.t_args(
      "runs.conversation.stepLabel",
      [
        ("number", format!("{:02}", picked.number).as_str()),
        ("step", &*picked.name),
      ],
    );
    let key = match shown.hidden_steps.len() {
      0 => "runs.conversation.showingOnlyAlone",
      1 => "runs.conversation.showingOnly.one",
      _ => "runs.conversation.showingOnly.other",
    };
    let text = ctx.t_args(key, [("step", &*picked), ("hidden", &steps(ctx, &shown.hidden_steps))]);
    lines.push(line("conversation-step-readback", &text, show_all_steps(ctx, step)));
  }
  let (calls, groups) = shown.hidden_tools;
  if choice.show == Show::Messages && groups > 0 {
    let text = ctx.t_args(
      if groups == 1 {
        "runs.conversation.toolsHidden.one"
      } else {
        "runs.conversation.toolsHidden.other"
      },
      [
        ("calls", calls.to_string().as_str()),
        ("groups", groups.to_string().as_str()),
      ],
    );
    let show = show.clone();
    let all = ActionButton::new(ctx.t("runs.conversation.showAll"))
      .compact()
      .id("conversation-show-all")
      .on_click(move || show.set(ShowChoice(Show::All)))
      .render(ctx);
    lines.push(line("conversation-tools-readback", &text, all));
  }
  (!lines.is_empty()).then(|| {
    Column::new()
      .width(Dimension::Pct(100.0))
      .padding_vertical(SpacingSize::Sm)
      .spacing(SpacingSize::Sm)
      .with_children(lines)
      .into()
  })
}

/// The search found nothing in the run, or in the step picked while other steps have matches
/// (`gLVQQ`): what was searched, and the way back.
pub(super) fn nothing_found(
  ctx: &Ctx,
  run: &RunView,
  (choice, elsewhere): (&Choice, Option<&Shown>),
  (step, query): (&Signal<StepFilter>, &Signal<String>),
) -> Element {
  let needle = choice.query.trim();
  let picked = choice.step.and_then(|picked| run.step_of(picked));
  let Some((picked, elsewhere)) = picked.zip(elsewhere.filter(|elsewhere| elsewhere.matches > 0)) else {
    let number = run.run.number().get().to_string();
    let ran = run.steps.iter().filter(|step| step.execution.is_some()).count();
    let searched = if ran == 1 {
      "runs.conversation.searched.one"
    } else {
      "runs.conversation.searched.other"
    };
    return tab_state::state(
      "conversation-nothing-found",
      Icon::SearchX,
      &ctx.t_args(
        "runs.conversation.nothingInRun",
        [("run", number.as_str()), ("query", needle)],
      ),
      &ctx.t_args(searched, [("count", ran.to_string().as_str())]),
      vec![clear_search(ctx, query)],
    );
  };
  let label = ctx.t_args(
    "runs.conversation.stepLabel",
    [
      ("number", format!("{:02}", picked.number).as_str()),
      ("step", &*picked.name),
    ],
  );
  let key = if elsewhere.matches == 1 {
    "runs.conversation.matchesElsewhere.one"
  } else {
    "runs.conversation.matchesElsewhere.other"
  };
  tab_state::state(
    "conversation-nothing-found",
    Icon::SearchX,
    &ctx.t_args(
      "runs.conversation.nothingInStep",
      [("step", &*label), ("query", needle)],
    ),
    &ctx.t_args(
      key,
      [
        ("count", elsewhere.matches.to_string().as_str()),
        ("steps", &steps(ctx, &elsewhere.matched_steps)),
      ],
    ),
    vec![show_all_steps(ctx, step)],
  )
}

/// "02 Plan and 03 Implement": the steps, by number and name.
fn steps(ctx: &Ctx, heads: &[StepHead]) -> String {
  let labels: Vec<Arc<str>> = heads
    .iter()
    .map(|head| {
      ctx.t_args(
        "runs.conversation.stepLabel",
        [
          ("number", format!("{:02}", head.number).as_str()),
          ("step", &*head.step),
        ],
      )
    })
    .collect();
  match labels.as_slice() {
    [] => String::new(),
    [one] => one.to_string(),
    [rest @ .., last] => {
      let rest: Vec<&str> = rest.iter().map(|label| &**label).collect();
      ctx
        .t_args(
          "common.listAnd",
          [
            ("rest", rest.join(&ctx.t("common.listSeparator")).as_str()),
            ("last", last),
          ],
        )
        .to_string()
    }
  }
}

fn line(id: &'static str, text: &str, action: Element) -> Element {
  Row::new()
    .id(id)
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Inline.role())
    .align_items(Alignment::Center)
    .child(Text::new(text).variant(TypographyStyle::Description))
    .child(action)
    .into()
}

fn clear_search(ctx: &Ctx, query: &Signal<String>) -> Element {
  let query = query.clone();
  ActionButton::new(ctx.t("runs.conversation.clearSearch"))
    .compact()
    .id("conversation-clear-search")
    .on_click(move || query.set(String::new()))
    .render(ctx)
}

fn show_all_steps(ctx: &Ctx, step: &Signal<StepFilter>) -> Element {
  let step = step.clone();
  ActionButton::new(ctx.t("runs.conversation.showAllSteps"))
    .compact()
    .id("conversation-show-all-steps")
    .on_click(move || step.set(StepFilter(None)))
    .render(ctx)
}
