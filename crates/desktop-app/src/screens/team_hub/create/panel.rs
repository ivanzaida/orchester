//! The form that creates a resource in the panel (`Agents · New agent profile` … `New tool`, A7–A12 in `acFkz`): the
//! kind and *not created yet · Create publishes v1* over the new resource's title, *For the team* (what the team holds,
//! the same on every computer) as rows, a note on what each computer fills in afterwards, and pinned at the bottom
//! Create and Cancel. No draft: Create makes v1 and sends it to the team server; it shows as created only once the
//! server has taken it ([`super::work`]) and the hub has read its list again with it, the form staying as it was sent
//! with Create pending meanwhile; a refusal is said in place and nothing is made, and Cancel leaves nothing.
//! In a local project (`Agents · New agent profile` … `New tool`, L3–L7) the same form has one section of the
//! resource's own fields and nothing to send: Create makes v1 on this computer, or for an MCP server, which has no
//! versions, the server itself.

use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, ScrollVertical, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::id::ProjectId;
use orchester_workspace::{
  agents::team_tool::{CallingPipeline, calling_pipelines},
  connections::data::ProjectConnections,
  local_archive::Archivable,
  sharing::{team::TeamConfiguration, team_edit::Refusal},
};

use super::{
  fields::{Body, Shown},
  instruction, kinds, local_mcp,
  mcp::Fields as TeamMcp,
  mode::Mode,
  notices, profile, skill, tool,
  work::{self, Failed, Press, Work},
};
use crate::{
  mcp::input_gate::{Guarded, InputGate, may_press},
  navigation::Navigation,
  queries::{self, Session, hub},
  routes::{HubView, ProjectPage, Route},
  screens::{
    connections::data,
    hub::{
      profile_form::{Connect, Offer},
      team::admins,
    },
    local_hub::view_of,
    team_hub::{bar, model::Kind, panel as shared, rows},
  },
  services::Services,
  theme::{HubSpacing, HubText, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
  },
};

#[derive(Clone, PartialEq)]
pub(in crate::screens) struct Props {
  pub repo_root: PathBuf,
  pub project: ProjectId,
  pub kind: Kind,
  /// Where the form creates its resource.
  pub mode: Mode,
  /// The names the resources of the kind have already; a new one takes another.
  pub taken: Vec<String>,
  /// A team project's configuration: who the admins are, which a refusal for a role that changed says.
  pub team: Option<TeamConfiguration>,
  /// What the hub's list holds of the kind now: the form gives way to what it made only once the list holds it.
  pub listed: Vec<Archivable>,
}
impl DevtoolsInspectable for Props {}

pub(in crate::screens) fn panel(ctx: &mut Ctx, props: Props) -> Element {
  ctx.mount_keyed::<NewPanel>(
    &format!(
      "{}-hub-new-{}-{}",
      props.mode.prefix(),
      props.kind.name(),
      props.project
    ),
    props,
  )
}

/// The fields of the kind's form: they live as long as the form is open, and a form of another kind is another mount.
enum Form {
  Profile(profile::Fields),
  McpServer(TeamMcp),
  LocalMcpServer(local_mcp::Fields),
  Instruction(instruction::Fields),
  Skill(skill::Fields),
  Tool(tool::Fields),
}

impl Form {
  fn new(ctx: &mut Ctx, props: &Props) -> Self {
    match (props.kind, props.mode) {
      (Kind::McpServer, Mode::Local) => Self::LocalMcpServer(local_mcp::Fields::new(ctx)),
      // A local profile runs on an account of the person's, which the project gets its connection on.
      (Kind::Profile, Mode::Local) => {
        let connect = Connect::new(ctx, props.repo_root.clone(), props.project);
        Self::Profile(profile::Fields::new(ctx, Some(connect)))
      }
      (kind, _) => Self::of_team_kind(ctx, kind),
    }
  }

  /// The form of `kind` whose fields a team and a local project share (an MCP server's differ).
  fn of_team_kind(ctx: &mut Ctx, kind: Kind) -> Self {
    match kind {
      Kind::Profile => Self::Profile(profile::Fields::new(ctx, None)),
      Kind::McpServer => Self::McpServer(TeamMcp::new(ctx)),
      Kind::Instruction => Self::Instruction(instruction::Fields::new(ctx)),
      Kind::Skill => Self::Skill(skill::Fields::new(ctx)),
      Kind::Tool => Self::Tool(tool::Fields::new(ctx)),
    }
  }

  fn body(&self, ctx: &mut Ctx, shown: &Shown<'_>) -> Body {
    match self {
      Self::Profile(fields) => fields.body(ctx, shown),
      Self::McpServer(fields) => fields.body(ctx, shown),
      Self::LocalMcpServer(fields) => fields.body(ctx, shown),
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
  /// What Create made, while the hub's list does not hold it yet: the form stays, Create pending, until it does.
  made: Signal<Option<Archivable>>,
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
    let work = work::work(ctx, (props.repo_root.clone(), props.project, props.mode));
    // Created, and taken by the team server: the hub reads its list again, and the form waits for it (`give_way`),
    // so the panel never shows the list's last answer without it.
    let made = ctx.signal(None);
    let kept = made.clone();
    ctx.watch(&work.state(), move |state| {
      if let Some(made) = state.data.filter(|_| state.is_fulfilled()) {
        kept.set(Some(made));
      }
    });
    Self {
      form: Form::new(ctx, &props),
      tried: ctx.signal(false),
      work,
      made,
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
      mode,
      taken,
      team,
      listed,
    } = ctx.props::<Props>().clone();
    if self.focus_first.get() {
      ctx.focus(&self.first);
      self.focus_first.set(false);
    }
    let made = self.made.get();
    let navigation = Navigation::of(ctx);
    give_way(made, &listed, (project, kind), &navigation);
    let state = self.work.state().get();
    // Made and not listed yet is still Create's: the form stays as it was sent.
    let pending = state.is_pending() || made.is_some();
    // The list that holds what was made holds its name too, which the form does not take for a clash with itself.
    let taken = if made.is_some() { Vec::new() } else { taken };
    let refused = state.error.filter(|_| !pending);
    let refused_name = taken_by_server(refused.as_ref());
    let offer = self.offer(ctx, (kind, mode), &repo_root);
    let connections = self.connections(ctx, (kind, mode), &repo_root);
    let pipelines = pipelines(ctx, (kind, mode), &repo_root);
    let shown = Shown {
      mode,
      taken: &taken,
      tried: self.tried.get(),
      locked: pending,
      offer: &offer,
      connections: connections.as_ref(),
      pipelines: &pipelines,
      first: &self.first,
      refused_name: refused_name.as_deref(),
      published: None,
    };
    let body = self.form.body(ctx, &shown);
    let prefix = mode.prefix();
    let content = Column::new()
      .id(format!("{prefix}-new-form"))
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(
        HubSpacing::Sections.role(),
        HubSpacing::BodyTop.role(),
      ))
      .spacing(HubSpacing::BodyTop.role())
      .child(head(ctx, kind, mode))
      .with_children(refusal(ctx, refused.as_ref(), team.as_ref()))
      .with_children(body.lead)
      .with_children(sections(ctx, (kind, mode), (body.rows, body.after), body.computer))
      .with_children(body.overlay);
    let scroll = ScrollVertical::new(content)
      .id(format!("{prefix}-hub-panel-scroll"))
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .min_height(Dimension::Px(0.0));
    let bar = self.bar(ctx, &body.make, (body.off, pending), (project, kind, mode), &navigation);
    Column::new()
      .id(format!("{prefix}-hub-panel"))
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
  /// What the profile form's fields offer: this computer's connections, which its CLI and model choices come from,
  /// and the resources its lists give, the team's live ones or the project's.
  fn offer(&self, ctx: &mut Ctx, (kind, mode): (Kind, Mode), root: &Path) -> Offer {
    if kind != Kind::Profile {
      return Offer::default();
    }
    let session = Session::of(ctx);
    let read = queries::state(&ctx.query(hub::profile_form(session, root.to_path_buf()))).data;
    let config = (mode == Mode::Team).then(|| Services::of(ctx).workspace.team_read(root).config().cloned());
    Offer::read(read.as_ref(), config.as_ref().map(|config| config.as_deref()))
  }

  /// The project's MCP servers and secrets, which a local MCP server's form takes the secrets its variables name from,
  /// and a local tool's form the secret it sends or connects with.
  fn connections(&self, ctx: &mut Ctx, (kind, mode): (Kind, Mode), root: &Path) -> Option<ProjectConnections> {
    if mode != Mode::Local || !matches!(kind, Kind::McpServer | Kind::Tool) {
      return None;
    }
    queries::state(&data::load(ctx, root.to_path_buf())).data
  }

  /// Create and Cancel, with what Create does under them at the bar's end.
  fn bar(
    &self,
    ctx: &Ctx,
    make: &Option<work::Make>,
    (off, busy): (Option<&'static str>, bool),
    (project, kind, mode): (ProjectId, Kind, Mode),
    navigation: &Navigation,
  ) -> Element {
    let prefix = mode.prefix();
    let create = ActionButton::new(
      format!("{prefix}-hub-new-create"),
      ButtonKind::Primary,
      ctx.t("team_new.bar.create"),
    )
    .icon(Icon::Plus)
    .pinned();
    let create = if busy {
      create.loading()
    } else if off.is_some() {
      create.disabled()
    } else {
      let (work, tried, gate, make) = (self.work.clone(), self.tried.clone(), self.gate.clone(), make.clone());
      create.on_click(move || {
        // What adds to what agents may do is a person's press, or automation's where they let it change agent
        // permissions; refused, the form stays as it was.
        let refused = make.as_ref().is_some_and(|make| {
          make.needs_a_person()
            && !may_press(
              gate.as_ref(),
              Guarded::PermissionWidening,
              "create what adds to what agents may do",
            )
        });
        if refused {
          return;
        }
        tried.set(true);
        if let Some(make) = make.clone() {
          work.run(Press { make });
        }
      })
    };
    let cancel = ActionButton::new(
      format!("{prefix}-hub-new-cancel"),
      ButtonKind::Secondary,
      ctx.t("team_new.bar.cancel"),
    )
    .pinned();
    let cancel = if busy {
      cancel.disabled()
    } else {
      cancel.opens(navigation.to(Route::Project(
        project,
        ProjectPage::hub(HubView::new(kind.filter(), None)),
      )))
    };
    let hint = match off {
      Some(why) => (ctx.t(why).to_string(), PaletteColor::Danger),
      None => (ctx.t(mode.bar_hint(kind)).to_string(), PaletteColor::TextMuted),
    };
    bar::bar_saying(vec![create.into(), cancel.into()], Some(hint))
  }
}

/// The team's pipelines, which a team tool's form lets call it; none for another form.
fn pipelines(ctx: &mut Ctx, (kind, mode): (Kind, Mode), root: &Path) -> Vec<CallingPipeline> {
  if (kind, mode) != (Kind::Tool, Mode::Team) {
    return Vec::new();
  }
  Services::of(ctx)
    .workspace
    .team_read(root)
    .config()
    .map_or_else(Vec::new, |config| calling_pipelines(config))
}

/// The form gives way to what it `made` once the hub's list holds it (`listed`): the route goes to it in place, so the
/// list and the panel show it chosen at once. Before that the list's last answer does not hold it, and going to it
/// would show another resource, or the kind's empty tab.
fn give_way(
  made: Option<Archivable>,
  listed: &[Archivable],
  (project, kind): (ProjectId, Kind),
  navigation: &Navigation,
) {
  if let Some(made) = made.filter(|made| listed.contains(made)) {
    let shown = Route::Project(project, ProjectPage::hub(view_of(kind.filter(), Some(made))));
    navigation.replace(shown);
  }
}

/// Why nothing was created, in place, under the note.
fn refusal(ctx: &Ctx, refused: Option<&Failed>, team: Option<&TeamConfiguration>) -> Option<Element> {
  // Only a team's server says a role changed, and then its configuration names the admins.
  let admins = refused
    .and(team)
    .map_or_else(Default::default, |team| admins(ctx, team));
  refused.map(|failed| notices::refused(ctx, failed, &admins))
}

/// A name the team server said is taken, which the list this computer holds did not have.
fn taken_by_server(refused: Option<&Failed>) -> Option<String> {
  match refused {
    Some(Failed::Refused(Refusal::NameTaken, name)) => Some(name.clone()),
    _ => None,
  }
}

/// What the form holds: *For the team*, the kind's own sections and the note on what each computer fills in afterwards
/// in a team's project; one section of the resource's own fields and its own sections in a local one (`Profile`,
/// `Server`, `Instruction`, `Skill`, `Tool` in L3–L7).
fn sections(
  ctx: &Ctx,
  (kind, mode): (Kind, Mode),
  (rows, after): (Vec<Row>, Vec<Element>),
  computer_note: Option<Arc<str>>,
) -> Vec<Element> {
  let first: Element = match mode {
    Mode::Team => for_team(ctx, rows).into(),
    Mode::Local => rows::section("local-new-section", &ctx.t(kinds::section_key(kind)), Vec::new(), rows).into(),
  };
  let note = computer_note
    .filter(|_| mode == Mode::Team)
    .map(|note| computer(&note).into());
  std::iter::once(first).chain(after).chain(note).collect()
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

/// The head: what it is and that it is not created yet over its title (`Head` in A7, L3).
fn head(ctx: &Ctx, kind: Kind, mode: Mode) -> Row {
  shared::head_named(
    &ctx.t(kinds::title_key(kind)),
    (
      ctx.t(kind.where_key()).to_string(),
      Some(ctx.t(mode.not_created(kind)).to_string()),
    ),
    None,
  )
}

/// What each computer fills in afterwards, apart from the team's rows (`Computer` note in A7).
fn computer(words: &str) -> Row {
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
      Text::new(words)
        .id("team-new-computer-text")
        .variant(HubText::Value.role())
        .color(PaletteColor::TextSecondary)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
}
