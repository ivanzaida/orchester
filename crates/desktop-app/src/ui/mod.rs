//! The app's UI kit (Kontur's shared components in `data/orchester2.pen`): controls, labels and blocks
//! every screen builds from, styled only through theme roles and worded only through i18n keys.
//! Clickable surfaces are pointer-only unless they are form controls (`pressable`); anything that cannot
//! be used stays in place, visibly unavailable with a reason (`unavailable`).

pub mod avatar;
pub mod breadcrumb;
pub mod button;
pub mod card;
pub mod chip;
pub mod choice_field;
pub mod code_box;
pub mod detail_row;
pub mod field;
pub mod form_section;
pub mod icon;
pub mod icon_button;
pub mod inspector;
pub mod list_entry;
pub mod loader;
pub mod markdown;
pub mod menu;
pub mod menu_button;
pub mod message;
pub mod meter;
pub mod nav_item;
pub mod notice;
pub mod option_card;
pub mod popover;
pub mod pressable;
pub mod state;
pub mod status;
pub mod table;
pub mod tabs;
pub mod toggle;
pub mod tool_row;
pub mod tooltip;
pub mod unavailable;
