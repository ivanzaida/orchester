//! A shared profile's page on this computer (`Shared configuration · redesign` `UC5xk`): what it names that is missing
//! here (`04` `w0Evp`: the bar with *Choose on this computer…*, which opens Sharing › On this computer), a teammate's
//! version that widens what agents may do here (`02d` `FuOXA`: what it adds, with *Use v<N> here*, which only the
//! person presses), *Runs here use* (`06` `sakAV`, `06c` `FO7Cq`): the person's own connection the profile's runs
//! take on this computer, chosen through the same picker as On this computer, kept here and never sent; and the page
//! head's *runs here: …* or *Can't run here*.

mod adds;

use std::{path::PathBuf, sync::Arc};

use adds::adds_list;
use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::{ConnectionId, ResourceId},
  resource::{AgentProfile, VersionNumber},
  run::Answerer,
  sharing::VersionWidening,
};
use orchester_store::{Bound, BoundBy, BoundReference, ProjectDb};
use orchester_workspace::{
  background,
  sharing::sync::{self, Here, HereReference, Standing},
};

use crate::{
  screens::{
    hub::{
      draft_bar::{DraftBar, Standing as BarStanding},
      parts::{Tone as DotTone, note},
    },
    settings::orchester::{PersonOnly, Said, follow_shared, sharing_said},
    shared_config::{
      bind_action,
      missing::{self, Whose},
      picker::{self, Item, Opener},
    },
  },
  services::Services,
  theme::{ExtraColor, ExtraSpacing, HubSpacing, HubText},
  ui::{
    button::{ActionButton, ButtonKind},
    chip::{BadgeTone, badge},
    field::field_error,
    icon::Icon,
  },
};

/// *Runs here use*'s select, as wide as the section's other dropdowns (`vxuTD` in `BzwDA`).
const SELECT_WIDTH: f32 = 260.0;
/// *Use v<N> here*, which only a person presses.
const USE_HERE: &str = "agent-use-here";
/// A connection chosen for *Runs here use*, which only a person chooses.
const RUNS_HERE: &str = "agent-runs-here";

/// What this computer has of a shared profile: its references here, and a version that waits for the person.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Shared {
  here: Here,
  waiting: Option<Waiting>,
}

impl DevtoolsInspectable for Shared {}

/// A teammate's version that widens, who published it, what it adds, and the version runs use meanwhile.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Waiting {
  number: VersionNumber,
  used: Option<VersionNumber>,
  publisher: String,
  adds: Vec<VersionWidening>,
  /// The MCP servers and secrets it names that the version runs use does not, as this computer has them.
  added: Vec<HereReference>,
}

/// What the page shows of a shared profile.
#[derive(Default)]
pub(super) struct Parts {
  /// The bars under the page head.
  pub bars: Vec<Element>,
  /// *Runs here use* with the note under its section.
  pub runs_here: Option<(Element, Element)>,
  /// What the page head ends in: *runs here: …* or *Can't run here*.
  pub status: Option<(DotTone, Arc<str>)>,
}

/// What the page shows of it, read for the profile `props` shows.
pub(super) fn parts(ctx: &mut Ctx, props: &super::editor::Props) -> Parts {
  let Some(entry) = &props.entry else {
    return Parts::default();
  };
  let root = props.project.open.repo_root.clone();
  let name = entry.profile.role().as_str().to_owned();
  match read(ctx, root.clone(), entry.id, props.revision.get()) {
    Err(reason) => Parts {
      bars: vec![field_error("agent-shared-unread", &reason.text(ctx)).into()],
      ..Parts::default()
    },
    Ok(None) => Parts::default(),
    Ok(Some(shared)) => {
      let project = props.project.open.project.id();
      let bars = bars(
        ctx,
        &shared,
        (project, entry.id, &name),
        (root.clone(), &props.revision),
      );
      Parts {
        bars,
        status: head_status(ctx, &shared, entry.id),
        runs_here: runs_here(ctx, &shared, (entry.id, &name), (root, &props.revision)),
      }
    }
  }
}

/// Reads it for profile `resource` of the project at `root`; anew when `revision` or a shared project's cache changes.
fn read(ctx: &mut Ctx, root: PathBuf, resource: ResourceId, revision: u64) -> Result<Option<Shared>, Said> {
  let servers = Services::of(ctx).servers;
  let shared = follow_shared(ctx, &servers);
  let state = ctx
    .future((root, resource, revision, shared), |(root, resource, ..)| async move {
      background::run(move || read_shared(&root, resource))
        .await
        .map_err(|_| Said::new("cloud.errors.local"))?
        .map_err(|error| sharing_said(&error))
    })
    .state()
    .get();
  // A failed re-read keeps the last data; it must not stay answerable (Use v<N> here, Runs here use) as if current.
  match (state.data, state.error) {
    (_, Some(reason)) => Err(reason),
    (Some(shared), None) => Ok(Some(shared)),
    (None, None) => Ok(None),
  }
}

fn read_shared(
  root: &std::path::Path,
  resource: ResourceId,
) -> Result<Shared, orchester_workspace::sharing::SharingError> {
  let here = Here::read(root)?;
  let waiting = match sync::waiting(root, resource)? {
    Some(waiting) => {
      // A version waits only once it is cached from the server, with its publisher; a cache cleared meanwhile
      // leaves the bar without the name rather than without the version.
      let publisher = ProjectDb::open(root)?
        .cached_version::<AgentProfile>(resource, waiting.number)?
        .map(|cached| cached.publisher.name)
        .unwrap_or_default();
      let added = here
        .added_in(resource, waiting.number, waiting.used)
        .into_iter()
        .filter(|reference| !matches!(reference.binding.reference, BoundReference::Connection { .. }))
        .cloned()
        .collect();
      Some(Waiting {
        number: waiting.number,
        used: waiting.used,
        publisher,
        adds: waiting.adds,
        added,
      })
    }
    None => None,
  };
  Ok(Shared { here, waiting })
}

/// The bars under the page head: what is missing here, and the version that waits for *Use v<N> here*.
fn bars(
  ctx: &mut Ctx,
  shared: &Shared,
  (project, resource, name): (orchester_core::id::ProjectId, ResourceId, &str),
  (root, revision): (PathBuf, &Signal<u64>),
) -> Vec<Element> {
  // *Missing on this computer: connection “DeepSeek”* and *Choose on this computer…* (`Pf5rK` in 04).
  let missing = shared.here.missing_for(resource);
  let mut bars: Vec<Element> = missing::bar(ctx, "agent-missing", &missing, (project, name, Whose::Resource))
    .into_iter()
    .collect();
  if let Some(waiting) = &shared.waiting {
    bars.push(ctx.mount_keyed::<UseHere>(
      &format!("agent-use-here-{resource}"),
      UseHereProps {
        root,
        resource,
        waiting: waiting.clone(),
        revision: revision.clone(),
      },
    ));
  }
  bars
}

/// The status the page head ends in: *runs here: Claude Code · Work* once the profile's connection is bound here,
/// *Can't run here* while it is missing; `None` for a profile that names no connection this computer binds.
fn head_status(ctx: &Ctx, shared: &Shared, resource: ResourceId) -> Option<(DotTone, Arc<str>)> {
  let reference = shared.here.connection_of(resource)?;
  Some(match &reference.standing {
    Standing::Bound { local, .. } => (
      DotTone::Success,
      ctx.t_args("sharing.runsHere.head", [("connection", local.as_str())]),
    ),
    Standing::Found { .. } | Standing::Missing => (DotTone::Danger, ctx.t("sharing.runsHere.cantRun")),
  })
}

#[derive(Clone)]
struct UseHereProps {
  root: PathBuf,
  resource: ResourceId,
  waiting: Waiting,
  revision: Signal<u64>,
}

impl PartialEq for UseHereProps {
  fn eq(&self, other: &Self) -> bool {
    self.root == other.root && self.resource == other.resource && self.waiting == other.waiting
  }
}

impl DevtoolsInspectable for UseHereProps {}

/// The widens bar with *Use v<N> here* (`V097I` in 02d).
struct UseHere {
  /// Takes the version the bar shows when pressed, so a newer version that arrives while it is mounted is the one
  /// used.
  use_here: FutureAction<VersionNumber, (), Said>,
  person: PersonOnly,
}

impl Component for UseHere {
  type Props = UseHereProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<UseHereProps>().clone();
    let use_here = ctx.future_action(move |number: VersionNumber| {
      let (root, resource, revision) = (props.root.clone(), props.resource, props.revision.clone());
      async move {
        background::run(move || sync::use_here(&root, resource, number, Answerer::Person))
          .await
          .map_err(|_| Said::new("cloud.errors.local"))?
          .map_err(|error| sharing_said(&error))?;
        revision.update(|count| *count += 1);
        Ok(())
      }
    });
    Self {
      use_here,
      person: PersonOnly::new(ctx),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<UseHereProps>().clone();
    let version = props.waiting.number.get().to_string();
    let adds = adds_list(ctx, &props.waiting);
    let state = self.use_here.state().get();
    let button = ActionButton::new(
      USE_HERE,
      ButtonKind::Primary,
      ctx.t_args("sharing.config.widens.use", [("version", version.as_str())]),
    )
    .icon(Icon::Check);
    let button = if state.is_pending() {
      button.loading()
    } else {
      let (action, person, number) = (self.use_here.clone(), self.person.clone(), props.waiting.number);
      button.on_click(move || {
        if person.allows(USE_HERE) {
          action.run(number);
        }
      })
    };
    let names = |used: &str| {
      [
        ("publisher", props.waiting.publisher.clone()),
        ("version", version.clone()),
        ("used", used.to_owned()),
      ]
    };
    let (sub, compare) = match props.waiting.used {
      Some(used) => {
        let used = used.get().to_string();
        // Profiles have no compare view yet: the action is shown, unavailable, with why.
        let compare = ActionButton::new(
          "agent-widens-compare",
          ButtonKind::Secondary,
          ctx.t_args("sharing.config.widens.compare", names(&used)),
        )
        .unavailable(ctx.t("sharing.config.widens.compareLater"));
        (
          ctx.t_args("sharing.config.widens.bodyUsed", names(&used)),
          Some(compare),
        )
      }
      None => (ctx.t_args("sharing.config.widens.body", names("")), None),
    };
    let mut bar = DraftBar::new(
      "agent-widens-bar",
      BarStanding::Widens,
      ctx.t_args("sharing.config.widens.title", [("version", version.as_str())]),
    )
    .sub(sub)
    .list(adds)
    .foot(ctx.t("sharing.config.widens.personOnly"));
    if let Some(compare) = compare {
      bar = bar.action(compare);
    }
    let bar = bar.action(button);
    let failed = state
      .error
      .clone()
      .filter(|_| !state.is_pending())
      .map(|reason| field_error("agent-use-here-error", &reason.text(ctx)));
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(bar)
      .with_children(self.person.reason(ctx, USE_HERE))
      .with_children(failed)
  }
}

/// *Runs here use* as the last row of Connection & model, and the note under the section; `None` for a profile whose
/// versions name no connection this computer binds.
fn runs_here(
  ctx: &mut Ctx,
  shared: &Shared,
  (resource, name): (ResourceId, &str),
  (root, revision): (PathBuf, &Signal<u64>),
) -> Option<(Element, Element)> {
  let reference = shared.here.connection_of(resource)?;
  let role = reference.binding.role.clone().unwrap_or_default();
  let none_here = shared.here.connections_of(&role).is_empty();
  let row = ctx.mount_keyed::<RunsHere>(
    &format!("agent-runs-here-{resource}"),
    RunsHereProps {
      root,
      reference: reference.clone(),
      choices: shared
        .here
        .connections_of(&role)
        .into_iter()
        .map(|connection| (connection.id(), connection.name().as_str().into()))
        .collect(),
      name: name.to_owned(),
      revision: revision.clone(),
    },
  );
  let words = ctx.t(if none_here {
    "sharing.runsHere.noteNoAccount"
  } else {
    "sharing.runsHere.note"
  });
  Some((row, note("agent-runs-here-note", Icon::Monitor, &words).into()))
}

#[derive(Clone)]
struct RunsHereProps {
  root: PathBuf,
  reference: HereReference,
  /// The local connections of the reference's role, by id and name.
  choices: Vec<(ConnectionId, Arc<str>)>,
  /// The profile's name, for *Builder can't run here*.
  name: String,
  revision: Signal<u64>,
}

impl PartialEq for RunsHereProps {
  fn eq(&self, other: &Self) -> bool {
    self.root == other.root
      && self.reference == other.reference
      && self.choices == other.choices
      && self.name == other.name
  }
}

impl DevtoolsInspectable for RunsHereProps {}

/// The row (`Runs here use` in 06): its label with *Yours*, the hint, and the picker or *Add an account…*.
struct RunsHere {
  bind: FutureAction<BoundReference, (), Said>,
  person: PersonOnly,
}

impl Component for RunsHere {
  type Props = RunsHereProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<RunsHereProps>().clone();
    Self {
      bind: bind_action(ctx, props.root, props.revision),
      person: PersonOnly::new(ctx),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<RunsHereProps>().clone();
    let (hint, control) = if props.choices.is_empty() {
      (
        Text::new(&ctx.t_args("sharing.runsHere.noAccount", [("resource", props.name.as_str())]))
          .id("agent-runs-here-hint")
          .variant(HubText::Hint.role())
          .color(PaletteColor::Danger),
        picker::add_account(ctx, "agent-runs-here-add-account"),
      )
    } else {
      (
        Text::new(&ctx.t("sharing.runsHere.hint"))
          .id("agent-runs-here-hint")
          .variant(HubText::Hint.role()),
        self.picker(ctx, &props),
      )
    };
    let label = Row::new()
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(
        Text::new(&ctx.t("sharing.runsHere.label"))
          .id("agent-runs-here-label")
          .variant(crate::theme::ExtraTypography::Strong.role()),
      )
      .child(badge(
        "agent-runs-here-yours",
        BadgeTone::Neutral,
        &ctx.t("sharing.runsHere.yours"),
      ));
    let state = self.bind.state().get();
    let failed = state
      .error
      .clone()
      .filter(|_| !state.is_pending())
      .map(|reason| field_error("agent-runs-here-error", &reason.text(ctx)));
    Row::new()
      .id("agent-runs-here-row")
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(HubSpacing::Row.role(), HubSpacing::FieldY.role()))
      .spacing(HubSpacing::FieldGap.role())
      .align_items(Alignment::Center)
      .background(ExtraColor::Hover.role())
      .child(
        Column::new()
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .spacing(ExtraSpacing::Tight.role())
          .child(label)
          .child(hint.width(Dimension::Pct(100.0)))
          .with_children(self.person.reason(ctx, RUNS_HERE))
          .with_children(failed),
      )
      .child(control)
  }
}

impl RunsHere {
  /// The person's connections of the role, the one runs take here checked, then *Add an account…*.
  fn picker(&self, ctx: &mut Ctx, props: &RunsHereProps) -> Element {
    let current = match (&props.reference.binding.reference, &props.reference.standing) {
      (BoundReference::Connection { bound: Some(bound), .. }, Standing::Bound { .. }) => Some(bound.local),
      _ => None,
    };
    let words = props
      .choices
      .iter()
      .find(|(id, _)| Some(*id) == current)
      .map_or_else(|| ctx.t("sharing.runsHere.choose"), |(_, name)| name.clone());
    let items = props
      .choices
      .iter()
      .map(|(id, name)| Item {
        label: name.clone(),
        glyph: Icon::User,
        detail: None,
        chosen: Some(*id) == current,
        unavailable: None,
      })
      .collect();
    let (bind, person, ids, target) = (
      self.bind.clone(),
      self.person.clone(),
      props.choices.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
      props.reference.binding.reference,
    );
    picker::picker(
      ctx,
      picker::Props {
        id: "agent-runs-here".into(),
        opener: Opener::Select {
          words,
          width: SELECT_WIDTH,
        },
        items,
        add_account: true,
        busy: self.bind.state().get().is_pending(),
        choose: Arc::new(move |index| {
          let (BoundReference::Connection { reference, .. }, Some(local)) = (target, ids.get(index)) else {
            return;
          };
          if person.allows(RUNS_HERE) {
            bind.run(BoundReference::Connection {
              reference,
              bound: Some(Bound {
                local: *local,
                by: BoundBy::Person,
              }),
            });
          }
        }),
      },
    )
  }
}
