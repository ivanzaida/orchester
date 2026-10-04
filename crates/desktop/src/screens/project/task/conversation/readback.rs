//! What a filter or a search leaves out, in lines under the toolbar with the way back (`SXbwX` in
//! `bdCsu`, `bm4Mg` in `Hn1xv`, `SwI8Y` in `JPBHA`; padding 10 / 32, a hairline under each): *Showing
//! 03 Implement only · 01, 02 and 04–07 are hidden ·* with *Show all steps*; *4 matches for
//! “callback” in 03 Implement and 07 Docs & cleanup · items without a match are hidden ·* with *Clear
//! search*; *No matches for “refresh token” · following live goes on ·*; *31 tool calls in 4 groups
//! hidden* with *Show all*. A search that finds nothing is also a state of its own, centred.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextOverflow},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};

use super::{
  super::tab_state,
  ShowChoice, StepFilter,
  divider::{list, step_label},
};
use crate::{
  tasks::{
    detail::RunView,
    timeline::{
      StepHead,
      rail::RailStep,
      view::{Choice, Show, Shown},
    },
  },
  theme::{ExtraColor, ExtraSpacing},
  ui::{
    button::ActionButton,
    icon::{Icon, IconSize, icon},
    pressable::{pressable, surface},
  },
};

/// What the read-back lines change.
pub(super) struct Controls<'a> {
  pub(super) step: &'a Signal<StepFilter>,
  pub(super) query: &'a Signal<String>,
  pub(super) show: &'a Signal<ShowChoice>,
}

/// The lines for what `choice` leaves out of `shown`; `live`: following live goes on during a search.
pub(super) fn readback(
  ctx: &Ctx,
  (choice, shown): (&Choice, &Shown),
  (rail, picked): (&[RailStep], Option<&RailStep>),
  controls: &Controls<'_>,
  live: bool,
) -> Vec<Element> {
  let mut lines = Vec::new();
  let needle = choice.query.trim();
  if !needle.is_empty() {
    lines.push(search_line(ctx, shown, needle, controls.query, live));
  }
  if let Some(picked) = picked {
    let hidden: Vec<usize> = rail
      .iter()
      .filter(|step| step.key != picked.key)
      .map(|step| step.number)
      .collect();
    let key = match hidden.len() {
      0 => "runs.conversation.showingOnlyAlone",
      1 => "runs.conversation.showingOnly.one",
      _ => "runs.conversation.showingOnly.other",
    };
    let text = ctx.t_args(
      key,
      [
        ("step", &*step_label(ctx, picked.number, &picked.name)),
        ("hidden", numbers(ctx, &hidden).as_str()),
      ],
    );
    let step = controls.step.clone();
    lines.push(line(
      "conversation-step-readback",
      Icon::Filter,
      &text,
      text_link(
        "conversation-show-all-steps",
        &ctx.t("runs.conversation.showAllSteps"),
        move || step.set(StepFilter(None)),
      ),
    ));
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
    let show = controls.show.clone();
    let all = text_link(
      "conversation-show-all",
      &ctx.t("runs.conversation.showAll"),
      move || show.set(ShowChoice(Show::All)),
    );
    lines.push(line("conversation-tools-readback", Icon::Wrench, &text, all));
  }
  lines
}

/// How many matches a search has and where, or that it has none.
fn search_line(ctx: &Ctx, shown: &Shown, needle: &str, query: &Signal<String>, live: bool) -> Element {
  let clear = {
    let query = query.clone();
    text_link(
      "conversation-clear-search",
      &ctx.t("runs.conversation.clearSearch"),
      move || query.set(String::new()),
    )
  };
  if shown.matches == 0 {
    let key = if live {
      "runs.conversation.noMatchesLive"
    } else {
      "runs.conversation.noMatches"
    };
    let text = ctx.t_args(key, [("query", needle)]);
    return line("conversation-search-readback", Icon::SearchX, &text, clear);
  }
  let key = if shown.matches == 1 {
    "runs.conversation.matches.one"
  } else {
    "runs.conversation.matches.other"
  };
  let text = ctx.t_args(
    key,
    [
      ("count", shown.matches.to_string().as_str()),
      ("query", needle),
      ("steps", &steps(ctx, &shown.matched_steps)),
    ],
  );
  line("conversation-search-readback", Icon::Search, &text, clear)
}

/// The search found nothing in the run, or in the step picked while other steps have matches
/// (`JPBHA`): what was searched, and the way back.
pub(super) fn nothing_found(
  ctx: &Ctx,
  run: &RunView,
  (choice, picked, elsewhere): (&Choice, Option<&RailStep>, Option<&Shown>),
  (step, query): (&Signal<StepFilter>, &Signal<String>),
) -> Element {
  let needle = choice.query.trim();
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
      vec![clear_search_button(ctx, query)],
    );
  };
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
      [
        ("step", &*step_label(ctx, picked.number, &picked.name)),
        ("query", needle),
      ],
    ),
    &ctx.t_args(
      key,
      [
        ("count", elsewhere.matches.to_string().as_str()),
        ("steps", &steps(ctx, &elsewhere.matched_steps)),
      ],
    ),
    vec![show_all_steps_button(ctx, step)],
  )
}

/// "02 Plan and 03 Implement": the steps, by number and name.
fn steps(ctx: &Ctx, heads: &[StepHead]) -> String {
  let mut labels: Vec<Arc<str>> = heads
    .iter()
    .map(|head| step_label(ctx, head.number, &head.step))
    .collect();
  labels.dedup();
  list(ctx, &labels)
}

/// "01, 02 and 04–07": step numbers, a run of three or more as a range.
fn numbers(ctx: &Ctx, numbers: &[usize]) -> String {
  let mut parts: Vec<Arc<str>> = Vec::new();
  let mut index = 0;
  while index < numbers.len() {
    let mut end = index;
    while end + 1 < numbers.len() && numbers[end + 1] == numbers[end] + 1 {
      end += 1;
    }
    if end - index >= 2 {
      parts.push(ctx.t_args(
        "runs.conversation.range",
        [
          ("first", format!("{:02}", numbers[index]).as_str()),
          ("last", format!("{:02}", numbers[end]).as_str()),
        ],
      ));
    } else {
      parts.extend(numbers[index..=end].iter().map(|number| format!("{number:02}").into()));
    }
    index = end + 1;
  }
  list(ctx, &parts)
}

/// A read-back line: its glyph, what is left out and the way back as a link.
fn line(id: &'static str, glyph: Icon, text: &str, action: Element) -> Element {
  Row::new()
    .id(id)
    .width(Dimension::Pct(100.0))
    .padding_vertical(ExtraSpacing::Inline.role())
    .padding_horizontal(SpacingSize::Section)
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(icon(glyph, IconSize::Small, PaletteColor::TextSecondary))
    .child(
      Text::new(text)
        .variant(TypographyStyle::Caption)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .min_width(Dimension::Px(0.0))
        .flex_shrink(1.0),
    )
    .child(Column::new().flex_shrink(0.0).child(action))
    .into()
}

/// A plain text link (11/500 `$text`), as the read-back lines draw their way back.
fn text_link(id: &'static str, label: &str, on_click: impl Fn() + Send + Sync + 'static) -> Element {
  pressable(surface().id(id), on_click)
    .child(
      Text::new(label)
        .variant(TypographyStyle::Link)
        .color(PaletteColor::TextPrimary)
        .nowrap(),
    )
    .into()
}

/// *Clear search* under a search that found nothing (`JPBHA`), with its ×.
fn clear_search_button(ctx: &Ctx, query: &Signal<String>) -> Element {
  let query = query.clone();
  ActionButton::new(ctx.t("runs.conversation.clearSearch"))
    .icon(Icon::X)
    .compact()
    .id("conversation-clear-search-state")
    .on_click(move || query.set(String::new()))
    .render(ctx)
}

/// *Show all steps* when the picked step has no match but others do, with its list.
fn show_all_steps_button(ctx: &Ctx, step: &Signal<StepFilter>) -> Element {
  let step = step.clone();
  ActionButton::new(ctx.t("runs.conversation.showAllSteps"))
    .icon(Icon::List)
    .compact()
    .id("conversation-show-all-steps-state")
    .on_click(move || step.set(StepFilter(None)))
    .render(ctx)
}
