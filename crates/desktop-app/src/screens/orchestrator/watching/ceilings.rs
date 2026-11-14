//! The ceilings of a subscription that leads its target to the end, in the `/watching` answer (`Ceilings · Webhooks
//! v2` `q5tvtE` in `L02` `HpcQJ`; `Parts · Ceilings` a–c `H6wvm`, board `Orchestrator autonomy · frames to approve`;
//! `docs/plan/orchestrator-autonomy/OA-10-…`).
//!
//! Right under the subscription's head row, a row 30 high, in past the head's icon: a 12 `gauge` in `text_muted`,
//! then each ceiling as its name in 11.5 `text_muted` and its value in Geist Mono 11 `text_secondary`, 5 apart:
//! *Runs of a task 3*, *Returns per run 4*, *Raises 1 by 50 %* and *Budget 12M of 40M tokens* (what its work spent of
//! what it may; *Budget none* without one); at its end *Change…* in 12 `text_secondary`.
//!
//! *Change…* opens the form 6 under it from its end (`Ceilings form` `Ea9jV`): 440 wide on `surface_raised` in a
//! `border_strong` stroke, radius 8, 16 inside, its parts 14 apart. A 13 `gauge` and *Ceilings · Webhooks v2* in
//! 13/600; a row per ceiling, its name in 12.5/500 over one hint line in 11.5 `text_muted` that names its bounds, and
//! its field at the row's end, 12 after them, the value in Geist Mono 12 at the box's end; *by* and *%* around the
//! share a limit is raised by, *tokens* after the budget, whose hint says what was used. Under a `border` line, 12
//! over it: *This subscription only · defaults in Project settings ↗*, which opens them, **Cancel** and **Save**. A
//! field out of bounds takes the `danger` stroke and its hint is replaced by what is allowed, in `danger` (c); Save
//! does nothing until every field is in bounds, then applies at once to this subscription and closes the form. Esc,
//! a press outside and Cancel close it and change nothing.
//!
//! A budget is typed as every budget is, an absolute whole number of tokens grouped while it is typed, never *40M*
//! (`orchester_core::tokens`), so its box is wider than the frame's 76.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, ShadowStyle, SpacingSize},
  },
  components::{Column, Row, Spacer, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::ProjectId,
  orchestrator::{Ceilings, WatchTarget},
  tokens::compact_tokens,
};
use orchester_workspace::orchestrator::{
  typed_ceilings::{TypedCeilings, Wrong},
  watching::LedToEnd,
};

use super::{
  LeadChange,
  answer::{Line, REST_HEIGHT, action, line},
};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, ProjectSettingsTab, Route},
  theme::{
    CEILINGS_FORM_WIDTH, DISABLED_OPACITY, ExtraRadius, ExtraSpacing, ExtraTypography, IconSize, LeadingText,
    WatchingText,
  },
  ui::{
    button::{ActionButton, ButtonKind},
    field::FieldInput,
    icon::{Icon, icon},
    popover::{PopoverSide, popover},
    pressable::Press,
  },
};

/// The boxes of the runs and the returns (`QHDtk`), of how often a limit is raised (`P392FZ`) and by how much
/// (`P2ISk`).
const COUNT_WIDTH: f32 = 64.0;
const TIMES_WIDTH: f32 = 44.0;
const SHARE_WIDTH: f32 = 52.0;
/// The budget's box: the frame's (`i7gbip`, 76) holds *40M*; this one holds a budget as it is typed, *40,000,000*.
const BUDGET_WIDTH: f32 = 112.0;

/// The ceilings row of one subscription.
#[derive(Clone)]
pub(super) struct CeilingsProps {
  /// The row's element id; *Change…* is `{id}-change` and the form `{id}-form`.
  pub(super) id: String,
  pub(super) project: ProjectId,
  pub(super) target: WatchTarget,
  /// The target as people read it, which the form's title names.
  pub(super) name: Arc<str>,
  pub(super) led: LedToEnd,
  /// A change of how a target is taken is under way: *Change…* waits for it.
  pub(super) busy: bool,
  pub(super) change: Arc<dyn Fn(WatchTarget, LeadChange) -> bool + Send + Sync>,
}

/// Compared by what it shows; the change is the screen's.
impl PartialEq for CeilingsProps {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
      && self.project == other.project
      && self.target == other.target
      && self.name == other.name
      && self.led == other.led
      && self.busy == other.busy
  }
}

impl DevtoolsInspectable for CeilingsProps {}

/// The ceilings row named by `props`, mounted by its id so its form keeps what is typed in it.
pub(super) fn row(ctx: &mut Ctx, props: CeilingsProps) -> Element {
  ctx.mount_keyed::<CeilingsRow>(&props.id.clone(), props)
}

/// The form's fields, as typed.
#[derive(Clone)]
struct Fields {
  runs: Signal<String>,
  returns: Signal<String>,
  raises: Signal<String>,
  percent: Signal<String>,
  budget: Signal<String>,
}

impl Fields {
  fn new(ctx: &mut Ctx) -> Self {
    Self {
      runs: ctx.signal(String::new()),
      returns: ctx.signal(String::new()),
      raises: ctx.signal(String::new()),
      percent: ctx.signal(String::new()),
      budget: ctx.signal(String::new()),
    }
  }

  /// The fields hold `typed`.
  fn fill(&self, typed: TypedCeilings) {
    self.runs.set(typed.runs);
    self.returns.set(typed.returns);
    self.raises.set(typed.raises);
    self.percent.set(typed.percent);
    self.budget.set(typed.budget);
  }

  /// What the fields hold, read tracked when `tracked`: the form follows what is typed.
  fn typed(&self, tracked: bool) -> TypedCeilings {
    let read = |field: &Signal<String>| if tracked { field.get() } else { field.get_untracked() };
    TypedCeilings {
      runs: read(&self.runs),
      returns: read(&self.returns),
      raises: read(&self.raises),
      percent: read(&self.percent),
      budget: read(&self.budget),
    }
  }
}

struct CeilingsRow {
  open: Signal<bool>,
  anchor: ElementRef,
  fields: Fields,
}

impl Component for CeilingsRow {
  type Props = CeilingsProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
      anchor: ctx.element_ref(),
      fields: Fields::new(ctx),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<CeilingsProps>().clone();
    let separator = ctx.t("orchestrator.lead.ceilings.groupSeparator");
    let ceilings = props.led.ceilings;
    // The form opens on the subscription's ceilings as they are now, whatever was typed in it before.
    let (open, fields, shown) = (
      self.open.clone(),
      self.fields.clone(),
      TypedCeilings::of(ceilings, &separator),
    );
    let change = action(
      &format!("{}-change", props.id),
      &ctx.t("orchestrator.lead.ceilings.change"),
      Press::run(move || {
        fields.fill(shown.clone());
        open.update(|open| *open = !*open);
      }),
    )
    .ref_element(self.anchor.clone())
    .describe("expanded", self.open.get().to_string());
    // Always in the same frame: the form hangs from *Change…*, which must stay the same element while a change runs.
    let change = Row::new().flex_shrink(0.0).child(change);
    let change = if props.busy {
      change.opacity(DISABLED_OPACITY)
    } else {
      change
    };
    let form = self.open.get().then(|| {
      popover(
        self.anchor.clone(),
        self.open.clone(),
        PopoverSide::BelowEnd,
        self.form(ctx, &props),
      )
    });
    line(Line::Under, REST_HEIGHT)
      .id(props.id.clone())
      .describe("budget", ceilings.budget().map_or(0, |budget| budget.get()).to_string())
      .child(icon(Icon::Gauge, IconSize::Sm, PaletteColor::TextMuted))
      .with_children(said(ctx, &props.id, props.led))
      .child(Spacer::new().flex(1.0))
      .child(change)
      .with_children(form)
  }
}

/// The four ceilings of `led` as the row says them, each named `{id}-{name}`.
fn said(ctx: &Ctx, id: &str, led: LedToEnd) -> Vec<Element> {
  let ceilings = led.ceilings;
  let raises = ctx.t_args(
    "orchestrator.lead.ceilings.raisesValue",
    [
      ("times", ceilings.raises_of_run().to_string()),
      ("percent", ceilings.raise_percent().to_string()),
    ],
  );
  let budget = match ceilings.budget() {
    Some(budget) => ctx.t_args(
      "orchestrator.lead.ceilings.budgetValue",
      [
        ("used", compact_tokens(led.spent.unwrap_or_default().get())),
        ("most", compact_tokens(budget.get())),
      ],
    ),
    None => ctx.t("orchestrator.lead.ceilings.budgetNone"),
  };
  let one = |name: &str, value: &str| -> Element {
    Row::new()
      .id(format!("{id}-{name}"))
      .flex_shrink(0.0)
      .spacing(ExtraSpacing::Pair.role())
      .align_items(Alignment::Center)
      .child(
        Text::new(&ctx.t(&format!("orchestrator.lead.ceilings.{name}")))
          .variant(WatchingText::Meta.role())
          .nowrap(),
      )
      .child(
        Text::new(value)
          .id(format!("{id}-{name}-value"))
          .variant(LeadingText::Ceiling.role())
          .nowrap(),
      )
      .into()
  };
  vec![
    one("runs", &ceilings.runs_of_task().to_string()),
    one("returns", &ceilings.returns_of_run().to_string()),
    one("raises", &raises),
    one("budget", &budget),
  ]
}

impl CeilingsRow {
  /// The form: its head, a row per ceiling, and its foot.
  fn form(&self, ctx: &mut Ctx, props: &CeilingsProps) -> Column {
    let id = format!("{}-form", props.id);
    let separator = ctx.t("orchestrator.lead.ceilings.groupSeparator");
    let wrong = self.fields.typed(true).read(&separator).err().unwrap_or_default();
    let head = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(icon(Icon::Gauge, IconSize::Base, PaletteColor::TextMuted))
      .child(
        Text::new(&ctx.t_args("orchestrator.lead.ceilings.form.title", [("name", &*props.name)]))
          .id(format!("{id}-title"))
          .variant(ExtraTypography::FormSection.role())
          .nowrap(),
      );
    let rows = self.rows(ctx, &id, (props.led, &wrong, &separator));
    Column::new()
      .id(id.clone())
      .width(Dimension::Px(CEILINGS_FORM_WIDTH))
      .padding(SpacingSize::Lg)
      .spacing(ExtraSpacing::Inset.role())
      .rounded(ExtraRadius::Tile.role())
      .background(PaletteColor::SurfaceRaised)
      .border_inside(BorderSize::Sm, PaletteColor::BorderStrong)
      .box_shadow(ShadowStyle::Md)
      .child(head)
      .with_children(rows)
      .child(self.foot(ctx, &id, props))
  }

  /// The four rows; a field named in `wrong` says what is allowed in place of its hint.
  fn rows(&self, ctx: &mut Ctx, id: &str, (led, wrong, separator): (LedToEnd, &[Wrong], &str)) -> Vec<Element> {
    let fields = &self.fields;
    let bounds = |ctx: &Ctx, key: &str, (min, max): (u8, u8)| {
      ctx.t_args(key, [("min", min.to_string()), ("max", max.to_string())])
    };
    let allowed = |ctx: &Ctx, range: (u8, u8)| bounds(ctx, "orchestrator.lead.ceilings.form.bounds", range);
    let runs_range = (Ceilings::MIN_RUNS, Ceilings::MAX_RUNS);
    let runs = wrong.contains(&Wrong::Runs);
    let runs_row = form_row(
      &format!("{id}-runs"),
      &ctx.t("orchestrator.lead.ceilings.form.runs.label"),
      if runs {
        (allowed(ctx, runs_range), true)
      } else {
        (
          bounds(ctx, "orchestrator.lead.ceilings.form.runs.hint", runs_range),
          false,
        )
      },
      vec![number(
        ctx,
        (
          &format!("{id}-runs-field"),
          "orchestrator.lead.ceilings.form.runs.label",
        ),
        (&fields.runs, COUNT_WIDTH),
        runs,
      )],
    );
    let returns_range = (0, Ceilings::MAX_RETURNS);
    let returns = wrong.contains(&Wrong::Returns);
    let returns_row = form_row(
      &format!("{id}-returns"),
      &ctx.t("orchestrator.lead.ceilings.form.returns.label"),
      if returns {
        (allowed(ctx, returns_range), true)
      } else {
        (
          bounds(ctx, "orchestrator.lead.ceilings.form.returns.hint", returns_range),
          false,
        )
      },
      vec![number(
        ctx,
        (
          &format!("{id}-returns-field"),
          "orchestrator.lead.ceilings.form.returns.label",
        ),
        (&fields.returns, COUNT_WIDTH),
        returns,
      )],
    );
    vec![
      runs_row,
      returns_row,
      self.raises_row(ctx, id, wrong),
      self.budget_row(ctx, id, (led, wrong.contains(&Wrong::Budget), separator)),
    ]
  }

  /// *Raises of a limit*: how often, *by*, how much, *%*; the first of the two that is out of bounds says what is
  /// allowed.
  fn raises_row(&self, ctx: &mut Ctx, id: &str, wrong: &[Wrong]) -> Element {
    let times = wrong.contains(&Wrong::Raises);
    let below = wrong.contains(&Wrong::PercentBelow);
    let share = below || wrong.contains(&Wrong::PercentAbove);
    let (min, max) = (0, Ceilings::MAX_RAISES);
    let most = Ceilings::MAX_RAISE_PERCENT.to_string();
    let hint = if times {
      let args = [("min", min.to_string()), ("max", max.to_string())];
      (ctx.t_args("orchestrator.lead.ceilings.form.bounds", args), true)
    } else if below {
      let least = TypedCeilings::MIN_RAISE_PERCENT.to_string();
      (
        ctx.t_args("orchestrator.lead.ceilings.form.raises.percentLeast", [("min", least)]),
        true,
      )
    } else if share {
      (
        ctx.t_args("orchestrator.lead.ceilings.form.raises.percentMost", [("max", most)]),
        true,
      )
    } else {
      let args = [("min", min.to_string()), ("max", max.to_string()), ("percent", most)];
      (ctx.t_args("orchestrator.lead.ceilings.form.raises.hint", args), false)
    };
    let controls = vec![
      number(
        ctx,
        (
          &format!("{id}-raises-field"),
          "orchestrator.lead.ceilings.form.raises.times",
        ),
        (&self.fields.raises, TIMES_WIDTH),
        times,
      ),
      unit(&ctx.t("orchestrator.lead.ceilings.form.raises.by")),
      number(
        ctx,
        (
          &format!("{id}-percent-field"),
          "orchestrator.lead.ceilings.form.raises.share",
        ),
        (&self.fields.percent, SHARE_WIDTH),
        share,
      ),
      unit(&ctx.t("orchestrator.lead.ceilings.form.raises.percent")),
    ];
    form_row(
      &format!("{id}-raises"),
      &ctx.t("orchestrator.lead.ceilings.form.raises.label"),
      hint,
      controls,
    )
  }

  /// *Budget*: the tokens the subscription's work may spend, empty for none; its hint says what was used.
  fn budget_row(&self, ctx: &mut Ctx, id: &str, (led, wrong, separator): (LedToEnd, bool, &str)) -> Element {
    let label = ctx.t("orchestrator.lead.ceilings.form.budget.label");
    let hint = match (wrong, led.spent) {
      (true, _) => (ctx.t("orchestrator.lead.ceilings.form.budget.invalid"), true),
      (false, Some(spent)) => (
        ctx.t_args(
          "orchestrator.lead.ceilings.form.budget.hintUsed",
          [("used", compact_tokens(spent.get()))],
        ),
        false,
      ),
      (false, None) => (ctx.t("orchestrator.lead.ceilings.form.budget.hint"), false),
    };
    let field = FieldInput::new(format!("{id}-budget-field"), label.clone(), self.fields.budget.clone())
      .width(Dimension::Px(BUDGET_WIDTH))
      .text_style(ExtraTypography::PropertyMono.role())
      .end_aligned()
      .grouped(separator);
    let field = if wrong { field.invalid() } else { field };
    let controls = vec![
      field.render(ctx),
      unit(&ctx.t("orchestrator.lead.ceilings.form.budget.unit")),
    ];
    form_row(&format!("{id}-budget"), &label, hint, controls)
  }

  /// The foot: where the defaults are, **Cancel** and **Save**, which applies the ceilings once every field holds
  /// one.
  fn foot(&self, ctx: &mut Ctx, id: &str, props: &CeilingsProps) -> Element {
    let route = Route::Project(props.project, ProjectPage::Settings(ProjectSettingsTab::Orchestrator));
    let note = action(
      &format!("{id}-defaults"),
      &ctx.t("orchestrator.lead.ceilings.form.note"),
      Press::Open(Navigation::of(ctx).to(route)),
    );
    let closing = self.open.clone();
    let cancel = ActionButton::new(
      format!("{id}-cancel"),
      ButtonKind::Secondary,
      ctx.t("orchestrator.lead.ceilings.form.cancel"),
    )
    .in_form()
    .on_click(move || closing.set(false));
    let (open, fields, change, target) = (
      self.open.clone(),
      self.fields.clone(),
      props.change.clone(),
      props.target,
    );
    let separator = ctx.t("orchestrator.lead.ceilings.groupSeparator");
    let save = ActionButton::new(
      format!("{id}-save"),
      ButtonKind::Primary,
      ctx.t("orchestrator.lead.ceilings.form.save"),
    )
    .in_form()
    .on_click(move || {
      // A field out of bounds says so in the form; nothing is applied until none is.
      let Ok(ceilings) = fields.typed(false).read(&separator) else {
        return;
      };
      // A change that did not start (it is a person's alone, and one may be under way) leaves the form as it is.
      if change(target, LeadChange::Ceilings(ceilings)) {
        open.set(false);
      }
    });
    Row::new()
      .id(format!("{id}-foot"))
      .width(Dimension::Pct(100.0))
      .padding(Padding::new().top(SpacingSize::Md))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .child(Row::new().flex(1.0).min_width(Dimension::Px(0.0)).child(note))
      .child(cancel)
      .child(save)
      .into()
  }
}

/// A row of the form, named `id`: `label` over its `hint` (in `danger` when it says what is allowed instead), and
/// its `controls` at its end.
fn form_row(id: &str, label: &str, (hint, wrong): (Arc<str>, bool), controls: Vec<Element>) -> Element {
  let hint = Text::new(&hint)
    .id(format!("{id}-hint"))
    .variant(ExtraTypography::Note.role())
    .width(Dimension::Pct(100.0));
  let hint = if wrong { hint.color(PaletteColor::Danger) } else { hint };
  let words = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Fine.role())
    .child(
      Text::new(label)
        .id(format!("{id}-label"))
        .variant(ExtraTypography::OptionTitle.role()),
    )
    .child(hint);
  Row::new()
    .id(id.to_owned())
    .describe("wrong", wrong.to_string())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(words)
    .with_children(controls)
    .into()
}

/// A field for a small whole number, `width` wide, named `id` and, for tools, by the words at the key `name`; in the
/// `danger` stroke while its value is `wrong`.
fn number(ctx: &mut Ctx, (id, name): (&str, &str), (value, width): (&Signal<String>, f32), wrong: bool) -> Element {
  let name = ctx.t(name);
  let field = FieldInput::new(id.to_owned(), name, value.clone())
    .width(Dimension::Px(width))
    .text_style(ExtraTypography::PropertyMono.role())
    .end_aligned();
  let field = if wrong { field.invalid() } else { field };
  field.render(ctx)
}

/// *by*, *%* or *tokens* beside a field.
fn unit(words: &str) -> Element {
  Text::new(words)
    .variant(LeadingText::Unit.role())
    .nowrap()
    .flex_shrink(0.0)
    .into()
}
