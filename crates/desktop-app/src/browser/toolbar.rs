//! The window's panel under the title bar (`Body` `JiHDn`, `Panel` `DsHim`) and its toolbar (`Toolbar` `Zv6Z5`): back,
//! forward and reload; the address line, which ends with the owner of the tab shown (*Run #102 · ORC-107*, *Your
//! tab*); and *Open in a browser window* (decision 7). Under the toolbar the line that says what an agent did or that
//! the person's input holds it (`page::activity`), and a line when the browser could not do what was asked; then the
//! page. Without a browser the toolbar is faded and takes no input (`07` `YEv2c`); with the browser in a window of its
//! own the address line shows the tab as it was and the button is gone (`05` `GjcAk`).

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    events::KeyboardEvent,
    theme::{BorderSize, FormTextRole, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextInput, TextOverflow},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_ipc::browser::{BrowserCommand, BrowserMode, BrowserTabId, Navigation};
use parking_lot::Mutex;

use super::{
  model::{self, Owner},
  page::activity,
  state::Showing,
};
use crate::{
  theme::{BrowserSpacing, BrowserText, DISABLED_OPACITY, ExtraColor, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    icon_button::IconButton,
  },
};

/// The toolbar (`Toolbar` `Zv6Z5`) and the address line (`Address` `fPcCX`).
const TOOLBAR_HEIGHT: f32 = 44.0;
const ADDRESS_HEIGHT: f32 = 28.0;

/// The panel with the toolbar, the lines under it and `page`.
pub fn panel(ctx: &mut Ctx, showing: &Showing, failed: bool, page: Element) -> Element {
  let toolbar = toolbar(ctx, showing);
  let activity = (!showing.unavailable() && showing.mode() == BrowserMode::Headless)
    .then(|| activity::line(ctx, showing))
    .flatten();
  let failure = failed.then(|| {
    Row::new()
      .id("browser-failed")
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(BrowserSpacing::Activity.role(), SpacingSize::Sm))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .child(icon(Icon::CircleAlert, IconSize::Md, PaletteColor::Danger))
      .child(Text::new(&ctx.t("browser.failed")).variant(TypographyStyle::Description))
  });
  Column::new()
    .id("browser-body")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .left(SpacingSize::Sm)
        .right(SpacingSize::Sm)
        .bottom(SpacingSize::Sm),
    )
    .background(PaletteColor::SurfaceBase)
    .child(
      Column::new()
        .id("browser-panel")
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .min_height(Dimension::Px(0.0))
        .clip()
        .background(PaletteColor::SurfacePanel)
        .border(Border::inside(BorderSize::Sm, ExtraColor::PanelBorder.role()))
        .rounded(RadiusSize::Lg)
        .child(toolbar)
        .with_children(activity)
        .with_children(failure)
        .child(page),
    )
    .into()
}

fn toolbar(ctx: &mut Ctx, showing: &Showing) -> Element {
  let tab = showing.shown().cloned();
  let usable = !showing.unavailable() && showing.mode() == BrowserMode::Headless;
  let navigate = |ctx: &mut Ctx, (id, glyph, key): (&str, Icon, &str), to: Navigation| {
    let button = IconButton::new(id, glyph, ctx.t(key));
    match tab.clone().filter(|_| usable) {
      Some(tab) => {
        let commands = showing.commands.clone();
        button
          .on_click(move || {
            commands.send(BrowserCommand::Navigate {
              tab: tab.clone(),
              to: to.clone(),
            });
          })
          .render(ctx)
      }
      None => button.render(ctx),
    }
  };
  let back = navigate(
    ctx,
    ("browser-back", Icon::ArrowLeft, "browser.toolbar.back"),
    Navigation::Back,
  );
  let forward = navigate(
    ctx,
    ("browser-forward", Icon::ArrowRight, "browser.toolbar.forward"),
    Navigation::Forward,
  );
  let reload = navigate(
    ctx,
    ("browser-reload", Icon::RotateCw, "browser.toolbar.reload"),
    Navigation::Reload,
  );
  let address = ctx.mount_keyed::<Address>("browser-address", showing.clone());
  let open_in_window = (showing.mode() == BrowserMode::Headless).then(|| {
    let button = ActionButton::new(
      "browser-open-in-window",
      ButtonKind::Ghost,
      ctx.t("browser.toolbar.openInWindow"),
    )
    .icon(Icon::AppWindow);
    if showing.unavailable() {
      button.disabled()
    } else {
      let commands = showing.commands.clone();
      button.on_click(move || commands.send(BrowserCommand::OpenInWindow))
    }
  });
  let row = Row::new()
    .id("browser-toolbar")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(TOOLBAR_HEIGHT))
    .flex_shrink(0.0)
    .padding(Padding::horizontal(SpacingSize::Sm))
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(back)
    .child(forward)
    .child(reload)
    // The 4 more before the address line (`Gap` `K8rnhr`).
    .child(Row::new().padding(Padding::new().left(SpacingSize::Xs)))
    .child(address)
    .with_children(open_in_window.map(|button| Row::new().padding(Padding::new().left(SpacingSize::Xs)).child(button)));
  if showing.unavailable() {
    row.opacity(DISABLED_OPACITY).into()
  } else {
    row.into()
  }
}

/// The address line: the address of the tab shown, which the person can type over and open with Enter.
pub struct Address {
  value: Signal<String>,
  focused: Signal<bool>,
  /// The tab and address the value follows while the person is not typing.
  follows: Mutex<Option<(Option<BrowserTabId>, String)>>,
}

impl Component for Address {
  type Props = Showing;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      value: ctx.signal(String::new()),
      focused: ctx.signal(false),
      follows: Mutex::new(None),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let showing = ctx.props::<Showing>().clone();
    let shown = showing.shown().and_then(|tab| showing.strip.find(tab)).cloned();
    let address = shown
      .as_ref()
      .map(|tab| model::shown_address(&tab.url).to_owned())
      .filter(|address| address != "about:blank")
      .unwrap_or_default();
    self.follow(shown.as_ref().map(|tab| tab.id.clone()), &address);
    let usable = !showing.unavailable() && showing.mode() == BrowserMode::Headless;
    let input = self.input(ctx, &showing, shown.as_ref().map(|tab| tab.id.clone()), usable);
    let owner = shown.as_ref().map(|tab| owner(ctx, &tab.owner));
    let stroke = if self.focused.get() || (showing.strip.is_empty() && usable) {
      PaletteColor::BorderFocus
    } else {
      PaletteColor::Border
    };
    Row::new()
      .id("browser-address-box")
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .height(Dimension::Px(ADDRESS_HEIGHT))
      .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .background(PaletteColor::SurfaceInput)
      .border(Border::inside(BorderSize::Sm, stroke))
      .rounded(RadiusSize::Md)
      .child(icon(Icon::Globe, IconSize::Base, PaletteColor::TextMuted))
      .child(input)
      .with_children(owner)
  }
}

impl Address {
  /// While the person is not typing, the value follows the tab shown and its address.
  fn follow(&self, tab: Option<BrowserTabId>, address: &str) {
    if self.focused.get_untracked() {
      return;
    }
    let mut follows = self.follows.lock();
    let next = (tab, address.to_owned());
    if follows.as_ref() != Some(&next) {
      self.value.set(next.1.clone());
      *follows = Some(next);
    }
  }

  fn input(&self, ctx: &Ctx, showing: &Showing, tab: Option<BrowserTabId>, usable: bool) -> TextInput {
    let theme = ctx.theme();
    let resolve = |color| {
      FormTextRole {
        typography: BrowserText::Address.role(),
        color,
      }
      .resolve(&theme.typography(), &theme.palette())
    };
    let placeholder = if showing.unavailable() {
      ctx.t("browser.toolbar.noBrowser")
    } else {
      ctx.t("browser.toolbar.typeAddress")
    };
    let (gained, lost) = (self.focused.clone(), self.focused.clone());
    let (value, commands) = (self.value.clone(), showing.commands.clone());
    let hooks = showing.person.clone();
    let input = TextInput::styled(self.value.clone(), resolve(PaletteColor::TextPrimary))
      .placeholder_style(resolve(PaletteColor::TextMuted))
      .placeholder(&placeholder)
      .caret_color(PaletteColor::TextPrimary)
      .id("browser-address")
      .describe("a11y_name", ctx.t("browser.toolbar.address"))
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .single_line()
      .on_focus(move || gained.set(true))
      .on_blur(move || lost.set(false))
      .on_key_down(move |event: KeyboardEvent| {
        if event.key != "Enter" {
          return;
        }
        event.prevent_default();
        let Some(url) = model::typed_address(&value.get_untracked()) else {
          return;
        };
        match tab.clone() {
          Some(tab) => commands.send(BrowserCommand::Navigate {
            tab,
            to: Navigation::Address { url },
          }),
          None => {
            let hooks = hooks.clone();
            commands.send_then(BrowserCommand::OpenTab { url: Some(url) }, move |reply| {
              if let orchester_ipc::browser::BrowserReply::TabOpened { tab } = reply {
                hooks.pick(&tab);
              }
            });
          }
        }
      });
    if usable { input } else { input.focusable(false) }
  }
}

/// Whose the tab shown is, at the address line's end (`Owner` `esAS2`).
fn owner(ctx: &Ctx, owner: &Owner) -> Element {
  let (glyph, text): (Icon, Arc<str>) = match owner {
    Owner::Person => (Icon::User, ctx.t("browser.toolbar.ownerYours")),
    Owner::Run { known: Some(known), .. } => (
      Icon::Bot,
      ctx.t_args(
        "browser.toolbar.ownerRun",
        [
          ("number", known.number.get().to_string()),
          ("key", known.key.to_string()),
        ],
      ),
    ),
    Owner::Run { known: None, .. } => (Icon::Bot, Arc::from("")),
  };
  Row::new()
    .id("browser-address-owner")
    .flex_shrink(0.0)
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Sm, PaletteColor::TextMuted))
    .child(
      Text::new(&text)
        .variant(ExtraTypography::Note.role())
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    )
    .into()
}
