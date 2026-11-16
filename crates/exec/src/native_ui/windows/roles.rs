//! The role a snapshot gives an element: its UI Automation control type, by the name UI Automation gives it, in lower
//! case (`UIA_ButtonControlTypeId` is `button`).

/// The control types from `UIA_ButtonControlTypeId` (50000) on, in their order.
const CONTROL_TYPES: [&str; 41] = [
  "button",
  "calendar",
  "checkbox",
  "combobox",
  "edit",
  "hyperlink",
  "image",
  "listitem",
  "list",
  "menu",
  "menubar",
  "menuitem",
  "progressbar",
  "radiobutton",
  "scrollbar",
  "slider",
  "spinner",
  "statusbar",
  "tab",
  "tabitem",
  "text",
  "toolbar",
  "tooltip",
  "tree",
  "treeitem",
  "custom",
  "group",
  "thumb",
  "datagrid",
  "dataitem",
  "document",
  "splitbutton",
  "window",
  "pane",
  "header",
  "headeritem",
  "table",
  "titlebar",
  "separator",
  "semanticzoom",
  "appbar",
];

/// The first control type id.
const FIRST: i32 = 50_000;

/// The role of control type `id`; `custom` for one UI Automation added after these.
pub(super) fn of(id: i32) -> &'static str {
  id.checked_sub(FIRST)
    .and_then(|index| usize::try_from(index).ok())
    .and_then(|index| CONTROL_TYPES.get(index))
    .copied()
    .unwrap_or("custom")
}
