//! The slash menu's panel (`Slash menu` `IQOLN` in `07` `jAiyu`, `v7Xwo` in `06` `yKWQx`): a `Menu` panel 640 wide,
//! its left edge at the composer's box and its bottom 8 over it. Under the head *Commands* the chat's commands; a line,
//! then under the head *Skills* the project's published skills (`graduation-cap`, the name in Geist Mono, the
//! description), which scroll after 8 rows; the keys under them ([`super::rows`]). A head says at its end how many of
//! its rows match once something is typed (*2 of 4*); else the commands' head the composer's own note and the skills'
//! how many are published. A section with no match is hidden; with nothing that matches at all (`07d` `G2ajn7`) the
//! menu says so, and that Enter sends the text as typed. While the agent the message goes to cannot take a skill,
//! every skill is faded and says why.

use std::{ops::Range, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::SpacingSize,
  },
  components::Placement,
  core::ElementRef,
  node::Element,
  ui,
};
use parking_lot::Mutex;

use super::{
  Pick, Run, Shown, SlashCommand, SlashField, argument, matched, query,
  rows::{Face, footer, heading, nothing, row},
  skills::{self, Read, SkillsOffer},
};
use crate::{
  theme::{ControlHeight, ExtraSpacing},
  ui::{
    icon::Icon,
    menu::{Entry, MenuPanel, menu_panel, separator},
    message::message_text,
  },
};

/// The menu's width (`IQOLN`).
const WIDTH: f32 = 640.0;
/// How many skills show before their section scrolls (`06d` `lXQi1`, the rules).
const SKILL_ROWS: u16 = 8;

/// The slash menu of the composer named `id`, over its box `anchor`, offering `commands` and the project's `skills`;
/// `note` is what the commands' head says at its end while nothing is typed (the run's agent and step in a run's
/// Conversation); `send` sends the draft as typed when nothing matches, where the field's own Enter does not.
pub struct SlashMenuProps {
  pub id: &'static str,
  pub field: SlashField,
  pub anchor: ElementRef,
  pub commands: Vec<SlashCommand>,
  pub skills: SkillsOffer,
  pub note: Option<Arc<str>>,
  pub send: Option<Run>,
}

#[derive(Clone)]
struct Props {
  id: &'static str,
  field: SlashField,
  anchor: ElementRef,
  faces: Vec<Face>,
  skills: SkillsOffer,
  note: Option<Arc<str>>,
}

/// Compared by what it shows; the anchor is the composer's for as long as it is shown.
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
      && self.field == other.field
      && self.faces == other.faces
      && self.skills == other.skills
      && self.note == other.note
  }
}

impl DevtoolsInspectable for Props {}

pub fn menu(ctx: &mut Ctx, menu: SlashMenuProps) -> Element {
  let faces = menu
    .commands
    .iter()
    .map(|command| Face {
      pick: Pick::Command(command.name),
      name: Arc::from(command.name),
      icon: command.icon,
      label: command.label.clone(),
      detail: command.detail.clone(),
      runs: command.does.is_some(),
      skill: false,
    })
    .collect();
  {
    let mut offered = menu.field.offered.lock();
    offered.commands = menu.commands;
    offered.send = menu.send;
  }
  let props = Props {
    id: menu.id,
    field: menu.field,
    anchor: menu.anchor,
    faces,
    skills: menu.skills,
    note: menu.note,
  };
  ctx.mount_keyed::<SlashMenu>(&format!("{}-slash", props.id), props)
}

/// The skills as the Skills section shows them: their rows and how many are published, or why they could not be read.
enum SkillRows {
  Shown(Vec<Face>),
  Unreadable(Arc<str>),
}

/// A row that matches what was typed, with its matched letters.
type Matched<'a> = (&'a Face, Range<usize>);

/// What the Skills section shows: how many skills are published, those that match, or why they could not be read.
struct SkillSection<'a> {
  total: usize,
  matched: &'a [Matched<'a>],
  unreadable: Option<Arc<str>>,
}

struct SlashMenu {
  /// The rows the menu showed last, what each chooses and whether it can be: lurq's active row goes back to the first
  /// that can be chosen when they change.
  last: Mutex<Option<Vec<(Pick, bool)>>>,
}

impl Component for SlashMenu {
  type Props = Props;

  fn create(_ctx: &mut Ctx) -> Self {
    Self { last: Mutex::new(None) }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let field = &props.field;
    let text = field.draft.get();
    let typed = query(&text);
    if typed.is_none() && field.closed.get() {
      field.closed.set(false);
    }
    // While a badge's picker is open, it shows instead; the picker's own search holds the focus then.
    let picking = field
      .argument
      .with(|held| held.iter().find(|badge| badge.picking).map(|badge| badge.command));
    field.take_focus_back(ctx);
    let focused = field.focused.get();
    let open = typed.is_some() && picking.is_none() && focused && !field.closed.get();
    let read = skills::read(ctx, &props.skills, open);
    let (Some(typed), true) = (typed, open) else {
      *field.shown.lock() = None;
      *self.last.lock() = None;
      return match picking {
        Some(command) => argument::picker(ctx, (props.id, field, &props.anchor), command),
        _ => {
          field.forget_choices();
          Element::new()
        }
      };
    };
    let skills = skill_rows(ctx, field, &props.skills, read);
    let skill_faces = match &skills {
      SkillRows::Shown(faces) => faces.as_slice(),
      SkillRows::Unreadable(_) => &[],
    };
    let commands = matching(&props.faces, typed);
    let matched_skills = matching(skill_faces, typed);
    let rows: Vec<(Pick, bool)> = commands
      .iter()
      .chain(&matched_skills)
      .map(|(face, _)| (face.pick.clone(), face.runs))
      .collect();
    let focused = self.focused(field, &rows);
    let any = !rows.is_empty();
    *field.shown.lock() = Some(Shown { rows });
    let panel = menu_panel(&format!("{}-slash-menu", props.id))
      .width(WIDTH)
      .keeps_focus(&field.keys);
    let panel = if !any && matches!(skills, SkillRows::Shown(_)) {
      panel.child(nothing(ctx, typed)).child(footer(ctx, typed, false))
    } else {
      let panel = panel.with_children(commands_section(ctx, &props, (typed, &commands), focused));
      let section = SkillSection {
        total: skill_faces.len(),
        matched: &matched_skills,
        unreadable: match &skills {
          SkillRows::Unreadable(why) => Some(why.clone()),
          SkillRows::Shown(_) => None,
        },
      };
      let skill_focus = focused.and_then(|at| at.checked_sub(commands.len()));
      skills_section(ctx, &props, (typed, section), skill_focus, panel).child(footer(ctx, typed, any))
    };
    hung(ctx, (&props.anchor, field), panel)
  }
}

/// `panel` as lurq's menu over the composer's box `anchor`, its bottom 8 over it, open while the draft asks for it: the
/// focus stays in the field, whose keys move the active row and choose it (`MenuPanel::keeps_focus`), and Esc closes
/// it until the draft no longer starts with `/`. A press elsewhere leaves it, as the field keeps it while it has the
/// focus.
fn hung(ctx: &Ctx, (anchor, field): (&ElementRef, &SlashField), panel: MenuPanel) -> Element {
  let closes = field.clone();
  // Held open by what the draft says: what a person does to close it is Esc, which `on_dismiss` hears.
  let menu = ui::Menu::controlled(true, anchor.clone(), |_| {}).on_dismiss(move || closes.close());
  panel
    .hang(menu)
    .placement(Placement::TopStart)
    .offset(ctx.theme().spacing_value(SpacingSize::Sm))
    .dismiss_on_outside_press(false)
    .dismiss_on_escape(false)
    .into()
}

impl SlashMenu {
  /// lurq's active row among `rows`, which shows *Enter* at its end: the first that can be chosen once they changed,
  /// as when the skills arrive after the menu opened, as lurq's menu sets it then; else where the keys or the pointer
  /// moved it. lurq draws the menu again when it moves.
  fn focused(&self, field: &SlashField, rows: &[(Pick, bool)]) -> Option<usize> {
    let mut last = self.last.lock();
    if last.as_deref() != Some(rows) {
      *last = Some(rows.to_vec());
      return rows.iter().position(|(_, runs)| *runs);
    }
    field
      .keys
      .active()
      .filter(|&at| rows.get(at).is_some_and(|(_, runs)| *runs))
  }
}

/// The rows of `faces` that match `typed` ([`face_matched`]), with the matched letters.
fn matching<'a>(faces: &'a [Face], typed: &str) -> Vec<Matched<'a>> {
  let mut found: Vec<Matched<'a>> = faces
    .iter()
    .filter_map(|face| face_matched(face, typed).map(|letters| (face, letters)))
    .collect();
  // A command whose own name is what was typed comes first, so Enter runs it (`/watching` is *What I watch*, not
  // *Stop watching*, whose label also holds the word); the rest keep their order.
  found.sort_by_key(|(face, _)| face.skill || !face.name.eq_ignore_ascii_case(typed));
  found
}

/// Where `typed` is in the row of `face`: in its label, whose letters show in bold; or in a command's own name, which
/// the person types after the `/` (`/unwatch` for *Stop watching*), with no letters in bold.
fn face_matched(face: &Face, typed: &str) -> Option<Range<usize>> {
  matched(&face.label, typed).or_else(|| {
    (!face.skill)
      .then(|| matched(&face.name, typed))
      .flatten()
      .map(|_| 0..0)
  })
}

/// The skills `read` found as rows, faded with why while the agent cannot take one; the field offers them to choose.
fn skill_rows(ctx: &Ctx, field: &SlashField, offer: &SkillsOffer, read: Read) -> SkillRows {
  let skills = match read {
    Read::Ready(skills) => skills,
    Read::Reading => Vec::new(),
    Read::Failed(reason) => {
      field.offered.lock().skills.clear();
      let reason = message_text(ctx, &reason);
      return SkillRows::Unreadable(ctx.t_args("composer.slash.skillsUnreadable", [("reason", reason)]));
    }
  };
  let faces = skills
    .iter()
    .map(|skill| Face {
      pick: Pick::Skill(skill.id),
      name: Arc::from(format!("skill-{}", skill.slug)),
      icon: Icon::GraduationCap,
      label: skill.slug.clone(),
      detail: offer.refused.clone().unwrap_or_else(|| skill.description.clone()),
      runs: offer.refused.is_none(),
      skill: true,
    })
    .collect();
  field.offered.lock().skills = skills;
  SkillRows::Shown(faces)
}

/// *Commands* and the commands that match `typed`, the focus at `focused`; nothing when none matches.
fn commands_section(
  ctx: &Ctx,
  props: &Props,
  (typed, commands): (&str, &[Matched<'_>]),
  focused: Option<usize>,
) -> Vec<Entry> {
  if commands.is_empty() {
    return Vec::new();
  }
  let filtered = !typed.is_empty();
  let end = if filtered {
    Some(count(ctx, commands.len(), props.faces.len()))
  } else {
    props.note.clone()
  };
  let mut content = vec![Entry::from(heading(ctx, ("commands", "composer.slash.commands"), end))];
  content.extend(commands.iter().enumerate().map(|(at, (face, letters))| {
    Entry::from(row(
      ctx,
      (props.id, &props.field),
      (face, letters.clone(), filtered),
      focused == Some(at),
    ))
  }));
  content
}

/// `panel` with a line after the commands, *Skills* and the skills that match `typed`, the focus at `focused` among
/// them, in a section that scrolls after 8 rows; or why they could not be read. Nothing when no skill matches, or none
/// is published.
fn skills_section(
  ctx: &Ctx,
  props: &Props,
  (typed, section): (&str, SkillSection<'_>),
  focused: Option<usize>,
  panel: MenuPanel,
) -> MenuPanel {
  let filtered = !typed.is_empty();
  let skills = section.matched;
  let end = match section.unreadable {
    Some(why) => why,
    None if skills.is_empty() => return panel,
    None if filtered => count(ctx, skills.len(), section.total),
    None => ctx.t_args("composer.slash.published", [("count", section.total.to_string())]),
  };
  let shown_commands = props.faces.iter().any(|face| face_matched(face, typed).is_some());
  let panel = if shown_commands {
    panel.child(separator())
  } else {
    panel
  };
  let panel = panel.child(heading(ctx, ("skills", "composer.slash.skills"), Some(end)));
  if skills.is_empty() {
    return panel;
  }
  let rows = skills.iter().enumerate().map(|(at, (face, letters))| {
    row(
      ctx,
      (props.id, &props.field),
      (face, letters.clone(), filtered),
      focused == Some(at),
    )
  });
  let rows_high = f32::from(SKILL_ROWS) * ControlHeight::Md.value();
  let gaps = f32::from(SKILL_ROWS - 1) * ExtraSpacing::Tight.value();
  panel.scrolling(rows, (rows_high + gaps, Some(ExtraSpacing::Tight)))
}

/// *2 of 4*: how many of a section's rows match.
fn count(ctx: &Ctx, shown: usize, total: usize) -> Arc<str> {
  ctx.t_args(
    "composer.slash.count",
    [("shown", shown.to_string()), ("total", total.to_string())],
  )
}
