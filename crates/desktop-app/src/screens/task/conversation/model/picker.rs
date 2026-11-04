//! The step's model picker (`Model menu` `LNBnH` in 08 `DPofN`, `l4SWxn` in 08b `p7r89Q`, the fixed one in 08e
//! `X2Hmq`): 380 wide, `surface_raised` in a `border_strong` stroke, 6 inside and 2 between rows, the large shadow.
//! Under the heading — the agent's mark and *Builder · Claude Code · Work · its models* (26 high, 11/500 `text_muted`)
//! — one row per model of the step's connection: its id in Geist Mono 12 and *The profile's model* under the profile's
//! in 11.5 `text_muted`; 6 over and under, 8 at the sides, 10 apart, radius 6; the chosen one on `hover` with a check,
//! the one in use tagged *now*. Then the effort (a 12 `text_muted` label 56 wide and its field, *now {effort}* in it,
//! whose options open under it, inside the picker), hidden for a model without efforts; *Applies* with its two options,
//! a radio and its words over what they reach in 11.5 `text_muted`, 24 in; and the foot: that the profile stays, with
//! *Cancel* and *Apply*, which waits until the model or effort differs from what the step runs on. A connection with
//! nothing to choose from shows its model locked, that it offers no effort, why and *Connections ↗*.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, ShadowStyle, SpacingSize},
  },
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::run::{ModelChoice, ModelScope};
use orchester_workspace::tasks::run_model::{OfferedModel, StepModel, change::ModelAsked};

use super::{Asked, ChipProps, Chosen, Draft, Failed, RunModel, choice_words, effort_word};
use crate::{
  navigation::Navigation,
  routes::{ConnectionsView, ProjectPage, Route},
  theme::{ExtraColor, ExtraSpacing, IconSize, ModelText},
  ui::{
    agent_mark::AgentMark,
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::{Icon, icon},
    menu::{MenuItem, separator},
    message::message_text,
    pressable::pressable,
    toggle::radio_mark,
  },
};

/// The picker's width (`LNBnH`), a heading's height (`LkZAz`) and the effort's label width (`OU3jd`).
const WIDTH: f32 = 380.0;
const HEADING_HEIGHT: f32 = 26.0;
const LABEL_WIDTH: f32 = 56.0;

pub(super) fn panel(ctx: &mut Ctx, props: &ChipProps, (model, draft): (&RunModel, &Draft)) -> Element {
  let step = &props.step;
  let heading = ctx.t_args(
    "conversation.model.menu.heading",
    [("agent", &*step.agent), ("connection", &*step.connection)],
  );
  let body = if step.fixed() {
    fixed(ctx, props)
  } else {
    choosing(ctx, props, (model, draft))
  };
  Column::new()
    .id("conversation-model-picker")
    .width(Dimension::Px(WIDTH))
    .padding(ExtraSpacing::Snug.role())
    .spacing(ExtraSpacing::Tight.role())
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
    .rounded(RadiusSize::Lg)
    .box_shadow(ShadowStyle::Lg)
    .child(
      Row::new()
        .id("conversation-model-heading")
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(HEADING_HEIGHT))
        .padding_horizontal(SpacingSize::Sm)
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(AgentMark::of(step.provider).render(ctx, IconSize::Sm))
        .child(Text::new(&heading).variant(ModelText::Heading.role()).nowrap()),
    )
    .with_children(body)
    .into()
}

/// The models, the effort, *Applies*, why the last change failed and the foot.
fn choosing(ctx: &mut Ctx, props: &ChipProps, (model, draft): (&RunModel, &Draft)) -> Vec<Element> {
  let step = &props.step;
  let chosen = draft.chosen().unwrap_or_else(|| step.now.clone());
  let mut rows: Vec<Element> = step
    .offered
    .iter()
    .enumerate()
    .map(|(index, offered)| model_row(ctx, step, (index, offered), (&chosen, draft)))
    .collect();
  if !step.efforts_of(&chosen.model).is_empty() {
    rows.push(separator(ctx));
    rows.extend(effort(ctx, step, &chosen, draft));
  }
  rows.push(separator(ctx));
  rows.push(heading(&ctx.t("conversation.model.applies")));
  rows.push(scope_row(ctx, props, ModelScope::Step, draft));
  rows.push(scope_row(ctx, props, ModelScope::Run, draft));
  rows.extend(model.failure().map(|failed| failure(ctx, step, &failed)));
  rows.push(foot(ctx, props, (model, draft), &chosen));
  rows
}

fn heading(words: &str) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEADING_HEIGHT))
    .padding_horizontal(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(Text::new(words).variant(ModelText::Heading.role()).nowrap())
    .into()
}

/// A model of the connection; choosing it keeps the effort chosen if the model offers it, else its profile's, else the
/// CLI's default.
fn model_row(
  ctx: &Ctx,
  step: &StepModel,
  (index, offered): (usize, &OfferedModel),
  (chosen, draft): (&ModelChoice, &Draft),
) -> Element {
  let picked = chosen.model == offered.id;
  let effort = [chosen.effort.as_ref(), step.profile.effort.as_ref()]
    .into_iter()
    .flatten()
    .find(|effort| offered.efforts.contains(effort))
    .cloned();
  let choice = ModelChoice {
    model: offered.id.clone(),
    effort,
  };
  let set = draft.chosen.clone();
  let note = (offered.id == step.profile.model).then(|| {
    Text::new(&ctx.t("conversation.model.profileModel"))
      .variant(ModelText::Sub.role())
      .nowrap()
  });
  let end: Option<Element> = if picked {
    Some(icon(Icon::Check, IconSize::Md, PaletteColor::TextPrimary).into())
  } else {
    (offered.id == step.now.model).then(|| {
      Text::new(&ctx.t("conversation.model.now"))
        .variant(ModelText::Tag.role())
        .nowrap()
        .into()
    })
  };
  let row = pressable(move || set.set(Chosen(Some(choice.clone()))))
    .id(format!("conversation-model-option-{index}"))
    .describe("model", offered.id.as_str().to_owned())
    .describe("chosen", picked.to_string())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Snug.role()))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Md)
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(Text::new(offered.id.as_str()).variant(ModelText::Name.role()).nowrap())
        .with_children(note),
    )
    .with_children(end);
  if picked {
    row.background(ExtraColor::Hover.role()).into()
  } else {
    row
      .hovered_style(Style::new().background(ExtraColor::Hover.role()))
      .into()
  }
}

/// The effort's row and, while it is open, its options: the CLI's default, then each effort the chosen model offers.
fn effort(ctx: &mut Ctx, step: &StepModel, chosen: &ModelChoice, draft: &Draft) -> Vec<Element> {
  let shown = draft.efforts.get();
  let words = chosen.effort.as_deref().map_or_else(
    || ctx.t("conversation.model.effortDefault"),
    |effort| effort_word(ctx, effort),
  );
  let now = step.now.effort.as_deref().map(|effort| {
    let effort = effort_word(ctx, effort);
    Text::new(&ctx.t_args("conversation.model.effortNow", [("effort", &*effort)]))
      .variant(ModelText::Sub.role())
      .nowrap()
  });
  let toggle = draft.efforts.clone();
  let field = pressable(move || toggle.update(|open| *open = !*open))
    .id("conversation-model-effort")
    .describe("state", if shown { "open" } else { "closed" })
    .describe("value", chosen.effort.clone().unwrap_or_default())
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Md)
    .background(PaletteColor::SurfaceInput)
    .border(Border::inside(
      BorderSize::Sm,
      if shown {
        PaletteColor::BorderFocus
      } else {
        PaletteColor::Border
      },
    ))
    .child(
      Text::new(&words)
        .id("conversation-model-effort-value")
        .variant(ModelText::Value.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .nowrap(),
    )
    .with_children(now)
    .child(icon(Icon::ChevronDown, IconSize::Md, PaletteColor::TextMuted));
  let row = Row::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Sm, SpacingSize::Xs))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t("conversation.model.effort"))
        .variant(ModelText::Label.role())
        .width(Dimension::Px(LABEL_WIDTH))
        .flex_shrink(0.0),
    )
    .child(field);
  let mut rows = vec![row.into()];
  if shown {
    rows.push(effort_options(ctx, step, chosen, draft));
  }
  rows
}

fn effort_options(ctx: &mut Ctx, step: &StepModel, chosen: &ModelChoice, draft: &Draft) -> Element {
  let options = std::iter::once(None).chain(step.efforts_of(&chosen.model).iter().cloned().map(Some));
  let items: Vec<Element> = options
    .enumerate()
    .map(|(index, effort)| {
      let label = effort.as_deref().map_or_else(
        || ctx.t("conversation.model.effortDefault"),
        |effort| effort_word(ctx, effort),
      );
      let picked = effort == chosen.effort;
      let choice = ModelChoice {
        model: chosen.model.clone(),
        effort,
      };
      let set = draft.chosen.clone();
      MenuItem::new(format!("conversation-model-effort-option-{index}"), label, move || {
        set.set(Chosen(Some(choice.clone())));
      })
      .chosen(picked)
      .render(ctx, &draft.efforts)
    })
    .collect();
  Column::new()
    .id("conversation-model-effort-menu")
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().left(SpacingSize::Sm).right(SpacingSize::Sm))
    .spacing(ExtraSpacing::Tight.role())
    .with_children(items)
    .into()
}

/// An option of *Applies*: a radio and its words, what it reaches under them; the chosen one on `hover`.
fn scope_row(ctx: &Ctx, props: &ChipProps, scope: ModelScope, draft: &Draft) -> Element {
  let on = draft.scope() == scope;
  let (id, name, note) = match scope {
    ModelScope::Step => (
      "conversation-model-scope-step",
      "conversation.model.scope.step",
      step_note(ctx, props),
    ),
    ModelScope::Run => (
      "conversation-model-scope-run",
      "conversation.model.scope.run",
      run_note(ctx, props),
    ),
  };
  let name = ctx.t(name);
  let set = draft.for_run.clone();
  let row = pressable(move || set.set(scope == ModelScope::Run))
    .id(id)
    .describe("chosen", on.to_string())
    .describe("a11y_name", name.clone())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Snug.role()))
    .rounded(RadiusSize::Md)
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Fine.role())
        .child(
          Row::new()
            .spacing(SpacingSize::Sm)
            .align_items(Alignment::Center)
            .child(radio_mark(on))
            .child(
              Text::new(&name)
                .variant(lurq::app::theme::TypographyStyle::Description)
                .color(PaletteColor::TextPrimary)
                .nowrap(),
            ),
        )
        .child(
          Row::new()
            .width(Dimension::Pct(100.0))
            .padding(Padding::new().left(SpacingSize::Lg))
            .child(
              Text::new(&note)
                .id(format!("{id}-note"))
                .variant(ModelText::Sub.role())
                .flex(1.0)
                .min_width(Dimension::Px(0.0)),
            ),
        ),
    );
  if on {
    row.background(ExtraColor::Hover.role()).into()
  } else {
    row
      .hovered_style(Style::new().background(ExtraColor::Hover.role()))
      .into()
  }
}

fn step_note(ctx: &Ctx, props: &ChipProps) -> Arc<str> {
  let names = &props.names;
  let later: Vec<&str> = names
    .changing
    .iter()
    .chain(&names.keeping)
    .map(|name| &**name)
    .collect();
  if later.is_empty() {
    return ctx.t_args("conversation.model.scope.stepNoteAlone", [("step", &*names.step)]);
  }
  let later = later.join(", ");
  ctx.t_args(
    "conversation.model.scope.stepNote",
    [("step", &*names.step), ("later", later.as_str())],
  )
}

fn run_note(ctx: &Ctx, props: &ChipProps) -> Arc<str> {
  let (names, step) = (&props.names, &props.step);
  let changing = names.changing.iter().map(|name| &**name).collect::<Vec<_>>().join(", ");
  let keeping = names.keeping.iter().map(|name| &**name).collect::<Vec<_>>().join(", ");
  let key = match (changing.is_empty(), keeping.is_empty()) {
    (false, false) => "conversation.model.scope.runNote",
    (false, true) => "conversation.model.scope.runNoteChanging",
    (true, false) => "conversation.model.scope.runNoteKeeping",
    (true, true) => "conversation.model.scope.runNoteAlone",
  };
  ctx.t_args(
    key,
    [
      ("changing", changing.as_str()),
      ("keeping", keeping.as_str()),
      ("agent", &*step.agent),
      ("connection", &*step.connection),
    ],
  )
}

fn failure(ctx: &Ctx, step: &StepModel, failed: &Failed) -> Element {
  let reason = match failed {
    Failed::NotOffered => ctx.t_args("conversation.model.notOffered", [("connection", &*step.connection)]),
    Failed::NotWaiting => ctx.t("conversation.model.notWaiting"),
    Failed::Unavailable => ctx.t("conversation.model.unavailable"),
    Failed::Other(message) => message_text(ctx, message),
  };
  field_error(
    "conversation-model-error",
    &ctx.t_args("conversation.model.failed", [("reason", &*reason)]),
  )
  .into()
}

/// That the profile stays on its model, *Cancel* and *Apply*; *Apply* waits until the choice differs from now.
fn foot(ctx: &mut Ctx, props: &ChipProps, (model, draft): (&RunModel, &Draft), chosen: &ModelChoice) -> Element {
  let step = &props.step;
  let note = ctx.t_args(
    "conversation.model.profileStays",
    [("agent", &*step.agent), ("choice", &*choice_words(ctx, &step.profile))],
  );
  let cancel = {
    let open = model.open.clone();
    ActionButton::new(
      "conversation-model-cancel",
      ButtonKind::Ghost,
      ctx.t("conversation.model.cancel"),
    )
    .on_click(move || open.set(false))
  };
  let apply = ActionButton::new(
    "conversation-model-apply",
    ButtonKind::Primary,
    ctx.t("conversation.model.apply"),
  );
  let apply = if *chosen == step.now {
    apply.disabled()
  } else {
    let (model, root, run) = (model.clone(), props.root.clone(), props.run);
    let asked = ModelAsked {
      step: step.step.clone(),
      to: chosen.clone(),
      scope: draft.scope(),
    };
    apply.on_click(move || {
      model.open.set(false);
      model.ask(&root, run, Asked::Change(asked.clone()));
    })
  };
  Row::new()
    .id("conversation-model-foot")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(SpacingSize::Sm)
        .left(SpacingSize::Sm)
        .right(SpacingSize::Sm)
        .bottom(SpacingSize::Xs),
    )
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Text::new(&note)
        .id("conversation-model-foot-note")
        .variant(ModelText::Sub.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(cancel)
    .child(apply)
    .into()
}

/// A connection with nothing to choose from: its model locked, that it offers no effort, why, and *Connections ↗*.
fn fixed(ctx: &mut Ctx, props: &ChipProps) -> Vec<Element> {
  let step = &props.step;
  let label = |words: Arc<str>| {
    Text::new(&words)
      .variant(ModelText::Label.role())
      .width(Dimension::Px(LABEL_WIDTH))
      .flex_shrink(0.0)
  };
  let line = |child: Element| {
    Row::new()
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(SpacingSize::Sm, SpacingSize::Xs))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(child)
  };
  let locked = Row::new()
    .id("conversation-model-locked")
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Md)
    .background(PaletteColor::SurfacePanel)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Text::new(step.now.model.as_str())
        .variant(ModelText::Name.role())
        .nowrap(),
    )
    .child(Spacer::new().flex(1.0))
    .child(icon(Icon::Lock, IconSize::Sm, PaletteColor::TextMuted));
  let model = line(label(ctx.t("conversation.model.fixed.model")).into()).child(locked);
  let effort = line(label(ctx.t("conversation.model.effort")).into()).child(
    Text::new(&ctx.t("conversation.model.fixed.effortNone"))
      .id("conversation-model-effort-none")
      .variant(ModelText::Label.role())
      .nowrap(),
  );
  let open = Navigation::of(ctx).to(Route::Project(
    props.project,
    ProjectPage::Connections(ConnectionsView::default()),
  ));
  let link = ActionButton::new(
    "conversation-model-connections",
    ButtonKind::Ghost,
    ctx.t("conversation.model.fixed.link"),
  )
  .after(Icon::ArrowUpRight)
  .opens(open);
  let foot = Row::new()
    .id("conversation-model-foot")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(SpacingSize::Sm)
        .left(SpacingSize::Sm)
        .right(SpacingSize::Sm)
        .bottom(SpacingSize::Xs),
    )
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::End)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Text::new(&ctx.t_args("conversation.model.fixed.note", [("agent", &*step.agent)]))
        .id("conversation-model-fixed-note")
        .variant(ModelText::Sub.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(link);
  vec![model.into(), effort.into(), foot.into()]
}
