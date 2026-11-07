//! The panel of a team tool (`Agents · Tools` `u9Hcc` A6, `yTVeE` M6; `docs/design/team-configuration.md`, "What a
//! computer fills in"): its head and chips, *From the team* (what it runs or calls, its arguments, who may call it
//! without asking and the secret it names, the same on every computer), *On this computer* (the value of the secret it
//! takes, and how it stands) and, pinned at the bottom, the fix of the first missing part, else the admin's *Edit*.
//!
//! A secret's value is the person's alone: a press that automation made does nothing and says so. A team tool names
//! its secret by id and has no team form for a name yet (`Undecided::TeamToolSecretById`), so there is nothing to
//! fill in for it here: the panel says so and offers no action.

mod computer;
mod from_team;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::{Row, Text},
  node::Element,
};
use orchester_core::{id::ProjectId, secret::SecretName, sharing::Missing};
use orchester_workspace::{
  agents::team_tool::TeamToolSheet,
  readiness::{ReadinessStanding, Undecided},
};

use super::{
  actions::{Actions, Busy},
  bar,
  model::{Entry, Listing},
  panel, secret, words,
};
use crate::{
  navigation::{Navigation, link::Open},
  routes::{ProjectPage, Route, ToolTab},
  screens::settings::orchester::PersonOnly,
  services::Services,
  theme::HubText,
  ui::{
    button::{ActionButton, ButtonKind},
    chip::BadgeTone,
    icon::Icon,
    pressable::Press,
  },
};

#[derive(Clone)]
pub(super) struct Props {
  pub repo_root: PathBuf,
  pub project: ProjectId,
  /// The person may change the team's resources: an admin of the team.
  pub may_change: bool,
  /// The hub is read again: what shows is the last answer, and a fix would act on it.
  pub reading: bool,
  pub listing: Listing,
  pub entry: Entry,
  pub sheet: TeamToolSheet,
}
impl DevtoolsInspectable for Props {}

impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.repo_root == other.repo_root
      && self.project == other.project
      && self.may_change == other.may_change
      && self.reading == other.reading
      && self.listing == other.listing
      && self.entry == other.entry
      && self.sheet == other.sheet
  }
}

/// The id the person-only check of a fix is made under.
const FIX: &str = "team-hub-fix";

pub(super) fn panel(ctx: &mut Ctx, props: Props) -> Element {
  ctx.mount_keyed::<ToolPanel>(&format!("team-hub-tool-{}", props.entry.id), props)
}

/// The panel of one tool: it is keyed by the resource, so an open dialog or a typed value never carries over to
/// another tool.
struct ToolPanel {
  actions: Actions,
  secret: secret::Entry,
  person: PersonOnly,
}

impl Component for ToolPanel {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<Props>().clone();
    let panel = Self {
      actions: Actions::new(ctx, &props.repo_root),
      secret: secret::Entry::new(ctx),
      person: PersonOnly::new(ctx),
    };
    panel.secret.follow_save(ctx, &panel.actions.add_value);
    panel
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let Props {
      listing, entry, sheet, ..
    } = &props;
    let missing = entry.standing.as_ref().map_or(&[][..], |standing| standing.missing());
    let now = Services::of(ctx).clock.now();
    let navigation = Navigation::of(ctx);
    let route = Route::Project(props.project, ProjectPage::Tool(entry.id, ToolTab::default()));
    let head_edit = props
      .may_change
      .then(|| panel::edit_button(ctx, navigation.to(route.clone())));
    let kind = ctx.t_args(
      "team_hub.tool.where",
      [("kind", from_team::kind(ctx, sheet).to_string())],
    );
    let version = words::published_line(ctx, now, entry, sheet.published.as_ref());
    let used_by = (!sheet.used_by.is_empty()).then(|| {
      Text::new(&ctx.t_args("team_hub.panel.usedBy", [("names", sheet.used_by.join(", "))]))
        .id("team-hub-used-by")
        .variant(HubText::Hint.role())
        .nowrap()
    });
    let chips = panel::chips_saying(ctx, entry, chip(ctx, listing, entry))
      .child(Row::new().flex(1.0))
      .with_children(used_by);
    let body = panel::body()
      .child(panel::head_of(entry, (kind.to_string(), version), head_edit))
      .child(chips)
      .child(from_team::section(ctx, sheet))
      .child(computer::section(ctx, entry, sheet, missing))
      .with_children(self.person.reason(ctx, FIX));
    let bar = self.bar(ctx, &props, missing, navigation.to(route));
    let dialog = self.secret.dialog(
      ctx,
      secret::Entry::save_by(&self.actions.add_value, &self.person),
      self.actions.add_value.state().get().is_pending(),
      self.person.reason(ctx, secret::SAVE),
    );
    panel::frame(entry, body, bar).with_children(dialog)
  }
}

impl Drop for ToolPanel {
  fn drop(&mut self) {
    self.secret.wipe();
  }
}

impl ToolPanel {
  /// The first missing part's fix: a value for its secret; else, for an admin, *Edit*.
  fn bar(&self, ctx: &Ctx, props: &Props, missing: &[Missing], edit: Open) -> Option<Element> {
    if let Some(Missing::Secret { name }) = missing.first() {
      return Some(self.add_value(ctx, props, name));
    }
    if !props.may_change {
      return None;
    }
    let button = ActionButton::new(
      "team-hub-action-edit",
      ButtonKind::Primary,
      ctx.t("team_hub.panel.edit"),
    )
    .icon(Icon::Pencil)
    .opens(edit);
    let hint = props.entry.number.and_then(|number| number.next()).map(|next| {
      ctx
        .t_args("team_hub.panel.publishNext", [("version", next.to_string())])
        .to_string()
    });
    Some(bar::bar(vec![button.into()], hint))
  }

  /// *Add value* for `name`: it opens the dialog, which only a person's press does.
  fn add_value(&self, ctx: &Ctx, props: &Props, name: &SecretName) -> Element {
    let (entry, person, asked) = (self.secret.clone(), self.person.clone(), name.clone());
    let press = Press::run(move || {
      if person.allows(FIX) {
        entry.ask(&asked);
      }
    });
    let button = ActionButton::new(
      "team-hub-fix-add-value",
      ButtonKind::Primary,
      ctx.t("team_hub.mcp.bar.addValue"),
    )
    .icon(Icon::KeyRound);
    let button = match self.actions.busy(props.reading) {
      Busy::Idle => button.press(press),
      Busy::Taking => button.loading(),
      Busy::Reading => button.disabled(),
    };
    bar::bar(
      vec![button.into()],
      Some(ctx.t("team_hub.mcp.bar.addValueHint").to_string()),
    )
  }
}

/// The panel's readiness chip: the line of the list, but for a secret named by id, which the panel says in its own
/// words.
fn chip(ctx: &Ctx, listing: &Listing, entry: &Entry) -> (Arc<str>, BadgeTone) {
  match &entry.standing {
    Some(ReadinessStanding::Undecided(Undecided::TeamToolSecretById(_))) => {
      (ctx.t("team_hub.tool.chip.secretById"), BadgeTone::Warning)
    }
    _ => words::chip(ctx, listing, entry),
  }
}
