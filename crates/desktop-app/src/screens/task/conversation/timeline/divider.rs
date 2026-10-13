//! Dividers of the chat: a 1 `border` rule at each side of a label. A step starts with its divider (`Step
//! divider` `qfdli`: the step, its agent, engine, times and tokens in 11/500 `text_muted`, 10 from the rules);
//! pressing a finished step's folds it. The other lines are event dividers (`Event divider` `YKpmi`: a 12
//! icon and 11/500 `text_secondary` words, 8 apart, and a link in `accent_text`): finished steps in a row
//! folded into one (*01–04 · 4 steps done · 16:20–16:24 · Show them*, `list-collapse`), the steps a failure
//! kept from starting, a return or restart (*Review returned the work to 03 Implement · attempt 2 · 16:31*,
//! `corner-down-left`) and the run's end.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextOverflow},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  connection::{AgentProvider, EndpointPreset, EngineVendor},
  time::Timestamp,
  tokens::compact_tokens,
};
use orchester_workspace::{
  display,
  tasks::timeline::{
    RunEnd, StepHead,
    mark::{Mark, MarkKind},
  },
};

use super::{Timeline, message::person_note};
use crate::{
  theme::{ConversationText, ExtraColor, ExtraSpacing, IconSize},
  ui::{
    icon::{Icon, icon},
    pressable::pressable,
  },
};

/// The rules' thickness (`IecXN`).
const RULE_HEIGHT: f32 = 1.0;

fn rule() -> Row {
  Row::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .height(Dimension::Px(RULE_HEIGHT))
    .background(PaletteColor::Border)
}

/// `label` between the rules, `gap` from them.
fn ruled(id: String, gap: SpacingSize, label: Element) -> Element {
  Row::new()
    .id(id)
    .width(Dimension::Pct(100.0))
    .spacing(gap)
    .align_items(Alignment::Center)
    .child(rule())
    .child(label)
    .child(rule())
    .into()
}

/// An event divider: `glyph`, `text` and `link` (its words and what a press does).
/// A link's words and what a press on it does.
type Link = (Arc<str>, Box<dyn Fn() + Send + Sync>);

fn event(id: String, glyph: Icon, text: &str, link: Option<Link>) -> Element {
  let link = link.map(|(words, action)| {
    pressable(action).id(format!("{id}-link")).child(
      Text::new(&words)
        .variant(TypographyStyle::FieldLabel)
        .color(ExtraColor::AccentText.role())
        .nowrap(),
    )
  });
  let label = Row::new()
    .flex_shrink(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Sm, PaletteColor::TextSecondary))
    .child(
      Text::new(text)
        .id(format!("{id}-text"))
        .variant(TypographyStyle::FieldLabel)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .min_width(Dimension::Px(0.0))
        .flex_shrink(1.0),
    )
    .with_children(link);
  ruled(id, SpacingSize::Sm, label.into())
}

/// "03 Implement".
pub(crate) fn step_label(ctx: &Ctx, number: usize, step: &str) -> Arc<str> {
  ctx.t_args(
    "conversation.stepLabel",
    [("number", format!("{number:02}").as_str()), ("step", step)],
  )
}

/// A step's divider (`qfdli`); a finished one folds when pressed.
pub(super) fn step(ctx: &Ctx, timeline: &Timeline, (head, key): (&StepHead, &Arc<str>), foldable: bool) -> Element {
  let text = Text::new(&step_text(ctx, head))
    .id("conversation-step-text")
    .variant(ConversationText::Divider.role())
    .nowrap()
    .text_overflow(TextOverflow::Elipsis)
    .min_width(Dimension::Px(0.0))
    .flex_shrink(1.0);
  let line = ruled(
    format!("conversation-step-{key}"),
    ExtraSpacing::Loose.role(),
    text.into(),
  );
  if !foldable {
    return line;
  }
  let (toggles, key) = (timeline.toggles.clone(), key.clone());
  pressable(move || toggles.flip(&key, true))
    .id("conversation-step-fold")
    .width(Dimension::Pct(100.0))
    .child(line)
    .into()
}

/// "05 Review · Reviewer · Claude / opus-4.1 · started 16:24 · 41k tokens", "04 Run checks · tool ·
/// 16:38–16:40".
fn step_text(ctx: &Ctx, head: &StepHead) -> Arc<str> {
  let step = step_label(ctx, head.number, &head.step);
  let time = match head.ended {
    Some(ended) => span(ctx, head.started, ended),
    None => {
      let start = head.started.map(display::clock).unwrap_or_default();
      ctx.t_args("conversation.divider.started", [("start", start.as_str())])
    }
  };
  if head.command {
    return ctx.t_args("conversation.divider.tool", [("step", &*step), ("time", &*time)]);
  }
  let engine = engine(ctx, (head.provider, head.vendor.as_ref()), &head.model);
  let engine = match head.attempt.filter(|attempt| *attempt > 1) {
    Some(attempt) => ctx.t_args(
      "conversation.divider.engineAttempt",
      [("engine", &*engine), ("attempt", attempt.to_string().as_str())],
    ),
    None => engine,
  };
  let text = ctx.t_args(
    "conversation.divider.agent",
    [
      ("step", &*step),
      ("agent", &*head.agent),
      ("engine", &*engine),
      ("time", &*time),
    ],
  );
  match head.usage.counted() {
    0 => text,
    tokens => ctx.t_args(
      "conversation.divider.tokens",
      [("text", &*text), ("tokens", compact_tokens(tokens).as_str())],
    ),
  }
}

/// Who receives an agent's prompts and the model it runs on ("Claude / opus-4.1", "DeepSeek /
/// deepseek-v4-pro"); the model alone once the profile's connection is gone. Orchester runs a native agent
/// itself (*· native*).
fn engine(ctx: &Ctx, (provider, vendor): (Option<AgentProvider>, Option<&EngineVendor>), model: &str) -> Arc<str> {
  let name = match vendor {
    Some(EngineVendor::Provider(provider)) => ctx.t(provider_key(*provider)),
    Some(EngineVendor::Preset(preset)) => ctx.t(preset_key(*preset)),
    Some(EngineVendor::Host(host)) => host.as_str().into(),
    None => return model.into(),
  };
  let engine = ctx.t_args("conversation.engine", [("provider", &*name), ("model", model)]);
  match provider {
    Some(provider) if provider.is_native() => ctx.t_args("conversation.engineNative", [("engine", &*engine)]),
    _ => engine,
  }
}

const fn provider_key(provider: AgentProvider) -> &'static str {
  match provider {
    AgentProvider::CodexCli => "conversation.provider.codex",
    AgentProvider::ClaudeCodeCli => "conversation.provider.claude",
    AgentProvider::AnthropicApi => "conversation.provider.anthropicApi",
    AgentProvider::OpenAiApi => "conversation.provider.openaiApi",
  }
}

const fn preset_key(preset: EndpointPreset) -> &'static str {
  match preset {
    EndpointPreset::Official => "conversation.preset.official",
    EndpointPreset::DeepSeek => "conversation.preset.deepseek",
    EndpointPreset::Custom => "conversation.preset.custom",
  }
}

/// "12:01–12:02", or one time when both fall on the same minute.
fn span(ctx: &Ctx, started: Option<Timestamp>, ended: Timestamp) -> Arc<str> {
  let end = display::clock(ended);
  match started.map(display::clock) {
    Some(start) if start != end => ctx.t_args("conversation.span", [("start", start.as_str()), ("end", end.as_str())]),
    _ => end.as_str().into(),
  }
}

/// Finished steps folded into one line, with *Show them*.
pub(super) fn folded(ctx: &Ctx, timeline: &Timeline, key: &Arc<str>, steps: &[(Arc<str>, StepHead)]) -> Element {
  let (Some((_, first)), Some((_, last))) = (steps.first(), steps.last()) else {
    return Column::new().into();
  };
  let numbers: Arc<str> = if first.number == last.number {
    format!("{:02}", first.number).into()
  } else {
    let (first, last) = (format!("{:02}", first.number), format!("{:02}", last.number));
    ctx.t_args(
      "conversation.range",
      [("first", first.as_str()), ("last", last.as_str())],
    )
  };
  let count = steps.len().to_string();
  let time = last
    .ended
    .map_or_else(|| "".into(), |ended| span(ctx, first.started, ended));
  let (text_key, link_key) = if steps.len() == 1 {
    ("conversation.fold.one", "conversation.fold.showIt")
  } else {
    ("conversation.fold.other", "conversation.fold.showThem")
  };
  let text = ctx.t_args(
    text_key,
    [("steps", &*numbers), ("count", count.as_str()), ("time", &*time)],
  );
  let (toggles, keys) = (
    timeline.toggles.clone(),
    steps.iter().map(|(key, _)| key.clone()).collect::<Vec<_>>(),
  );
  let show: Box<dyn Fn() + Send + Sync> = Box::new(move || toggles.open_all(keys.clone()));
  event(
    format!("conversation-{key}"),
    Icon::ListCollapse,
    &text,
    Some((ctx.t(link_key), show)),
  )
}

/// "05 Review, 06 Fix notes and 07 Docs & cleanup did not start".
pub(super) fn not_started(ctx: &Ctx, steps: &[StepHead]) -> Element {
  let labels: Vec<Arc<str>> = steps
    .iter()
    .map(|head| step_label(ctx, head.number, &head.step))
    .collect();
  let key = if steps.len() == 1 {
    "conversation.notStarted.one"
  } else {
    "conversation.notStarted.other"
  };
  let text = ctx.t_args(key, [("steps", list(ctx, &labels).as_str())]);
  event("conversation-not-started".into(), Icon::CircleDashed, &text, None)
}

/// "02 Plan and 03 Implement": `labels` joined as a list.
fn list(ctx: &Ctx, labels: &[Arc<str>]) -> String {
  match labels {
    [] => String::new(),
    [one] => one.to_string(),
    [rest @ .., last] => {
      let rest: Vec<&str> = rest.iter().map(|label| &**label).collect();
      let separator = ctx.t("conversation.list.separator");
      ctx
        .t_args(
          "conversation.list.and",
          [("rest", rest.join(&separator).as_str()), ("last", last)],
        )
        .to_string()
    }
  }
}

/// A return or restart: what returned the work, to which step, in which attempt; a person's note is their
/// message under it.
pub(super) fn mark(ctx: &mut Ctx, timeline: &Timeline, mark: &Mark) -> Element {
  let time = mark.at.map(display::clock).unwrap_or_default();
  let attempt = mark.attempt.to_string();
  let labelled = |ctx: &Ctx, name: &Arc<str>| labelled(ctx, timeline, name);
  let (glyph, text) = match &mark.kind {
    MarkKind::Returned { to, failed, .. } => {
      let key = if mark.by_person {
        "conversation.mark.returnedByYou"
      } else {
        "conversation.mark.returned"
      };
      let args = [
        ("failed", &**failed),
        ("step", &*labelled(ctx, to)),
        ("attempt", attempt.as_str()),
        ("time", time.as_str()),
      ];
      (Icon::CornerDownLeft, ctx.t_args(key, args))
    }
    MarkKind::Restarted { step } => {
      let key = if mark.by_person {
        "conversation.mark.restartedByYou"
      } else {
        "conversation.mark.restarted"
      };
      let args = [
        ("step", &*labelled(ctx, step)),
        ("attempt", attempt.as_str()),
        ("time", time.as_str()),
      ];
      (Icon::RotateCcw, ctx.t_args(key, args))
    }
    MarkKind::ReturnedWithNote { to, .. } => {
      let args = [
        ("step", &*labelled(ctx, to)),
        ("attempt", attempt.as_str()),
        ("time", time.as_str()),
      ];
      (
        Icon::CornerDownLeft,
        ctx.t_args("conversation.mark.returnedWithNote", args),
      )
    }
  };
  let line = event(format!("conversation-mark-{}", mark.attempt), glyph, &text, None);
  let MarkKind::ReturnedWithNote { note, .. } = &mark.kind else {
    return line;
  };
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .child(line)
    .child(person_note(
      ctx,
      &format!("conversation-mark-note-{}", mark.attempt),
      note,
      mark.at,
    ))
    .into()
}

/// "03 Implement" for the step named `name` of the run, or the name alone once its pipeline has no such
/// step.
fn labelled(ctx: &Ctx, timeline: &Timeline, name: &Arc<str>) -> Arc<str> {
  timeline
    .detail
    .run
    .as_ref()
    .and_then(|run| run.steps.iter().find(|step| step.name == *name))
    .map_or_else(|| name.clone(), |step| step_label(ctx, step.number, &step.name))
}

/// The run's end.
pub(super) fn end(ctx: &Ctx, timeline: &Timeline, end: &RunEnd, at: Timestamp) -> Element {
  let (number, time) = (timeline.run_number().to_string(), display::clock(at));
  let (run, time) = (("run", number.as_str()), ("time", time.as_str()));
  let (glyph, text) = match end {
    RunEnd::Ready => (Icon::GitPullRequest, ctx.t_args("conversation.end.ready", [run, time])),
    RunEnd::Merged(branch) => (
      Icon::GitMerge,
      ctx.t_args("conversation.end.merged", [run, ("branch", &**branch), time]),
    ),
    RunEnd::Failed(step) => (
      Icon::CircleX,
      ctx.t_args("conversation.end.failed", [run, ("step", &**step), time]),
    ),
    RunEnd::Cancelled => (Icon::Square, ctx.t_args("conversation.end.cancelled", [run, time])),
  };
  event("conversation-end".into(), glyph, &text, None)
}
