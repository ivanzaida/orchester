//! The run's permissions on the task page: the *Permissions* row of "This run" (`C5Ai5o` in 02.10)
//! with the current agent's effective permissions and a *change ↗* link, which opens the run's
//! permissions popover above the row (02.J `q4K1G`, panel `NNRH8`, see `popover`).
//!
//! A change applies to this run only: from the next invocation, or now, restarting the agent at
//! its next checkpoint. Under the row, a change applied now reads as pending ("Applying new
//! permissions at the next checkpoint…" with *Keep for next invocation instead*) and then as
//! applied ("New permissions active since 12:09", `LJmTH`). A change also saved to the profile
//! names the draft it went to, with a link to the profile.

mod popover;
mod settings;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction, Placement},
    theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Popup, Row, Text},
  core::{ElementRef, Signal},
  layout::{Alignment, layout_kind::Justify, text_style::TextAlign},
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::{ResourceId, RunId},
  permission::AgentPermissions,
  run::PermissionsApply,
};
use orchester_ipc::Command;

use self::settings::{Draft, Edit};
use super::heading::agent_of;
use crate::{
  agents::{
    data::save_permissions_to_draft,
    permissions::{Length, parts},
  },
  background, display,
  messages::Message,
  navigation::route::{AgentSelection, Page},
  runner::Runners,
  screens::project::ProjectNav,
  tasks::{
    detail::{ChangeState, RunPermissions, StepView},
    run::control_message,
  },
  ui::{
    button::ActionButton,
    field::field_error,
    icon::Icon,
    message_text,
    notice::Notice,
    pressable::{pressable, surface},
  },
};

/// Popover distance above the row (`q4K1G`), and the row's height (`C5Ai5o`).
const POPOVER_GAP: f32 = 6.0;
const ROW_HEIGHT: f32 = 20.0;
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
    let popover = Popup::new(self.anchor.clone(), self.popover(ctx, &props, &apply))
      .open(self.open.clone())
      .placement(Placement::TopEnd)
      .offset(0.0, POPOVER_GAP);
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(self.row(ctx, &props))
      .with_children(state_notice(ctx, &props, &keep))
      .with_children(self.saved_line(ctx, &props))
      .child(popover)
  }
}

impl RunPermissionsRow {
  /// "Permissions  Builder · Edit files · 3 commands · Network off", with *change ↗* under it.
  fn row(&self, ctx: &Ctx, props: &RunPermissionsProps) -> Element {
    let pending = props.permissions.state == Some(ChangeState::Pending);
    let value = if pending {
      ctx.t_args("runs.permissions.applyingValue", [("agent", agent_of(&props.step))])
    } else {
      value(ctx, &props.step, &props.permissions)
    };
    let (open, edit, base) = (self.open.clone(), self.edit.clone(), props.permissions.base.clone());
    let link_key = if props.live {
      "runs.permissions.change"
    } else {
      "runs.permissions.view"
    };
    let link = pressable(surface().id("task-permissions-view"), move || {
      edit.open(&base);
      open.set(true);
    })
    .rounded(RadiusSize::Sm)
    .child(Text::new(&ctx.t(link_key)).variant(TypographyStyle::Link));
    let text = Text::new(&value)
      .variant(TypographyStyle::Caption)
      .color(PaletteColor::TextPrimary);
    let value: Element = if props.inline_link {
      Row::new()
        .flex(1.0)
        .spacing(SpacingSize::Xs)
        .align_items(Alignment::Center)
        .justify(Justify::End)
        .child(text.nowrap())
        .child(
          Text::new(&ctx.t("common.separator"))
            .variant(TypographyStyle::Caption)
            .color(PaletteColor::TextPrimary),
        )
        .child(link)
        .into()
    } else {
      // The value wraps in the narrow "This run" panel rather than losing its end; the link
      // follows it, right-aligned under the value.
      Column::new()
        .flex(1.0)
        .spacing(SpacingSize::Xs)
        .align_items(Alignment::End)
        .child(text.text_align(TextAlign::Right))
        .child(link)
        .into()
    };
    Row::new()
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
      .child(value)
      .into()
  }

  /// "Also saved to the Builder profile draft v3 · open ↗".
  fn saved_line(&self, ctx: &Ctx, props: &RunPermissionsProps) -> Option<Element> {
    let draft = self.saved.get()?;
    let nav = self.nav.clone();
    let profile = props.permissions.profile_id;
    let text = ctx.t_args(
      "runs.permissions.savedToProfile",
      [
        ("agent", agent_of(&props.step)),
        ("version", draft.to_string().as_str()),
      ],
    );
    let link = pressable(surface().id("task-permissions-open-profile"), move || {
      nav.show(Page::Agents(AgentSelection::Profile(profile)));
    })
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

/// Pending (loader, what happens to work in flight, *Keep for next invocation instead*) or
/// applied (check, since when, the checkpoint and the invocation) (`LJmTH`).
fn state_notice(ctx: &Ctx, props: &RunPermissionsProps, keep: &KeepAction) -> Vec<Element> {
  let agent = agent_of(&props.step);
  let step = &*props.step.name;
  match &props.permissions.state {
    Some(ChangeState::Pending) => {
      let state = keep.state().get();
      let pending = state.is_pending();
      let (keep, run, profile) = (keep.clone(), props.run, props.permissions.profile_id);
      let button = ActionButton::new(ctx.t("runs.permissions.keepForNext"))
        .id("task-permissions-keep")
        .icon(Icon::Undo2)
        .compact()
        .height(Dimension::Px(KEEP_BUTTON_HEIGHT));
      let button = if pending {
        button.disabled()
      } else {
        button.on_click(move || keep.run((run, profile)))
      };
      let mut elements: Vec<Element> = vec![
        Notice::new(
          Icon::Loader,
          ctx.t("runs.permissions.pendingTitle"),
          ctx.t_args("runs.permissions.pendingDetail", [("agent", agent), ("step", step)]),
        )
        .id("task-permissions-pending")
        .into(),
      ];
      elements.extend(
        state
          .error
          .filter(|_| !pending)
          .map(|error| field_error(&message_text(ctx, &error))),
      );
      elements.push(
        Row::new()
          .width(Dimension::Pct(100.0))
          .justify(lurq::layout::layout_kind::Justify::End)
          .child(button.render(ctx))
          .into(),
      );
      elements
    }
    Some(ChangeState::Applied {
      at,
      checkpoint,
      invocation,
    }) => {
      let invocation = invocation.to_string();
      let detail = match checkpoint {
        Some(revision) => ctx.t_args(
          "runs.permissions.appliedDetail",
          [
            ("agent", agent),
            ("step", step),
            ("checkpoint", display::short_revision(revision.as_str())),
            ("invocation", invocation.as_str()),
          ],
        ),
        None => ctx.t_args(
          "runs.permissions.appliedDetailNoCheckpoint",
          [("agent", agent), ("step", step), ("invocation", invocation.as_str())],
        ),
      };
      vec![
        Notice::new(
          Icon::Check,
          ctx.t_args(
            "runs.permissions.appliedTitle",
            [("time", display::clock(*at).as_str())],
          ),
          detail,
        )
        .id("task-permissions-applied")
        .into(),
      ]
    }
    None => Vec::new(),
  }
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
