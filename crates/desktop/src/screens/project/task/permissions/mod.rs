//! The run's permissions on the task page: the row of "This run" (`H1wCPK` in 02.10) and of the
//! working context (`YOcfT`, `Lr8Dg`, `d6xleF`) with what the step's agent runs with and a
//! *change ↗* link, which opens the run's permissions popover above the row (02.J `q4K1G`, panel
//! `NNRH8`, see `popover`).
//!
//! The row shows what the step's current or last invocation launched with, as recorded, never the
//! run's settings for the next one; its label says which (*Running with*, *Last ran with*, *Next
//! run uses*, see `policy`). A change applies to this run only: from the next invocation, or now,
//! restarting the agent at its next checkpoint; under the row its notices say where it stands
//! (`notice`). A change also saved to the profile names the draft it went to, with a link to the
//! profile.

mod notice;
mod policy;
mod popover;
mod settings;

use std::path::PathBuf;

use lurq::{
  app::{
    component::Component,
    ctx::{CollisionStrategy, Ctx, FutureAction, Placement},
    theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Popup, Row, ScrollVertical, Spacer, Text},
  core::{ElementRef, Signal},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::{ResourceId, RunId},
  permission::AgentPermissions,
  run::{Invocation, PermissionsApply},
};
use orchester_ipc::Command;

pub use self::policy::permissions_property;
use self::{
  policy::{PolicyText, ROW_HEIGHT, Wording, label_text, policy_property, policy_text, value_text},
  settings::{Draft, Edit},
};
use super::heading::agent_of;
use crate::{
  agents::data::save_permissions_to_draft,
  background,
  messages::Message,
  navigation::route::{AgentSelection, Page},
  runner::Runners,
  screens::project::ProjectNav,
  tasks::{
    detail::{RunPermissions, StepView},
    permissions::InvocationPolicy,
    run::control_message,
  },
  theme::{STATUS_BAR_HEIGHT, TITLE_BAR_HEIGHT},
  ui::pressable::{pressable, surface},
};

/// Popover distance above the row (`q4K1G`): its bottom stays this far above the row.
const POPOVER_GAP: f32 = 6.0;
/// Height of *Keep for next invocation instead* (`RL7CZ`).
const KEEP_BUTTON_HEIGHT: f32 = 30.0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunPermissionsProps {
  pub step: StepView,
  pub permissions: RunPermissions,
  pub run: RunId,
  pub run_number: u32,
  /// The run has not ended, so its permissions can change.
  pub live: bool,
  /// The project folder, where the profile's draft is saved.
  pub repo_root: PathBuf,
  /// The link follows the value on its line, as in the Conversation tab's wide column (`Lr8Dg` in
  /// 02.S `JOG1i`), instead of under it.
  pub inline_link: bool,
}

impl lurq::app::component::DevtoolsInspectable for RunPermissionsProps {}

/// A change for the run, and whether to save it to the profile's draft too.
struct ApplyRequest {
  run: RunId,
  profile: ResourceId,
  permissions: AgentPermissions,
  apply: PermissionsApply,
  save_to: Option<PathBuf>,
}

/// The change was applied; with the number of the profile draft it was saved to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Applied(Option<u32>);

impl lurq::app::component::DevtoolsInspectable for Applied {}

type ApplyAction = FutureAction<ApplyRequest, Applied, Message>;
type KeepAction = FutureAction<(RunId, ResourceId), (), Message>;

pub struct RunPermissionsRow {
  open: Signal<bool>,
  anchor: ElementRef,
  edit: Edit,
  /// The profile draft the last change was also saved to.
  saved: Signal<Option<u32>>,
  nav: ProjectNav,
}

impl Component for RunPermissionsRow {
  type Props = RunPermissionsProps;

  fn create(ctx: &mut Ctx) -> Self {
    let apply = apply_action(ctx);
    keep_action(ctx);
    let row = Self {
      open: ctx.signal(false),
      anchor: ElementRef::default(),
      edit: Edit {
        draft: ctx.signal(Draft::default()),
        save: ctx.signal(false),
        new_command: ctx.signal(String::new()),
        command_error: ctx.signal(None),
        adding: ctx.signal(false),
        command_input: ElementRef::new(),
      },
      saved: ctx.signal(None),
      nav: ctx
        .use_context::<ProjectNav>()
        .expect("the project shell provides ProjectNav"),
    };
    let (open, saved) = (row.open.clone(), row.saved.clone());
    ctx.watch(&apply.state(), move |state| {
      if let Some(Applied(draft)) = state.data.filter(|_| !state.is_pending()) {
        saved.set(draft);
        open.set(false);
      }
    });
    row
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<RunPermissionsProps>().clone();
    let apply = apply_action(ctx);
    let keep = keep_action(ctx);
    // Popup collision bounds cover the whole window, so the content reserves the title and status
    // bars; it scrolls when it is taller than the room above the row, so its bottom always stays
    // `POPOVER_GAP` above the row instead of being pushed over it.
    let window = ctx.window().info().logical_height() - TITLE_BAR_HEIGHT - STATUS_BAR_HEIGHT;
    let above = self.anchor.bounds().y - TITLE_BAR_HEIGHT - POPOVER_GAP;
    let available = if above > 0.0 { above.min(window) } else { window }.max(0.0);
    let content = Column::new()
      .child(Spacer::new().height(Dimension::Px(TITLE_BAR_HEIGHT)))
      .child(
        ScrollVertical::new(self.popover(ctx, &props, &apply))
          .id("task-permissions-scroll")
          .max_height(Dimension::Px(available)),
      )
      .child(Spacer::new().height(Dimension::Px(STATUS_BAR_HEIGHT)));
    let popover = Popup::new(self.anchor.clone(), content)
      .open(self.open.clone())
      .placement(Placement::TopEnd)
      .collision(CollisionStrategy::Clamp)
      .offset(0.0, POPOVER_GAP - STATUS_BAR_HEIGHT);
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(self.row(ctx, &props))
      .with_children(notice::state_notice(ctx, &props, &keep))
      .with_children(self.saved_line(ctx, &props))
      .child(popover)
  }
}

impl RunPermissionsRow {
  /// "Running with  Builder · Edit files · 3 commands · Network off · MCP asks · change ↗". A full
  /// value in the "This run" panel stacks above its link (`H1wCPK`); elsewhere the link follows the
  /// value on its line and moves under it when the line cannot hold both (`iPHGE`). The link is
  /// never clipped.
  fn row(&self, ctx: &Ctx, props: &RunPermissionsProps) -> Element {
    let text = policy_text(
      ctx,
      &props.step,
      (&props.permissions.invocation_policy, &props.permissions),
      Wording::Row,
    );
    let (open, edit, base) = (self.open.clone(), self.edit.clone(), props.permissions.base.clone());
    let link_key = if props.live {
      "runs.permissions.change"
    } else {
      "runs.permissions.view"
    };
    let stacked = text.full && !props.inline_link;
    let link_text = Text::new(&ctx.t(link_key)).variant(TypographyStyle::Caption);
    let link = pressable(surface().id("task-permissions-view"), move || {
      edit.open(&base);
      open.set(true);
    })
    .rounded(RadiusSize::Sm)
    .child(if stacked {
      link_text
    } else {
      link_text.color(PaletteColor::TextPrimary)
    });
    let value = if stacked {
      Column::new()
        .flex(1.0)
        .spacing(SpacingSize::Xs)
        .align_items(Alignment::End)
        .child(value_text(&text.value).id("task-permissions-value"))
        .child(link)
        .into()
    } else {
      inline_value(ctx, &text, link.into())
    };
    Row::new()
      .id("task-permissions")
      .width(Dimension::Pct(100.0))
      .min_height(Dimension::Px(ROW_HEIGHT))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Start)
      .ref_element(self.anchor.clone())
      .child(label_text(&text.label))
      .child(value)
      .into()
  }

  /// "Also saved to the Builder profile draft v3 · open ↗".
  fn saved_line(&self, ctx: &Ctx, props: &RunPermissionsProps) -> Option<Element> {
    let draft = self.saved.get()?;
    let profile = props.permissions.profile_id;
    let text = ctx.t_args(
      "runs.permissions.savedToProfile",
      [
        ("agent", agent_of(&props.step)),
        ("version", draft.to_string().as_str()),
      ],
    );
    let link = self
      .nav
      .link(Page::Agents(AgentSelection::Profile(profile)))
      .on(surface().id("task-permissions-open-profile"))
      .rounded(RadiusSize::Sm)
      .child(Text::new(&ctx.t("runs.permissions.openProfile")).variant(TypographyStyle::Link));
    Some(
      Row::new()
        .id("task-permissions-saved")
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Start)
        .child(Text::new(&text).variant(TypographyStyle::Caption).flex(1.0))
        .child(link)
        .into(),
    )
  }
}

/// The value and "· change ↗" on one line, right-aligned; when they do not fit, the link and its
/// separator wrap under the value, which itself wraps at its separators.
fn inline_value(ctx: &Ctx, text: &PolicyText, link: Element) -> Element {
  Row::new()
    .flex(1.0)
    .wrap()
    .spacing(SpacingSize::Xs)
    .justify(Justify::End)
    .align_items(Alignment::End)
    .child(value_text(&text.value).id("task-permissions-value"))
    .child(
      Row::new()
        .spacing(SpacingSize::Xs)
        .align_items(Alignment::Center)
        .child(
          Text::new(&ctx.t("common.separator"))
            .variant(TypographyStyle::Caption)
            .color(PaletteColor::TextPrimary),
        )
        .child(link),
    )
    .into()
}

/// What `invocation`, a launch of `step`, ran with, for the Activity panel about it (`iPHGE`):
/// *Running with* while it is open, *Ran with* once it ended; `None` for a step without permissions
/// (a tool step).
pub fn invocation_property(ctx: &Ctx, step: &StepView, invocation: &Invocation) -> Option<Element> {
  let permissions = step.permissions.as_ref()?;
  let text = policy_text(
    ctx,
    step,
    (&InvocationPolicy::of(invocation), permissions),
    Wording::Activity,
  );
  Some(policy_property(&text).id("activity-permissions").into())
}

fn apply_action(ctx: &mut Ctx) -> ApplyAction {
  let runners = ctx
    .use_context::<Runners>()
    .expect("the root provides Runners before mounting screens");
  ctx.future_action(move |request: ApplyRequest| {
    let runners = runners.clone();
    async move {
      let command = Command::ChangeRunPermissions {
        run: request.run,
        profile: request.profile,
        permissions: request.permissions.clone(),
        apply: request.apply,
      };
      runners.control_run(command).await.map_err(|error| {
        tracing::warn!(%error, "the runner did not change the run's permissions");
        control_message(&error)
      })?;
      let Some(repo_root) = request.save_to else {
        return Ok(Applied(None));
      };
      let (profile, permissions) = (request.profile, request.permissions);
      let saved = background::run(move || save_permissions_to_draft(&repo_root, profile, permissions)).await?;
      saved.map(|draft| Applied(Some(draft))).map_err(|error| {
        tracing::error!(%error, "cannot save the permissions to the profile draft");
        Message::new("runs.permissions.errors.notSavedToProfile")
      })
    }
  })
}

fn keep_action(ctx: &mut Ctx) -> KeepAction {
  let runners = ctx
    .use_context::<Runners>()
    .expect("the root provides Runners before mounting screens");
  ctx.future_action(move |(run, profile): (RunId, ResourceId)| {
    let runners = runners.clone();
    async move {
      let command = Command::KeepPermissionsForNextInvocation { run, profile };
      runners.control_run(command).await.map_err(|error| {
        tracing::warn!(%error, "the runner did not keep the permissions for the next invocation");
        control_message(&error)
      })
    }
  })
}
