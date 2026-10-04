//! Design variables from `data/orchester.pen` (Pencil `get_variables`), plus values that
//! the Foundations components use directly. Visible only inside the theme module:
//! UI code reads lurq theme roles, never these constants.

pub const CANVAS: &str = "#101010";
pub const SIDEBAR: &str = "#141414";
pub const SURFACE: &str = "#1A1A1A";
pub const RAISED: &str = "#222222";
pub const HOVER: &str = "#2C2C2C";
pub const BORDER: &str = "#363636";
pub const HAIRLINE: &str = "#2B2B2B";
pub const TEXT: &str = "#EEEEEE";
pub const MUTED: &str = "#A3A3A3";
pub const SUBTLE: &str = "#8F8F8F";
pub const ACCENT: &str = "#E6E6E6";
pub const INVERSE_TEXT: &str = "#181818";
/// The one status colour: the *Unreadable* status label's icon and word (Pencil `danger`, `mGbYs`).
pub const DANGER: &str = "#F87171";
/// Backdrop behind the close dialog (`a8U756`).
pub const SCRIM: &str = "#000000A6";
/// `canvas` fully transparent: where the fade over a clipped edge of the tab strip ends (`xpSC0`).
pub const CANVAS_CLEAR: &str = "#10101000";
/// The macOS traffic lights in the platform's standard colors (`lenEp`, `nsw7u`, `Vw8Pc` in
/// `V70DN`), the only non-grayscale colors of the chrome.
pub const TRAFFIC_CLOSE: &str = "#FF5F57";
pub const TRAFFIC_MINIMIZE: &str = "#FEBC2E";
pub const TRAFFIC_ZOOM: &str = "#28C840";

/// Code and diff colors (Pencil `code-*` and `diff-*`, used by the diff line `vEZ42`): a
/// restrained GitHub Dark palette for code only; the application chrome stays grayscale.
pub const CODE_BG: &str = "#0D1117";
pub const CODE_TEXT: &str = "#E6EDF3";
pub const CODE_KEYWORD: &str = "#FF7B72";
pub const CODE_STRING: &str = "#A5D6FF";
pub const CODE_FUNCTION: &str = "#D2A8FF";
pub const CODE_VARIABLE: &str = "#FFA657";
pub const CODE_CONSTANT: &str = "#79C0FF";
pub const CODE_COMMENT: &str = "#8B949E";
pub const DIFF_ADD_BG: &str = "#14261C";
pub const DIFF_ADD_TEXT: &str = "#7EE787";
pub const DIFF_REMOVE_BG: &str = "#301C20";
pub const DIFF_REMOVE_TEXT: &str = "#FFA198";

pub const FONT: &str = "Inter";
/// Pencil's `mono`, bundled from `assets/fonts/GeistMono-Regular.ttf` (SIL OFL 1.1).
pub const MONO: &str = "Geist Mono";

pub const RADIUS_CONTROL: f32 = 7.0;
pub const RADIUS_PANEL: f32 = 12.0;
/// Navigation item corner radius (`TSJpn`).
pub const RADIUS_ITEM: f32 = 6.0;

pub const SPACE_SM: f32 = 8.0;
pub const SPACE_MD: f32 = 16.0;
pub const SPACE_LG: f32 = 24.0;

/// Field and button heights and horizontal padding (`ZGwlv`, `u1XBP`).
pub const FIELD_HEIGHT: f32 = 40.0;
pub const BUTTON_HEIGHT: f32 = 36.0;
pub const CONTROL_PADDING_X: f32 = 12.0;
/// Focused field stroke (`o8Oug`).
pub const FOCUS_STROKE: f32 = 1.5;
/// Underline of the selected tab (`Iefue`, `Okyxt`).
pub const TAB_INDICATOR_STROKE: f32 = 2.0;
/// One digit of a count in `TypographyStyle::Mono` (Geist Mono 10, whose digits all advance
/// 0.6 em), e.g. the room the Open tab keeps for its count (`tidLB` in `i59Wy`, `DF8II`).
pub const COUNT_DIGIT_WIDTH: f32 = 6.0;
/// Chevron of a select field and check of its selected option (`eASPF` in `ZGwlv`, `ClxwV` in
/// `L20XvS`); also the width of the check slot every option reserves.
pub const SELECT_ICON_SIZE: f32 = 14.0;
/// Lucide icon font, the icon library of every Pencil icon node.
pub const ICON_FONT: &str = "lucide";

/// Corner radius of inline notices and the runner status card (`q5LzI`, `O2Jjjf`).
pub const RADIUS_NOTICE: f32 = 8.0;
/// Corner radius of the bars in the Orchester mark (`DIMZ6`).
pub const RADIUS_MARK: f32 = 2.0;
/// Corner radius of a title bar tab's avatar and close button (`b9QQb`, `OvWLh` in `nAKyb`).
pub const RADIUS_TAB_CONTROL: f32 = 4.0;
/// Corner radius of the *Open in new tab* button on a project switcher row (`NcJQ0`).
pub const RADIUS_ROW_ACTION: f32 = 5.0;
/// Corner radius of a category chip (`EfQuh`).
pub const RADIUS_CHIP: f32 = 5.0;
/// Corner radius of a segment of the conversation's *Show* control, in the usage dashboard's period
/// style (toolbar of `RrScB`).
pub const RADIUS_SEGMENT: f32 = 5.0;
/// Corner radius of an open select menu: option radius 7 plus its padding 4 (`u0ByM`).
pub const RADIUS_SELECT_MENU: f32 = 11.0;
/// Switch track (`K8ItM`, radius 8 on a 16px track). lurq clamps a radius to half the box, so
/// the 10px knob is round with it too.
pub const RADIUS_PILL: f32 = 8.0;

/// Gaps and paddings used by the Getting started screens, the sidebar and the overview.
/// Pencil node ids are listed on the `ExtraSpacing` variants that use them.
pub const SPACE_HAIR: f32 = 2.0;
pub const SPACE_MARK: f32 = 3.0;
pub const SPACE_TIGHT: f32 = 6.0;
pub const SPACE_INDICATOR: f32 = 7.0;
pub const SPACE_INLINE: f32 = 10.0;
pub const SPACE_STACK: f32 = 12.0;
pub const SPACE_GROUP: f32 = 14.0;
pub const SPACE_STEP: f32 = 20.0;
pub const SPACE_RAIL: f32 = 28.0;
pub const SPACE_HEADING_TOP: f32 = 26.0;
pub const SPACE_HEADING_BOTTOM: f32 = 22.0;
pub const SPACE_FORM: f32 = 22.0;
pub const SPACE_SECTION: f32 = 32.0;
pub const SPACE_INDENT: f32 = 40.0;
pub const SPACE_PAGE: f32 = 48.0;
pub const SPACE_HERO: f32 = 64.0;
pub const SPACE_FOOTER: f32 = 72.0;
pub const SPACE_COLUMNS: f32 = 112.0;
pub const SPACE_MARGIN: f32 = 120.0;
pub const SPACE_FIELD: f32 = 9.0;
pub const SPACE_PROPERTY: f32 = 15.0;
pub const SPACE_DIVIDER: f32 = 18.0;
pub const SPACE_SELECT_MENU: f32 = 4.0;
/// Above and below a transcript command's lines in its viewport (`Z3M6Nz` in `uA3Sh`).
pub const SPACE_COMMAND_LINES: f32 = 4.0;
pub const SPACE_DIALOG_MARGIN: f32 = 32.0;
/// Padding of a note field (`NbNN0`, the changes a plan needs).
pub const SPACE_TEXT_AREA: f32 = 14.0;
pub const SPACE_BESIDE_GUTTER: f32 = 20.0;
/// A check row's detail under its title: the row's icon and the gap after it (`gzztk` in 02.Q
/// `LMrbK`).
pub const SPACE_CHECK_INDENT: f32 = 28.0;
/// Why an option cannot be chosen, under it: indented past a compact radio and its gap (`J5RohL`,
/// `L1B0m` in 04.B `W0Owo`).
pub const SPACE_REASON_INDENT: f32 = 22.0;
/// A tool group and a card's body in the conversation timeline start where a message's text does:
/// past its 24px glyph and the 12px gap (`uoK7y`, the cards of `ZxYGh`, `haIEI`).
pub const SPACE_TIMELINE_INDENT: f32 = 36.0;

/// Stroke of the pipeline canvas connectors (`zpizf`, `gXNw9`).
pub const CONNECTOR_STROKE: f32 = 1.25;

/// Custom window title bar height (`QQmsI`, `V70DN`).
pub const TITLE_BAR_HEIGHT: f32 = 32.0;
/// Window status bar height (`pmOpt`).
pub const STATUS_BAR_HEIGHT: f32 = 28.0;
/// Width of each Windows window control (`QQmsI`).
pub const WINDOW_CONTROL_WIDTH: f32 = 46.0;
/// Width of the lifecycle dialogs at most: close with active work, waiting and start-up
/// recovery (`um9B1`, `KS2RU`, `BovYZ`).
pub const LIFECYCLE_DIALOG_WIDTH: f32 = 880.0;
/// The tallest a long value's box in the approval dialog grows before its lines scroll inside it:
/// 12 lines with the box's padding (`E5NBo` in `bjrgu`, "Approval choices"). The full shell string
/// of 04.H9 (`bwWuT` in `coKjY`) shares it.
pub const APPROVAL_VALUE_MAX_HEIGHT: f32 = 232.0;

/// Drop shadow of the status bar operations popover (`E1msJS`): outer, x 0, y 8, blur 24,
/// no spread. The only shadow in the design.
pub const POPOVER_SHADOW_Y: f32 = 8.0;
pub const POPOVER_SHADOW_BLUR: f32 = 24.0;
pub const POPOVER_SHADOW_COLOR: &str = "#00000066";

/// Focus ring (`xURvJ`, anatomy `Oz9EN`): a 2px light ring behind a 2px dark gap, drawn as
/// two spread box shadows (Pencil `focus-ring`, `focus-gap`, `focus-width`, `focus-extent`).
pub const FOCUS_RING: &str = "#EEEEEE";
pub const FOCUS_GAP: &str = "#101010";
pub const FOCUS_WIDTH: f32 = 2.0;
pub const FOCUS_EXTENT: f32 = 4.0;

/// Scroll thumb of a long select menu (`g3poI`) and of a scroll gutter (`nf9DG` in `JuVaA`): 4px
/// wide in `border`, radius 2, at least 24px long, no track.
pub const SCROLLBAR_WIDTH: f32 = 4.0;
pub const SCROLLBAR_RADIUS: f32 = 2.0;
pub const SCROLLBAR_MIN_THUMB: f32 = 24.0;
pub const NO_TRACK: &str = "#00000000";
/// Select menu thumb (`YD8PV`): 3px from the menu's outer right edge and 11px (the corner radius)
/// from its top and bottom, so it never enters a rounded corner.
pub const SELECT_SCROLLBAR_EDGE_INSET: f32 = 3.0;
pub const SELECT_SCROLLBAR_END_INSET: f32 = 11.0;
/// Scroll gutter thumb (`nf9DG` in `NwsEg`): centred in the 12px gutter, 4px from either side,
/// and 4px from the viewport's top and bottom.
pub const GUTTER_SCROLLBAR_INSET: f32 = 4.0;
/// Horizontal thumb of a transcript command (`hpzPu` in `G9skx`): centred in the 6px band at the
/// bottom of the box, and starting where the text does, 12px in.
pub const COMMAND_SCROLLBAR_EDGE_INSET: f32 = 1.0;
pub const COMMAND_SCROLLBAR_END_INSET: f32 = 12.0;
/// Thumbs of a capped value box in the approval dialog (`e0vyng` in `bjrgu`): from the very start
/// of the viewport, which the box's 6px padding already holds off its border.
pub const VALUE_BOX_SCROLLBAR_END_INSET: f32 = 0.0;

/// Unparsed CLI lines in the agent transcript are muted to 60 % (`Ane9j` in `SfXU1`).
pub const UNPARSED_OPACITY: f32 = 0.6;
/// A permission option the connection cannot enforce, or one that is not available yet, at 50 %
/// (`BNliD` in 04.04 `T1rjv`; the compact 04.B `W0Owo` shows 45 %).
pub const UNAVAILABLE_OPTION_OPACITY: f32 = 0.5;
/// A button that cannot be pressed keeps its variant's look at 40 %, primary and secondary alike
/// (`kLJJb` Button / Disabled state, `HN7cO`, `W4y2y` and `WuZqY` in `angoF`).
pub const DISABLED_OPACITY: f32 = 0.4;
/// A switch whose value is fixed fades as one row, its label with it, to 45 % (`lW7Sp` in 04.B
/// `W0Owo`).
pub const LOCKED_SWITCH_OPACITY: f32 = 0.45;

/// Scheduling review (02.05 `W1Dq6`, 02.06 `IyJ5a`): a task in the order (`gj5Cf`, a `zemhy` row).
pub const TASK_REVIEW_ROW_HEIGHT: f32 = 50.0;
/// The 508px execution settings (`qdApo`) plus their reserved scroll gutter, which sits in the
/// page padding.
pub const TASK_REVIEW_RAIL_WIDTH: f32 = 508.0 + SPACE_STACK;
/// Execution-mode cards (`YZcH2`, `jPX6u`).
pub const TASK_REVIEW_MODE_HEIGHT: f32 = 88.0;
/// The settings' selects and token fields (`gIQBb`, `MUKob`, `dMiX2`, `XZCQ5`).
pub const TASK_REVIEW_FIELD_HEIGHT: f32 = 38.0;
/// The drag handle tile of an order row (`aqRNL` in `gj5Cf`).
pub const TASK_REVIEW_HANDLE_SIZE: f32 = 32.0;
/// The action strip under the review (`JV6sD`).
pub const TASK_REVIEW_FOOTER_HEIGHT: f32 = 76.0;
/// Queue N tasks, the strip's primary action (`QAYeP`).
pub const TASK_REVIEW_ACTION_HEIGHT: f32 = 40.0;
pub const TASK_REVIEW_QUEUE_WIDTH: f32 = 154.0;
/// A key/value setting: time limit and pipeline version (`U673I`, `RG0FF`, `tfGRt`).
pub const TASK_REVIEW_PROPERTY_HEIGHT: f32 = 22.0;
/// Back to tasks in the review's topbar (`uBqK9`).
pub const TASK_REVIEW_BACK_WIDTH: f32 = 132.0;
/// The narrowest window that holds the review's two columns: the 216 sidebar, 32 page padding, an
/// order column of 440 (its rows and its Move and Undo buttons), the 32 gap, the 520 settings with
/// their gutter and 20 padding. Narrower, the settings go under the order (not drawn yet).
pub const TASK_REVIEW_COLUMNS_MIN_WINDOW: f32 = 1260.0;
/// The narrowest window that holds the retry limit's evidence beside its 360 decision panel
/// (`Q4LnK`, `Bi6pU` in 02.B3): the 216 sidebar, the evidence's 64 padding, a check row of 560 (its
/// step and agent, and "Reported failed · attempt 3 · 22k tokens · 3 min") and the panel.
/// Narrower, the panel goes under the evidence and the page scrolls (not drawn yet).
pub const RETRY_LIMIT_COLUMNS_MIN_WINDOW: f32 = 1200.0;
/// The narrowest window whose Conversation toolbar (`JKl4s` in `RrScB`) holds everything on one line
/// with Details closed: the 216 sidebar, the toolbar's 64 padding, the 240 step filter, *Show* (164),
/// the 180 search, the live state (*Following live*, 99), *Details* (81) and five 12 gaps.
/// Narrower, *Show* and the search go to a second line (not drawn yet).
pub const CONVERSATION_TOOLBAR_ONE_LINE_MIN_WINDOW: f32 = 1104.0;
/// The narrowest window, less the Details pane when it is open, whose composer (`C3nu0z`) holds its
/// recipient, *Attach* and both send buttons on one line: the 216 sidebar, the column's 64 padding,
/// the composer's 24, the 260 recipient, *Attach* (104), *Send after this turn* (166), *Send now*
/// (104) and three 10 gaps. Narrower, the buttons go to a line of their own (not drawn yet).
pub const CONVERSATION_COMPOSER_ONE_LINE_MIN_WINDOW: f32 = 980.0;
/// What the Tools view adds to that line: *Show raw lines* (`q4fCXK`) and its gap.
pub const CONVERSATION_TOOLBAR_RAW_SWITCH: f32 = 140.0;
/// How much wider *Waits for your decision* is than *Following live*.
pub const CONVERSATION_TOOLBAR_DECISION_STATE: f32 = 50.0;
/// The task table (`clVjO`, `qVMJh`) drops columns as the window narrows. Its fixed cells and gaps
/// take 392 plus Workflow / run (212, or 164 without Priority) of the window less its 216 sidebar
/// and 64 page padding; Pipeline adds 152 and Priority 112. Pipeline stays while the title keeps
/// 200 beside every column; the tag chips under the titles while it keeps 200 without Pipeline;
/// Priority while it keeps 120. Narrower, the title still keeps 120 down to a 956 window.
pub const TASK_TABLE_PIPELINE_MIN_WINDOW: f32 = 1348.0;
pub const TASK_TABLE_TAGS_MIN_WINDOW: f32 = 1196.0;
pub const TASK_TABLE_PRIORITY_MIN_WINDOW: f32 = 1116.0;
/// The narrowest window whose Overview keeps the attention column (`NM5My`, 288 and a 32 gap)
/// beside the task list: the 216 sidebar, 64 page padding and a list row's fixed parts (state
/// tile, the longest workflow label, tag chips, time, chevron and their gaps, about 530) leave its
/// title and meta 200. Narrower, the column goes under the list at full width (no frame draws it).
pub const OVERVIEW_ATTENTION_BESIDE_MIN_WINDOW: f32 = 1340.0;
/// Provider section row gap in plan-limit hints (`mFTm5`, `pNIEg`).
pub const PLAN_LIMIT_ROWS_GAP: f32 = 6.0;
