//! The *Users* tab of an admin (`R8ziTN` in 10.07): every account of the server with its role and
//! state, *Reset password* and *Disable* on each row (*Enable* on a disabled one, acting at once),
//! the admin's own row pointing to Account, and *Create user*.

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextOverflow},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_api::User;
use orchester_core::id::UserId;

use super::{
  ServerDialog,
  account::request_entry,
  details::is_self,
  frame::BUTTON_HEIGHT,
  table::{Cell, Width, heading, row, table},
};
use crate::{
  messages::Message,
  sharing::{ServerEntry, feed::ServerFeed},
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    field::field_error,
    icon::{Icon, IconSize, icon},
    message_text,
    status_label::status_label,
  },
};

/// Row and row button heights, and the column widths (`Vokgg`, `PJVMQ`, `FZL3Y`, `PLOU1`, `iDsFA`).
const ROW_HEIGHT: f32 = 56.0;
const ROW_BUTTON_HEIGHT: f32 = 28.0;
const ROLE_WIDTH: f32 = 80.0;
const STATE_WIDTH: f32 = 110.0;
const ACTIONS_WIDTH: f32 = 300.0;
/// The actions in a narrow window: *Reset password* and *Disable* side by side.
const NARROW_ACTIONS_WIDTH: f32 = 250.0;

/// A user of the server as the page shows them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Person {
  pub id: UserId,
  pub username: String,
  pub display_name: String,
  pub admin: bool,
  pub disabled: bool,
}

impl From<User> for Person {
  fn from(user: User) -> Self {
    Self {
      id: user.id,
      username: user.username,
      display_name: user.display_name,
      admin: user.admin,
      disabled: user.disabled,
    }
  }
}

#[derive(Clone)]
pub(super) struct UsersProps {
  pub entry: ServerEntry,
  pub dialog: Signal<ServerDialog>,
  /// The users as the details read them; `None` while they are read for the first time.
  pub people: Option<Result<People, Message>>,
  /// Bumped after an Enable, for the details to read the users again.
  pub reload: Signal<u64>,
}

impl PartialEq for UsersProps {
  fn eq(&self, other: &Self) -> bool {
    self.entry == other.entry && self.people == other.people
  }
}

impl lurq::app::component::DevtoolsInspectable for UsersProps {}

/// The server's users.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct People(pub Vec<Person>);

impl lurq::app::component::DevtoolsInspectable for People {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Enabled;

impl lurq::app::component::DevtoolsInspectable for Enabled {}

type EnableAction = FutureAction<(ServerEntry, UserId), Enabled, Message>;

pub(super) struct UsersTab;

impl Component for UsersTab {
  type Props = UsersProps;

  fn create(ctx: &mut Ctx) -> Self {
    let enable = enable_action(ctx);
    let reload = ctx.props::<UsersProps>().reload.clone();
    ctx.watch(&enable.state(), move |state| {
      if !state.is_pending() && state.data.is_some() {
        reload.update(|count| *count += 1);
      }
    });
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<UsersProps>().clone();
    let enable = enable_action(ctx);
    let people = props.people.clone();
    let problem = enable.state().get().error;
    let content: Element = match people {
      Some(Ok(People(people))) => users_table(ctx, &props, &people, &enable),
      Some(Err(error)) => Column::new()
        .id("server-users-error")
        .child(field_error(&message_text(ctx, &error)))
        .into(),
      None => Element::new(),
    };
    Column::new()
      .id("server-users")
      .width(Dimension::Pct(100.0))
      .padding_vertical(SpacingSize::Lg)
      .padding_horizontal(SpacingSize::Section)
      .spacing(ExtraSpacing::Step.role())
      .child(heading_line(ctx, &props))
      .child(content)
      .with_children(problem.map(|error| {
        Column::new()
          .id("server-enable-error")
          .child(field_error(&message_text(ctx, &error)))
      }))
      .child(admin_note(ctx))
  }
}

/// The users of `entry`'s server for an admin, read again when `revision` changes; `None` while
/// they are read for the first time, and for a person who is not an admin.
pub(super) fn load_people(
  ctx: &mut Ctx,
  entry: &ServerEntry,
  admin: bool,
  revision: (u64, u64),
) -> Option<Result<People, Message>> {
  let servers = ServerFeed::of(ctx).servers;
  let deps = (request_entry(entry), admin, revision);
  let load = ctx.future(deps, move |(entry, admin, _)| {
    let servers = servers.clone();
    async move {
      if !admin {
        return Ok(None);
      }
      match servers.users(&entry).await {
        Ok(users) => Ok(Some(People(users.into_iter().map(Person::from).collect()))),
        Err(error) => Err(error.message(&entry.address)),
      }
    }
  });
  let state = load.state().get();
  match (state.data, state.error) {
    (_, Some(error)) => Some(Err(error)),
    (Some(people), None) => people.map(Ok),
    (None, None) => None,
  }
}

/// *Users of …*, what admins do here, and *Create user* (`qHZZs`).
fn heading_line(ctx: &Ctx, props: &UsersProps) -> Element {
  let (dialog, server) = (props.dialog.clone(), props.entry.stored.id);
  let create = ActionButton::new(ctx.t("servers.users.create"))
    .icon(Icon::UserPlus)
    .id("server-create-user")
    .height(Dimension::Px(BUTTON_HEIGHT))
    .on_click(move || dialog.set(ServerDialog::CreateUser(server)))
    .render(ctx);
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Lg)
    .align_items(Alignment::Center)
    .child(
      Column::new()
        .flex(1.0)
        .spacing(ExtraSpacing::Tight.role())
        .child(
          Text::new(&ctx.t_args("servers.users.heading", [("server", props.entry.stored.name.as_str())]))
            .variant(ExtraTypography::SectionHeading.role()),
        )
        .child(
          Text::new(&ctx.t("servers.users.description"))
            .variant(ExtraTypography::Detail.role())
            .width(Dimension::Pct(100.0)),
        ),
    )
    .child(create)
    .into()
}

fn users_table(ctx: &Ctx, props: &UsersProps, people: &[Person], enable: &EnableAction) -> Element {
  // A narrow window drops State; a disabled user says so under the username.
  let wide = ctx.window().logical_width() >= crate::theme::SERVERS_DETAILS_WIDE_MIN_WINDOW;
  let actions_width = Width::Px(if wide { ACTIONS_WIDTH } else { NARROW_ACTIONS_WIDTH });
  let heading = heading(
    [
      Some((Width::Fill, Some(ctx.t("servers.users.name")))),
      Some((Width::Px(ROLE_WIDTH), Some(ctx.t("servers.users.role")))),
      wide.then(|| (Width::Px(STATE_WIDTH), Some(ctx.t("servers.users.state")))),
      Some((actions_width, None)),
    ]
    .into_iter()
    .flatten(),
  );
  let admins = people.iter().filter(|person| person.admin && !person.disabled).count();
  let rows = people.iter().map(|person| {
    let role = if person.admin {
      Text::new(&ctx.t("servers.users.admin"))
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::TextPrimary)
    } else {
      Text::new(&ctx.t("servers.users.member")).variant(TypographyStyle::Caption)
    };
    let (glyph, state) = if person.disabled {
      (Icon::CircleSlash, ctx.t("servers.users.disabled"))
    } else {
      (Icon::CircleCheck, ctx.t("servers.users.active"))
    };
    let last_admin = person.admin && !person.disabled && admins == 1;
    let disabled = (!wide && person.disabled).then_some(&*state);
    row(
      Dimension::Px(ROW_HEIGHT),
      [
        Some(Cell::new(Width::Fill, name(ctx, props, person, disabled))),
        Some(Cell::new(Width::Px(ROLE_WIDTH), role)),
        wide.then(|| Cell::new(Width::Px(STATE_WIDTH), status_label(glyph, &state))),
        Some(Cell::new(actions_width, actions(ctx, props, person, last_admin, enable)).at_end()),
      ]
      .into_iter()
      .flatten(),
    )
  });
  table("server-user-table", heading, rows)
}

/// The display name, *you* on the admin's own row, and the username in mono; in a narrow window a
/// disabled user's state after it.
fn name(ctx: &Ctx, props: &UsersProps, person: &Person, disabled: Option<&str>) -> Element {
  let you = is_self(&props.entry, person.id).then(|| {
    Text::new(&ctx.t("servers.users.you"))
      .variant(ExtraTypography::Footnote.role())
      .color(PaletteColor::TextMuted)
  });
  Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(SpacingSize::Xs)
    .child(
      Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(
          Text::new(&person.display_name)
            .variant(ExtraTypography::FileName.role())
            .nowrap()
            .text_overflow(TextOverflow::Elipsis),
        )
        .with_children(you),
    )
    .child(
      Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(Text::new(&person.username).variant(TypographyStyle::Mono).nowrap())
        .with_children(disabled.map(|state| {
          Text::new(state)
            .variant(ExtraTypography::Footnote.role())
            .color(PaletteColor::TextMuted)
            .nowrap()
        })),
    )
    .into()
}

fn actions(ctx: &Ctx, props: &UsersProps, person: &Person, last_admin: bool, enable: &EnableAction) -> Element {
  let row = Row::new().spacing(SpacingSize::Sm).align_items(Alignment::Center);
  if is_self(&props.entry, person.id) {
    return row
      .child(Text::new(&ctx.t("servers.users.ownPassword")).variant(TypographyStyle::Caption))
      .into();
  }
  let id = |what: &str| Arc::<str>::from(format!("server-user-{what}-{}", person.username));
  let server = props.entry.stored.id;
  if person.disabled {
    let pending = enable.state().get().is_pending();
    let (action, entry, user) = (enable.clone(), props.entry.clone(), person.id);
    let button = ActionButton::new(ctx.t("servers.users.enable"))
      .icon(Icon::UserCheck)
      .id(id("enable"))
      .height(Dimension::Px(ROW_BUTTON_HEIGHT));
    let button = if pending {
      button.on_click(|| {}).enabled(false)
    } else {
      button.on_click(move || action.run((entry.clone(), user)))
    };
    return row.child(button.render(ctx)).into();
  }
  let (reset, disable) = (props.dialog.clone(), props.dialog.clone());
  let (for_reset, for_disable) = (person.clone(), person.clone());
  row
    .child(
      ActionButton::new(ctx.t("servers.users.reset"))
        .icon(Icon::KeyRound)
        .id(id("reset"))
        .height(Dimension::Px(ROW_BUTTON_HEIGHT))
        .on_click(move || reset.set(ServerDialog::ResetPassword(server, for_reset.clone())))
        .render(ctx),
    )
    .child(
      ActionButton::new(ctx.t("servers.users.disable"))
        .icon(Icon::UserX)
        .id(id("disable"))
        .height(Dimension::Px(ROW_BUTTON_HEIGHT))
        .on_click(move || disable.set(ServerDialog::Disable(server, for_disable.clone(), last_admin)))
        .render(ctx),
    )
    .into()
}

/// What disabling does, and where members are managed (`p01RpC`).
fn admin_note(ctx: &Ctx) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Start)
    .child(icon(Icon::Info, IconSize::Small, PaletteColor::TextSecondary))
    .child(
      Text::new(&ctx.t("servers.users.note"))
        .variant(ExtraTypography::CellLabel.role())
        .flex(1.0),
    )
    .into()
}

/// Enables a disabled user at once.
fn enable_action(ctx: &mut Ctx) -> EnableAction {
  let servers = ServerFeed::of(ctx).servers;
  ctx.future_action(move |(entry, user): (ServerEntry, UserId)| {
    let servers = servers.clone();
    async move {
      servers
        .set_disabled(&entry, user, false)
        .await
        .map(|_| Enabled)
        .map_err(|error| {
          tracing::error!(%error, server = %entry.stored.id, "cannot enable a user");
          error.message(&entry.address)
        })
    }
  })
}
