//! The run's permissions on the task page: the *Permissions* row of "This run" (`C5Ai5o` in 02.10)
//! with the current agent's effective permissions and a *view ↗* link, which opens the run's
//! permissions popover above the row (02.J `q4K1G`, panel `NNRH8`).
//!
//! Changing permissions for a run needs the runner to take new permissions for the next agent
//! invocation (and, for *Apply now*, to restart one at a checkpoint). That is not built, so the
//! popover shows the values as they are, says why they cannot change here, and its *Apply now*,
//! *Apply from next invocation* and *Also save to the profile* are visibly unavailable.

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, Placement},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Popup, Rect, Row, Text},
  core::{ElementRef, Signal},
  layout::{Alignment, layout_kind::Justify, text_style::TextAlign},
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::permission::{
  AgentPermissions, CommandPermission, NetworkAccess, RepositoryAccess, capability::PermissionOption,
};

use super::heading::agent_of;
use crate::{
  agents::permissions::{Length, option_title, parts},
  screens::project::connections::status::short_key,
  tasks::detail::{RunPermissions, StepView},
  theme::{ExtraColor, ExtraRadius, ExtraShadow, ExtraSpacing, ExtraTypography, UNAVAILABLE_OPTION_OPACITY},
  ui::{
    button::ActionButton,
    command_chip::CommandChip,
    icon::{Icon, IconSize, icon},
    pressable::{pressable, surface},
    switch::Switch,
  },
};

/// Popover width and its distance above the row (`NNRH8`, `q4K1G`).
const POPOVER_WIDTH: f32 = 460.0;
const POPOVER_GAP: f32 = 6.0;
const ROW_HEIGHT: f32 = 20.0;
const DIVIDER_HEIGHT: f32 = 1.0;
/// Button height in the popover footer (`lTaB5`).
const FOOTER_BUTTON_HEIGHT: f32 = 30.0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunPermissionsProps {
  pub step: StepView,
  pub permissions: RunPermissions,
}

impl lurq::app::component::DevtoolsInspectable for RunPermissionsProps {}

pub struct RunPermissionsRow {
  open: Signal<bool>,
  anchor: ElementRef,
}

impl Component for RunPermissionsRow {
  type Props = RunPermissionsProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
      anchor: ElementRef::default(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<RunPermissionsProps>().clone();
    let value = value(ctx, &props.step, &props.permissions);
    let open = self.open.clone();
    let link = pressable(surface().id("task-permissions-view"), move || open.set(true))
      .rounded(RadiusSize::Sm)
      .child(Text::new(&ctx.t("runs.permissions.view")).variant(TypographyStyle::Link));
    // The value wraps in the narrow "This run" panel rather than losing its end; the link follows
    // it, right-aligned under the value.
    let value = Column::new()
      .flex(1.0)
      .spacing(SpacingSize::Xs)
      .align_items(Alignment::End)
      .child(
        Text::new(&value)
          .variant(TypographyStyle::Caption)
          .color(PaletteColor::TextPrimary)
          .text_align(TextAlign::Right),
      )
      .child(link);
    let row = Row::new()
      .id("task-permissions")
      .width(Dimension::Pct(100.0))
      .min_height(Dimension::Px(ROW_HEIGHT))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Start)
      .ref_element(self.anchor.clone())
      .child(
        Text::new(&ctx.t("runs.summary.permissions"))
          .variant(TypographyStyle::Caption)
          .nowrap()
          .flex_shrink(0.0),
      )
      .child(value);
    let popover = Popup::new(self.anchor.clone(), self.popover(ctx, &props))
      .open(self.open.clone())
      .placement(Placement::TopEnd)
      .offset(0.0, POPOVER_GAP);
    Column::new().width(Dimension::Pct(100.0)).child(row).child(popover)
  }
}

impl RunPermissionsRow {
  /// Permissions for this run (`NNRH8`), as they are; changing them is not available yet.
  fn popover(&self, ctx: &Ctx, props: &RunPermissionsProps) -> Element {
    let permissions = &props.permissions.permissions;
    let subtitle = ctx.t_args(
      "runs.permissions.popover.subtitle",
      [
        ("agent", agent_of(&props.step)),
        ("step", &*props.step.name),
        (
          "connection",
          &*props.step.provider.map_or_else(
            || ctx.t("pipelines.inspector.permissions.none"),
            |provider| ctx.t(short_key(provider)),
          ),
        ),
        ("model", props.step.model.as_deref().unwrap_or_default()),
        ("version", props.permissions.profile.to_string().as_str()),
      ],
    );
    let close = self.open.clone();
    let header = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Start)
      .child(
        Column::new()
          .flex(1.0)
          .spacing(SpacingSize::Xs)
          .child(Text::new(&ctx.t("runs.permissions.popover.title")).variant(TypographyStyle::Label))
          .child(Text::new(&subtitle).variant(TypographyStyle::Caption)),
      )
      .child(
        pressable(surface().id("task-permissions-close"), move || close.set(false))
          .rounded(RadiusSize::Sm)
          .child(icon(Icon::X, IconSize::Dismiss, PaletteColor::TextSecondary)),
      );
    let notice = Row::new()
      .width(Dimension::Pct(100.0))
      .padding(ExtraSpacing::Stack.role())
      .spacing(ExtraSpacing::Inline.role())
      .background(ExtraColor::Surface.role())
      .rounded(ExtraRadius::Notice.role())
      .child(icon(Icon::Lock, IconSize::Event, PaletteColor::TextSecondary))
      .child(
        Text::new(&ctx.t("runs.permissions.popover.unavailable"))
          .variant(ExtraTypography::Detail.role())
          .flex(1.0),
      );
    Column::new()
      .id("task-permissions-popover")
      .width(Dimension::Px(POPOVER_WIDTH))
      .padding(SpacingSize::Md)
      .spacing(ExtraSpacing::Group.role())
      .background(PaletteColor::SurfaceRaised)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Lg)
      .box_shadow(ExtraShadow::Popover.role())
      .child(header)
      .child(notice)
      .child(repository_group(ctx, permissions))
      .child(commands_group(ctx, permissions))
      .child(network_group(ctx, props))
      .child(Rect::new(Dimension::Pct(100.0), Dimension::Px(DIVIDER_HEIGHT)).background(ExtraColor::Hairline.role()))
      .child(save_to_profile(ctx, props))
      .child(self.footer(ctx))
      .into()
  }

  /// Cancel closes; the two ways to apply are unavailable (`dIP0h`).
  fn footer(&self, ctx: &Ctx) -> Element {
    let close = self.open.clone();
    let height = Dimension::Px(FOOTER_BUTTON_HEIGHT);
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .justify(Justify::End)
      .align_items(Alignment::Center)
      .child(
        ActionButton::new(ctx.t("common.cancel"))
          .id("task-permissions-cancel")
          .compact()
          .in_form()
          .height(height)
          .on_click(move || close.set(false))
          .render(ctx),
      )
      .child(
        ActionButton::new(ctx.t("runs.permissions.popover.applyNow"))
          .id("task-permissions-apply-now")
          .icon(Icon::RefreshCw)
          .compact()
          .height(height)
          .render(ctx),
      )
      .child(
        ActionButton::new(ctx.t("runs.permissions.popover.applyNext"))
          .id("task-permissions-apply-next")
          .icon(Icon::Check)
          .primary()
          .compact()
          .height(height)
          .render(ctx),
      )
      .into()
  }
}

/// The step's agent and its permissions in compact words: "Builder · Edit files · 3 commands ·
/// Network off" (`YOcfT`); `None` for a step without them (a tool step).
pub fn permissions_value(ctx: &Ctx, step: &StepView) -> Option<Arc<str>> {
  step
    .permissions
    .as_ref()
    .map(|permissions| value(ctx, step, permissions))
}

fn value(ctx: &Ctx, step: &StepView, permissions: &RunPermissions) -> Arc<str> {
  let (repository, commands, network) = parts(
    ctx,
    &permissions.permissions,
    permissions.network_stays_on,
    Length::Compact,
  );
  ctx.t_args(
    "runs.permissions.value",
    [
      ("agent", agent_of(step)),
      ("repository", &*repository),
      ("commands", &*commands),
      ("network", &*network),
    ],
  )
}

/// A setting's label over its options (`G2l6MT`).
fn group(ctx: &Ctx, label: &str, content: Vec<Element>) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(Text::new(&ctx.t(label)).variant(TypographyStyle::Caption))
    .with_children(content)
    .into()
}

/// A compact radio option (`ihdDb`); the run's value is marked, nothing can be chosen here.
fn radio(ctx: &Ctx, option: PermissionOption, selected: bool) -> Element {
  let (glyph, color) = if selected {
    (Icon::CircleDot, PaletteColor::TextPrimary)
  } else {
    (Icon::Circle, PaletteColor::TextSecondary)
  };
  Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .describe("selected", if selected { "true" } else { "false" })
    .child(icon(glyph, IconSize::Event, color.clone()))
    .child(
      Text::new(&ctx.t(option_title(option)))
        .variant(TypographyStyle::FieldLabel)
        .color(color),
    )
    .into()
}

fn options(options: Vec<Element>) -> Element {
  Row::new()
    .spacing(ExtraSpacing::Step.role())
    .align_items(Alignment::Center)
    .with_children(options)
    .into()
}

fn repository_group(ctx: &Ctx, permissions: &AgentPermissions) -> Element {
  let read_only = permissions.repository() == RepositoryAccess::ReadOnly;
  group(
    ctx,
    "agents.permissions.repository",
    vec![options(vec![
      radio(ctx, PermissionOption::ReadOnly, read_only),
      radio(ctx, PermissionOption::EditWorktree, !read_only),
    ])],
  )
}

fn commands_group(ctx: &Ctx, permissions: &AgentPermissions) -> Element {
  let commands = permissions.commands();
  let mut content = vec![options(vec![
    radio(
      ctx,
      PermissionOption::AllowedCommands,
      matches!(commands, CommandPermission::Allowed(_)),
    ),
    radio(
      ctx,
      PermissionOption::AnyCommand,
      *commands == CommandPermission::AnyInWorktree,
    ),
  ])];
  if let CommandPermission::Allowed(list) = commands {
    content.push(
      Row::new()
        .wrap()
        .spacing(ExtraSpacing::Tight.role())
        .with_children(list.entries().iter().enumerate().map(|(index, entry)| {
          Element::from(CommandChip::new(
            format!("task-permissions-command-{index}"),
            entry.as_str().into(),
          ))
        }))
        .into(),
    );
  }
  group(ctx, "agents.permissions.commands", content)
}

/// Network (`q4Nelz`): the run's value on a switch that cannot be flipped here.
fn network_group(ctx: &Ctx, props: &RunPermissionsProps) -> Element {
  let network = props.permissions.permissions.network();
  let label = match (network, props.permissions.network_stays_on) {
    (NetworkAccess::Off, false) => ctx.t("agents.permissions.networkOff"),
    (NetworkAccess::Off, true) => ctx.t("agents.permissions.compact.networkStaysOn"),
    (NetworkAccess::On, _) => ctx.t("agents.permissions.networkOn"),
  };
  let on = network == NetworkAccess::On || props.permissions.network_stays_on;
  group(
    ctx,
    "agents.permissions.network",
    vec![Switch::new("task-permissions-network", label, on).locked()],
  )
}

/// *Also save to the Builder profile* (`u8JAYJ`), unavailable like applying.
fn save_to_profile(ctx: &Ctx, props: &RunPermissionsProps) -> Element {
  let agent: Arc<str> = agent_of(&props.step).into();
  Row::new()
    .id("task-permissions-save-to-profile")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Start)
    .opacity(UNAVAILABLE_OPTION_OPACITY)
    .describe("state", "unavailable")
    .child(icon(Icon::Square, IconSize::Event, PaletteColor::TextSecondary))
    .child(
      Column::new()
        .flex(1.0)
        .spacing(ExtraSpacing::Mark.role())
        .child(
          Text::new(&ctx.t_args("runs.permissions.popover.saveToProfile", [("agent", &*agent)]))
            .variant(TypographyStyle::FieldLabel)
            .color(PaletteColor::TextPrimary),
        )
        .child(Text::new(&ctx.t("runs.permissions.popover.saveToProfileDetail")).variant(ExtraTypography::Hint.role())),
    )
    .into()
}
