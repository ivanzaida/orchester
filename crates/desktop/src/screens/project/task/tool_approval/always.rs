//! The *Always allow* step of the approval dialog: exactly what is added and where, before anything is.
//! A command (`yBfuQ` in `NV6xJ`): the prefix to add, the program and its subcommand first and the whole
//! command as the narrowest choice, and the profile version it is published as, which runs keep it and
//! what happens to an unsaved draft. An MCP tool (`cYrgT` in `XKja4`): the tool, the server setting it
//! goes into, every profile that gets the server, and that it starts now. *Back* returns to the request;
//! the primary adds and approves.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, Row, ScrollVertical, Text},
  core::Signal,
  layout::Alignment,
  node::{CursorIcon, Element, border::Border, dimension::Dimension},
};
use orchester_core::{permission::CommandPrefix, resource::VersionNumber, run::ApprovalAnswer};

use super::{super::approval_value::ValueRows, Pressed, View, footer, request};
use crate::{
  tasks::approvals::{AlwaysShown, CommandAlways, ShownApproval},
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography, select_menu_scrollbar},
  ui::{
    form::Submit,
    icon::{Icon, IconSize, icon},
    pressable::{Ring, keyboard_stop, stroke_selected_focused},
    property::{ValueStyle, property, styled_property},
  },
};

/// A prefix or a target row (`yJXn1`): 34 high.
const ROW_HEIGHT: f32 = 34.0;

/// The step's sections, and what Enter does: the primary.
pub(super) fn sections(ctx: &mut Ctx, view: &View<'_>) -> (Vec<Element>, Submit) {
  let shown = view.shown();
  let Ok(always) = &shown.choices.always else {
    // `modal` opens this step only for an offered answer.
    return (Vec::new(), None);
  };
  let Content {
    title,
    explanation,
    body,
    primary,
  } = match always {
    AlwaysShown::Command(command) => command_content(ctx, view, command),
    AlwaysShown::Tool {
      server,
      tool,
      profiles,
      others,
    } => {
      let names = [
        ("server", &**server),
        ("tool", &**tool),
        ("agent", shown.approval.agent.as_str()),
      ];
      Content {
        title: ctx.t_args("runs.toolApproval.always.toolTitle", names),
        explanation: ctx.t_args("runs.toolApproval.always.toolExplanation", names),
        body: tool_body(ctx, shown, (server, tool), (profiles, others)),
        primary: (
          ctx.t_args("runs.toolApproval.always.addTool", [("server", &**server)]),
          Some(ApprovalAnswer::Always { prefix: None }),
        ),
      }
    }
  };
  let submit: Submit = match (&primary.1, view.busy) {
    (Some(answer), false) => {
      Some(Arc::new(view.answer(Pressed::Always, answer.clone())) as Arc<dyn Fn() + Send + Sync>)
    }
    _ => None,
  };
  let details = Column::new()
    .id("tool-approval-always-details")
    .width(Dimension::Pct(100.0))
    .padding_vertical(ExtraSpacing::Step.role())
    .padding_horizontal(ExtraSpacing::Rail.role())
    .spacing(ExtraSpacing::Stack.role())
    .child(request(ctx, view))
    .child(property(&ctx.t("runs.toolApproval.agent"), &request::agent(ctx, shown)))
    .with_children(body);
  let sections = vec![
    header(&title, &explanation),
    ScrollVertical::new(details)
      .width(Dimension::Pct(100.0))
      .flex_shrink(1.0)
      .scrollbar(select_menu_scrollbar())
      .into(),
    footer::always(ctx, view, primary),
  ];
  (sections, submit)
}

/// The request the step adds for (`Mchq9`, `NIVdj`): a command whole, or the server and the tool.
fn request(ctx: &Ctx, view: &View<'_>) -> Element {
  let approval = &view.shown().approval;
  if approval.command {
    return ValueRows::new(ctx, view.copied, view.arguments).command(&approval.action);
  }
  let tool = ctx.t_args(
    "runs.toolApproval.always.toolValue",
    [("server", approval.server.as_str()), ("tool", approval.action.as_str())],
  );
  styled_property(&ctx.t("runs.toolApproval.always.tool"), &tool, ValueStyle::Mono)
}

/// What the step shows for one kind of request, and its primary: its label and answer.
struct Content {
  title: Arc<str>,
  explanation: Arc<str>,
  body: Vec<Element>,
  primary: (Arc<str>, Option<ApprovalAnswer>),
}

/// A command's step: the chosen prefix (the widest until another is chosen) and where it goes.
fn command_content(ctx: &Ctx, view: &View<'_>, command: &CommandAlways) -> Content {
  let CommandAlways {
    prefixes,
    profile,
    pinned,
    next,
    draft,
    pipelines,
  } = command;
  let chosen = prefixes.get(view.prefix.get()).or_else(|| prefixes.first());
  let prefix = chosen.map_or("", CommandPrefix::as_str);
  let names = [("prefix", prefix), ("profile", &**profile)];
  let version = next.to_string();
  Content {
    title: ctx.t_args("runs.toolApproval.always.commandTitle", names),
    explanation: ctx.t_args("runs.toolApproval.always.commandExplanation", names),
    body: command_body(
      ctx,
      view,
      prefixes,
      (profile, prefix),
      (*pinned, *next, *draft, pipelines),
    ),
    primary: (
      ctx.t_args("runs.toolApproval.always.addCommand", [("version", version.as_str())]),
      chosen.map(|prefix| ApprovalAnswer::Always {
        prefix: Some(prefix.clone()),
      }),
    ),
  }
}

fn header(title: &str, explanation: &str) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_vertical(SpacingSize::Lg)
    .padding_horizontal(ExtraSpacing::Rail.role())
    .spacing(SpacingSize::Sm)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(
      Text::new(title)
        .id("tool-approval-always-title")
        .variant(ExtraTypography::DialogTitle.role()),
    )
    .child(Text::new(explanation).variant(ExtraTypography::Explanation.role()))
    .into()
}

/// A command's prefix choices and the profile version note.
fn command_body(
  ctx: &Ctx,
  view: &View<'_>,
  prefixes: &[CommandPrefix],
  (profile, prefix): (&str, &str),
  (pinned, next, draft, pipelines): (Option<VersionNumber>, VersionNumber, bool, &[Arc<str>]),
) -> Vec<Element> {
  let chosen = view.prefix.get();
  let rows = prefixes.iter().enumerate().map(|(index, offered)| {
    let detail = if index + 1 == prefixes.len() && prefixes.len() > 1 {
      ctx.t("runs.toolApproval.always.narrowest")
    } else {
      ctx.t("runs.toolApproval.always.widest")
    };
    prefix_row(view.prefix, (index, chosen == index), offered.as_str(), &detail)
  });
  let choices = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Tight.role())
    .child(Text::new(&ctx.t("runs.toolApproval.always.prefixLabel")).variant(TypographyStyle::Caption))
    .with_children(rows);
  let next_label = next.to_string();
  let pinned_label = pinned.map(|pinned| pinned.to_string()).unwrap_or_default();
  let run = view.shown().run.map(|run| run.get().to_string()).unwrap_or_default();
  let names = [
    ("prefix", prefix),
    ("profile", profile),
    ("version", next_label.as_str()),
    ("pinned", pinned_label.as_str()),
    ("run", run.as_str()),
  ];
  let mut lines = vec![ctx.t_args("runs.toolApproval.always.versionStarts", names)];
  if draft {
    lines.push(ctx.t_args("runs.toolApproval.always.versionDraft", names));
  }
  if !pipelines.is_empty() {
    let used = pipelines.iter().map(|name| &**name).collect::<Vec<_>>().join(", ");
    lines.push(ctx.t_args(
      "runs.toolApproval.always.versionPipelines",
      [("profile", profile), ("pipelines", used.as_str())],
    ));
  }
  let heading = ctx.t_args("runs.toolApproval.always.versionHeading", names);
  vec![choices.into(), note(Icon::History, &heading, &lines)]
}

/// One prefix choice (`yJXn1`, `DMzW4`): a radio, the prefix and what it covers; the chosen one has the
/// 1.5 `$text` stroke.
fn prefix_row(choice: &Signal<usize>, (index, selected): (usize, bool), prefix: &str, detail: &str) -> Element {
  let (mark, stroke, stroke_color) = if selected {
    (Icon::CircleDot, BorderSize::Md, PaletteColor::TextPrimary)
  } else {
    (Icon::Circle, BorderSize::Sm, ExtraColor::Hairline.role())
  };
  let button = Button::empty()
    .button()
    .id(format!("tool-approval-prefix-{index}"))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ROW_HEIGHT))
    .padding_horizontal(ExtraSpacing::Inline.role())
    .spacing(ExtraSpacing::Inline.role())
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Sm)
    .border(Border::inside(stroke, stroke_color))
    .describe("selected", if selected { "true" } else { "false" })
    .child(icon(mark, IconSize::Button, PaletteColor::TextPrimary))
    .child(Text::new(prefix).variant(ExtraTypography::MonoField.role()).nowrap())
    .child(Text::new(detail).variant(TypographyStyle::Caption).flex(1.0));
  let choice = choice.clone();
  keyboard_stop(button, Ring::Outer)
    .focused_style(stroke_selected_focused(Ring::Outer, selected))
    .cursor(CursorIcon::Pointer)
    .on_click(move |_| choice.set(index))
    .into()
}

/// An MCP tool: what is added, where, and the profiles it affects; then that it starts now.
fn tool_body(
  ctx: &Ctx,
  shown: &ShownApproval,
  (server, tool): (&str, &str),
  (profiles, others): (&[Arc<str>], &[Arc<str>]),
) -> Vec<Element> {
  let join = |names: &[Arc<str>]| names.iter().map(|name| &**name).collect::<Vec<_>>().join(", ");
  // The asking agent's profile first.
  let mut ordered = profiles.to_vec();
  ordered.sort_by_key(|name| **name != *shown.approval.agent);
  let (getting, not_getting) = (join(&ordered), join(others));
  let names = [
    ("server", server),
    ("tool", tool),
    ("profiles", getting.as_str()),
    ("others", not_getting.as_str()),
    ("agent", shown.approval.agent.as_str()),
  ];
  let affects = if others.is_empty() {
    ctx.t_args("runs.toolApproval.always.toolProfilesAll", names)
  } else {
    ctx.t_args("runs.toolApproval.always.toolProfiles", names)
  };
  let what = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Tight.role())
    .child(Text::new(&ctx.t("runs.toolApproval.always.toolLabel")).variant(TypographyStyle::Caption))
    .child(target_row(
      "tool-approval-always-tool",
      Icon::Plus,
      (tool, true),
      &ctx.t_args("runs.toolApproval.always.toolWhere", names),
    ))
    .child(target_row(
      "tool-approval-always-profiles",
      Icon::Users,
      (&getting, false),
      &affects,
    ));
  vec![
    what.into(),
    note(
      Icon::Zap,
      &ctx.t("runs.toolApproval.always.toolStartsHeading"),
      &[ctx.t_args("runs.toolApproval.always.toolStarts", names)],
    ),
  ]
}

/// What is added or who it affects, as a row of the step (`uuHXG`, `wsQxg`).
fn target_row(id: &'static str, glyph: Icon, (value, mono): (&str, bool), detail: &str) -> Element {
  let value = if mono {
    Text::new(value).variant(ExtraTypography::MonoField.role())
  } else {
    Text::new(value).variant(ExtraTypography::NoticeHeading.role())
  };
  Row::new()
    .id(id)
    .width(Dimension::Pct(100.0))
    .min_height(Dimension::Px(ROW_HEIGHT))
    .padding_horizontal(ExtraSpacing::Inline.role())
    .spacing(ExtraSpacing::Inline.role())
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Sm)
    .border(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(icon(glyph, IconSize::Button, PaletteColor::TextSecondary))
    .child(value.nowrap())
    .child(Text::new(detail).variant(TypographyStyle::Caption).flex(1.0))
    .into()
}

/// A note on `$raised` (`e4AWI6`): its icon, heading and lines.
fn note(glyph: Icon, heading: &str, lines: &[Arc<str>]) -> Element {
  let copy = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Tight.role())
    .child(
      Text::new(heading)
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextPrimary),
    )
    .with_children(lines.iter().map(|line| {
      Text::new(line)
        .variant(ExtraTypography::WrappingProperty.role())
        .color(PaletteColor::TextSecondary)
    }));
  Row::new()
    .id("tool-approval-always-note")
    .width(Dimension::Pct(100.0))
    .padding(ExtraSpacing::Group.role())
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Start)
    .background(PaletteColor::SurfaceRaised)
    .rounded(ExtraRadius::Notice.role())
    .child(icon(glyph, IconSize::Notice, PaletteColor::TextSecondary))
    .child(copy)
    .into()
}
