//! The form that creates a resource in the panel (`Agents · New agent profile` … `New tool`, A7–A12 in `acFkz`): the
//! kind and *not created yet · Create publishes v1* over the new resource's title, *For the team* (what the team holds,
//! the same on every computer) as rows, a note on what each computer fills in afterwards, and pinned at the bottom
//! Create and Cancel. No draft: Create makes v1 and sends it to the team server; it shows as created only once the
//! server has taken it ([`super::work`]), a refusal is said in place and nothing is made, and Cancel leaves nothing.

use std::path::{Path, PathBuf};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, ScrollVertical, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::id::{ProjectId, ResourceId};
use orchester_workspace::{
  agents::data::ProjectAgents,
  background,
  messages::Message,
  sharing::{team::TeamConfiguration, team_edit::Refusal},
};

use super::{
  fields::{Body, Shown},
  instruction, kinds, mcp, notices, profile, skill, tool,
  work::{self, Failed, Press, Work},
};
use crate::{
  mcp::input_gate::{Guarded, InputGate, input_may_decide},
  navigation::Navigation,
  routes::{HubView, ProjectPage, Route},
  screens::team_hub::{
    bar,
    model::{Kind, Listing},
    panel as shared, rows,
  },
  services::Services,
  theme::{HubSpacing, HubText, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::{Icon, icon},
    message::message_text,
  },
};

#[derive(Clone, PartialEq)]
pub(in crate::screens::team_hub) struct Props {
  pub repo_root: PathBuf,
  pub project: ProjectId,
  pub kind: Kind,
  pub listing: Listing,
  /// The team's configuration: who the admins are, which a refusal for a role that changed says.
  pub team: TeamConfiguration,
}
impl DevtoolsInspectable for Props {}

pub(in crate::screens::team_hub) fn panel(ctx: &mut Ctx, props: Props) -> Element {
  ctx.mount_keyed::<NewPanel>(&format!("team-hub-new-{}-{}", props.kind.name(), props.project), props)
}

/// The fields of the kind's form: they live as long as the form is open, and a form of another kind is another mount.
enum Form {
  Profile(profile::Fields),
  McpServer(mcp::Fields),
  Instruction(instruction::Fields),
  Skill(skill::Fields),
  Tool(tool::Fields),
}

impl Form {
  fn new(ctx: &mut Ctx, kind: Kind) -> Self {
    match kind {
      Kind::Profile => Self::Profile(profile::Fields::new(ctx)),
      Kind::McpServer => Self::McpServer(mcp::Fields::new(ctx)),
      Kind::Instruction => Self::Instruction(instruction::Fields::new(ctx)),
      Kind::Skill => Self::Skill(skill::Fields::new(ctx)),
      Kind::Tool => Self::Tool(tool::Fields::new(ctx)),
    }
  }

  fn body(&self, ctx: &mut Ctx, shown: &Shown<'_>) -> Body {
    match self {
      Self::Profile(fields) => fields.body(ctx, shown),
      Self::McpServer(fields) => fields.body(ctx, shown),
      Self::Instruction(fields) => fields.body(ctx, shown),
      Self::Skill(fields) => fields.body(ctx, shown),
      Self::Tool(fields) => fields.body(ctx, shown),
    }
  }
}

struct NewPanel {
  form: Form,
  /// Create was pressed: a required field that is empty says so.
  tried: Signal<bool>,
  work: Work,
  /// *Cancel sending*: takes back the new resource that waits in the outbox, and the form goes with it.
  cancel: FutureAction<ResourceId, (), Message>,
  gate: Option<InputGate>,
  /// The name field, which has the focus when the form opens (the design draws it focused).
  first: ElementRef,
  /// The form has not given the focus to its name field yet.
  focus_first: Signal<bool>,
}

impl Component for NewPanel {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<Props>().clone();
    let work = work::work(ctx, (props.repo_root.clone(), props.project));
    let navigation = Navigation::of(ctx);
    let (project, filter) = (props.project, props.kind.filter());
    // Created, and taken by the team server: the hub shows it, and its form is gone with the route.
    ctx.watch(&work.state(), move |state| {
      let Some(made) = state.data.as_ref().filter(|_| state.is_fulfilled()) else {
        return;
      };
      if !made.waiting {
        let shown = Route::Project(project, ProjectPage::hub(HubView::new(filter, Some(made.id))));
        navigation.replace(shown);
      }
    });
    let cancel = {
      let (servers, root) = (Services::of(ctx).servers, props.repo_root.clone());
      ctx.future_action(move |id: ResourceId| {
        let (servers, root) = (servers.clone(), root.clone());
        async move { servers.cancel_created(&root, id).await }
      })
    };
    let back = Navigation::of(ctx);
    ctx.watch(&cancel.state(), move |state| {
      if state.is_fulfilled() {
        back.replace(Route::Project(project, ProjectPage::hub(HubView::new(filter, None))));
      }
    });
    Self {
      form: Form::new(ctx, props.kind),
      tried: ctx.signal(false),
      work,
      cancel,
      gate: ctx.use_context::<InputGate>(),
      first: ElementRef::default(),
      focus_first: ctx.signal(true),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Props {
      repo_root,
      project,
      kind,
      listing,
      team,
    } = ctx.props::<Props>().clone();
    if self.focus_first.get() {
      ctx.focus(&self.first);
      self.focus_first.set(false);
    }
    let state = self.work.state().get();
    let pending = state.is_pending();
    let waiting = state
      .data
      .as_ref()
      .filter(|made| made.waiting && state.is_fulfilled())
      .map(|made| made.id);
    let refused = state.error.filter(|_| !pending);
    let refused_name = taken_by_server(refused.as_ref());
    let agents = self.agents(ctx, kind, &repo_root);
    let shown = Shown {
      kind,
      listing: &listing,
      tried: self.tried.get(),
      locked: pending || waiting.is_some(),
      agents: agents.as_ref(),
      first: &self.first,
      refused_name: refused_name.as_deref(),
      published: None,
    };
    let body = self.form.body(ctx, &shown);
    let for_team = for_team(ctx, body.rows);
    let navigation = Navigation::of(ctx);
    let content = Column::new()
      .id("team-new-form")
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(
        HubSpacing::Sections.role(),
        HubSpacing::BodyTop.role(),
      ))
      .spacing(HubSpacing::BodyTop.role())
      .child(head(ctx, kind))
      .with_children(self.refusal(ctx, refused.as_ref(), waiting, &team))
      .with_children(self.cancel_failure(ctx))
      .child(for_team)
      .child(computer(ctx, body.computer));
    let scroll = ScrollVertical::new(content)
      .id("team-hub-panel-scroll")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .min_height(Dimension::Px(0.0));
    let bar = self.bar(
      ctx,
      &body.make,
      (body.off, pending || waiting.is_some()),
      (project, kind),
      &navigation,
    );
    Column::new()
      .id("team-hub-panel")
      .describe("state", "new")
      .describe("kind", kind.name())
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .height(Dimension::Pct(100.0))
      .child(scroll)
      .child(bar)
  }
}

impl NewPanel {
  /// Why nothing was created, or that it waits for the team server: in place, under the note.
  fn refusal(
    &self,
    ctx: &Ctx,
    refused: Option<&Failed>,
    waiting: Option<ResourceId>,
    team: &TeamConfiguration,
  ) -> Option<Element> {
    if let Some(id) = waiting {
      let cancel = ActionButton::new(
        "team-new-cancel-sending",
        ButtonKind::Secondary,
        ctx.t("team_new.waiting.cancel"),
      )
      .icon(Icon::X);
      let action = self.cancel.clone();
      let cancel = if action.state().get().is_pending() {
        cancel.loading()
      } else {
        cancel.on_click(move || action.run(id))
      };
      return Some(notices::waiting(ctx, cancel));
    }
    refused.map(|failed| notices::refused(ctx, failed, team))
  }

  /// Why *Cancel sending* did nothing: it is being sent right now, the server took the resource meanwhile, or the
  /// resource holds more than its first version.
  fn cancel_failure(&self, ctx: &Ctx) -> Option<Element> {
    let state = self.cancel.state().get();
    let why = state.error.as_ref().filter(|_| state.is_rejected())?;
    Some(field_error("team-new-cancel-error", &message_text(ctx, why)).into())
  }

  /// This computer's connections, which the profile form's CLI and model choices come from.
  fn agents(&self, ctx: &mut Ctx, kind: Kind, root: &Path) -> Option<ProjectAgents> {
    if kind != Kind::Profile {
      return None;
    }
    ctx
      .future(root.to_path_buf(), move |root| async move {
        background::run(move || {
          ProjectAgents::load_in(&root).map_err(|error| orchester_workspace::messages::store_error(&error))
        })
        .await?
      })
      .state()
      .get()
      .data
  }

  /// Create and Cancel, with what Create does under them at the bar's end.
  fn bar(
    &self,
    ctx: &Ctx,
    make: &Option<work::Make>,
    (off, busy): (Option<&'static str>, bool),
    (project, kind): (ProjectId, Kind),
    navigation: &Navigation,
  ) -> Element {
    let create = ActionButton::new("team-hub-new-create", ButtonKind::Primary, ctx.t("team_new.bar.create"))
      .icon(Icon::Plus)
      .pinned();
    let create = if busy && self.work.state().get().is_pending() {
      create.loading()
    } else if busy || off.is_some() {
      create.disabled()
    } else {
      let (work, tried, gate, make) = (self.work.clone(), self.tried.clone(), self.gate.clone(), make.clone());
      create.on_click(move || {
        tried.set(true);
        if let Some(make) = make.clone() {
          // A person's press, or automation's where they let it change what agents may do.
          let decided = input_may_decide(gate.as_ref(), Guarded::PermissionWidening);
          work.run(Press { make, decided });
        }
      })
    };
    let cancel = ActionButton::new(
      "team-hub-new-cancel",
      ButtonKind::Secondary,
      ctx.t("team_new.bar.cancel"),
    )
    .pinned();
    let cancel = if self.work.state().get().is_pending() {
      cancel.disabled()
    } else {
      cancel.opens(navigation.to(Route::Project(
        project,
        ProjectPage::hub(HubView::new(kind.filter(), None)),
      )))
    };
    let hint = match off {
      Some(why) => (ctx.t(why).to_string(), PaletteColor::Danger),
      None => (ctx.t("team_new.bar.hint").to_string(), PaletteColor::TextMuted),
    };
    bar::bar_saying(vec![create.into(), cancel.into()], Some(hint))
  }
}

/// A name the team server said is taken, which the list this computer holds did not have.
fn taken_by_server(refused: Option<&Failed>) -> Option<String> {
  match refused {
    Some(Failed::Refused(Refusal::NameTaken, name)) => Some(name.clone()),
    _ => None,
  }
}

/// *For the team*: what the team holds, the same on every computer, as the rows of the kind's form.
fn for_team(ctx: &Ctx, rows: Vec<Row>) -> Column {
  rows::section(
    "team-new-team",
    &ctx.t("team_new.forTeam"),
    vec![
      Text::new(&ctx.t("team_new.goesOnCreate"))
        .id("team-new-team-hint")
        .variant(HubText::Hint.role())
        .nowrap()
        .into(),
    ],
    rows,
  )
}

/// The head: what it is and that it is not created yet over its title (`Head` in A7).
fn head(ctx: &Ctx, kind: Kind) -> Row {
  shared::head_named(
    &ctx.t(kinds::title_key(kind)),
    (
      ctx.t(kind.where_key()).to_string(),
      Some(ctx.t("team_new.notCreatedYet").to_string()),
    ),
    None,
  )
}

/// What each computer fills in afterwards, apart from the team's rows (`Computer` note in A7).
fn computer(ctx: &Ctx, key: &str) -> Row {
  Row::new()
    .id("team-new-computer")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, SpacingSize::Sm))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Md)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(icon(Icon::Monitor, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(&ctx.t(key))
        .id("team-new-computer-text")
        .variant(HubText::Value.role())
        .color(PaletteColor::TextSecondary)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
}
