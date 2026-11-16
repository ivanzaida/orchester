//! The platform-independent parts of the native UI driver's macOS side: how AX attributes become a snapshot element,
//! how an action is planned through AX actions only (and when it is unsupported), and the permission state. They run
//! on every platform; the AX, window server and ScreenCaptureKit calls themselves need a Mac.

use orchester_core::orchester_tools::ui::UiAction;
use orchester_exec::native_ui::{
  NativePermission, NativePermissions, NativeUiError,
  accessibility::{self, AxFacts, AxPlan, AxValue},
};

fn element(role: &str) -> AxFacts {
  AxFacts {
    reference: accessibility::reference(0x2a),
    role: role.to_owned(),
    ..AxFacts::default()
  }
}

fn with_actions(role: &str, actions: &[&str]) -> AxFacts {
  AxFacts {
    actions: actions.iter().map(|action| (*action).to_owned()).collect(),
    ..element(role)
  }
}

#[test]
fn ax_roles_are_shown_in_the_windows_drivers_words_and_others_without_their_prefix() {
  assert_eq!(accessibility::role("AXButton"), "button");
  assert_eq!(accessibility::role("AXTextField"), "edit");
  assert_eq!(accessibility::role("AXPopUpButton"), "combobox");
  assert_eq!(accessibility::role("AXStaticText"), "text");
  assert_eq!(accessibility::role("AXValueIndicator"), "valueindicator");
  assert_eq!(accessibility::role("custom"), "custom");
}

#[test]
fn a_ref_is_minted_from_the_elements_hash() {
  assert_eq!(accessibility::reference(0x2a), "x2a");
  assert_eq!(accessibility::reference(u64::MAX), "xffffffffffffffff");
}

#[test]
fn an_element_is_named_by_its_title_then_its_description_then_a_labels_text() {
  let titled = AxFacts {
    title: Some("Save".to_owned()),
    description: Some("save button".to_owned()),
    ..element("AXButton")
  };
  assert_eq!(titled.name().as_deref(), Some("Save"));
  let described = AxFacts {
    title: Some(String::new()),
    description: Some("Close".to_owned()),
    ..element("AXButton")
  };
  assert_eq!(described.name().as_deref(), Some("Close"));
  let label = AxFacts {
    value: Some(AxValue::Text("Ready".to_owned())),
    ..element("AXStaticText")
  };
  let shown = label.element();
  assert_eq!(shown.name.as_deref(), Some("Ready"));
  assert_eq!(shown.value, None, "a label's text is its name, not a value too");
  assert_eq!(shown.role, "text");
}

#[test]
fn states_follow_the_attributes() {
  let field = AxFacts {
    enabled: Some(false),
    focused: Some(true),
    value: Some(AxValue::Text("hello".to_owned())),
    value_settable: false,
    ..element("AXTextField")
  };
  let shown = field.element();
  assert_eq!(shown.states, ["disabled", "focused", "readonly"]);
  assert_eq!(shown.value.as_deref(), Some("hello"));
  let row = AxFacts {
    selected: Some(true),
    expanded: Some(false),
    ..element("AXRow")
  };
  assert_eq!(row.states(), ["collapsed", "selected"]);
}

#[test]
fn a_check_boxs_value_is_its_checked_state() {
  for (value, state) in [(0.0, "unchecked"), (1.0, "checked"), (2.0, "mixed")] {
    let check = AxFacts {
      value: Some(AxValue::Number(value)),
      ..element("AXCheckBox")
    };
    let shown = check.element();
    assert_eq!(shown.states, [state]);
    assert_eq!(shown.value, None);
  }
}

#[test]
fn a_password_fields_text_is_never_shown() {
  let password = AxFacts {
    subrole: Some("AXSecureTextField".to_owned()),
    value: Some(AxValue::Text("secret".to_owned())),
    value_settable: true,
    ..element("AXTextField")
  };
  let shown = password.element();
  assert_eq!(shown.value, None);
  assert_eq!(shown.states, ["password"]);
  assert!(!format!("{shown:?}").contains("secret"));
}

#[test]
fn a_number_value_is_shown_as_its_number() {
  let slider = AxFacts {
    value: Some(AxValue::Number(0.5)),
    ..element("AXSlider")
  };
  assert_eq!(slider.element().value.as_deref(), Some("0.5"));
  let stepper = AxFacts {
    value: Some(AxValue::Number(3.0)),
    ..element("AXIncrementor")
  };
  assert_eq!(stepper.element().value.as_deref(), Some("3"));
}

#[test]
fn a_query_matches_the_name_identifier_or_ax_role_in_any_case() {
  let button = AxFacts {
    title: Some("Save Draft".to_owned()),
    identifier: Some("saveButton".to_owned()),
    ..element("AXButton")
  };
  assert!(button.matches("draft"));
  assert!(button.matches("savebutton"));
  assert!(button.matches("axbutton"));
  assert!(!button.matches("cancel"));
}

#[test]
fn click_presses_else_picks() {
  let button = with_actions("AXButton", &["AXShowMenu", "AXPress"]);
  assert_eq!(
    accessibility::plan(&button, &UiAction::Click),
    Ok(AxPlan::Perform("AXPress"))
  );
  let item = with_actions("AXMenuItem", &["AXPick"]);
  assert_eq!(
    accessibility::plan(&item, &UiAction::Click),
    Ok(AxPlan::Perform("AXPick"))
  );
}

#[test]
fn an_element_without_the_action_answers_unsupported() {
  let label = with_actions("AXStaticText", &["AXShowMenu"]);
  let refused = accessibility::plan(&label, &UiAction::Click);
  assert_eq!(
    refused,
    Err(NativeUiError::Unsupported {
      action: "click",
      needs: "the AXPress or AXPick action",
      reference: "x2a".to_owned(),
      role: "text".to_owned(),
    })
  );
  let text = refused.err().map(|error| error.to_string()).unwrap_or_default();
  assert!(
    text.starts_with("unsupported for this element: click needs the AXPress or AXPick action"),
    "{text}"
  );
  assert!(matches!(
    accessibility::plan(&label, &UiAction::Scroll),
    Err(NativeUiError::Unsupported { action: "scroll", .. })
  ));
  assert!(matches!(
    accessibility::plan(&element("AXImage"), &UiAction::Type("x".to_owned())),
    Err(NativeUiError::Unsupported { action: "type", .. })
  ));
  assert!(matches!(
    accessibility::plan(&element("AXImage"), &UiAction::Select("x".to_owned())),
    Ok(AxPlan::Pick { otherwise, .. }) if matches!(*otherwise, Err(NativeUiError::Unsupported { action: "select", .. }))
  ));
}

#[test]
fn keys_are_never_pressed() {
  let field = with_actions("AXTextField", &["AXPress"]);
  assert_eq!(
    accessibility::plan(&field, &UiAction::Key("Enter".to_owned())),
    Err(NativeUiError::NoKeys)
  );
}

#[test]
fn type_sets_a_settable_value_and_refuses_a_read_only_one() {
  let field = AxFacts {
    value: Some(AxValue::Text(String::new())),
    value_settable: true,
    ..element("AXTextField")
  };
  assert_eq!(
    accessibility::plan(&field, &UiAction::Type("hello".to_owned())),
    Ok(AxPlan::SetValue(AxValue::Text("hello".to_owned())))
  );
  let fixed = AxFacts {
    value_settable: false,
    ..field
  };
  assert_eq!(
    accessibility::plan(&fixed, &UiAction::Type("hello".to_owned())),
    Err(NativeUiError::ReadOnly("x2a".to_owned()))
  );
}

#[test]
fn scroll_uses_scroll_to_visible() {
  let row = with_actions("AXRow", &["AXScrollToVisible"]);
  assert_eq!(
    accessibility::plan(&row, &UiAction::Scroll),
    Ok(AxPlan::Perform("AXScrollToVisible"))
  );
}

#[test]
fn select_picks_an_option_opening_a_pop_up_first() {
  let pop_up = with_actions("AXPopUpButton", &["AXPress", "AXShowMenu"]);
  let Ok(AxPlan::Pick {
    option,
    open,
    otherwise,
  }) = accessibility::plan(&pop_up, &UiAction::Select("Large".to_owned()))
  else {
    panic!("a select is a pick first");
  };
  assert_eq!(option, "Large");
  assert_eq!(open, Some("AXShowMenu"));
  assert_eq!(
    *otherwise,
    Err(NativeUiError::NoOption {
      reference: "x2a".to_owned(),
      option: "Large".to_owned(),
    })
  );
}

#[test]
fn select_sets_or_steps_a_number() {
  let slider = AxFacts {
    value: Some(AxValue::Number(0.0)),
    value_settable: true,
    ..element("AXSlider")
  };
  let otherwise = |facts: &AxFacts, option: &str| match accessibility::plan(facts, &UiAction::Select(option.to_owned()))
  {
    Ok(AxPlan::Pick { otherwise, .. }) => *otherwise,
    other => panic!("a select is a pick first: {other:?}"),
  };
  assert_eq!(otherwise(&slider, "0.75"), Ok(AxPlan::SetValue(AxValue::Number(0.75))));
  assert!(matches!(
    otherwise(&slider, "loud"),
    Err(NativeUiError::NoOption { .. })
  ));
  let stepper = AxFacts {
    value: Some(AxValue::Number(1.0)),
    actions: vec!["AXIncrement".to_owned(), "AXDecrement".to_owned()],
    ..element("AXIncrementor")
  };
  assert_eq!(otherwise(&stepper, "4"), Ok(AxPlan::StepTo(4.0)));
  let fixed = AxFacts {
    actions: Vec::new(),
    ..stepper
  };
  assert!(matches!(
    otherwise(&fixed, "4"),
    Err(NativeUiError::Unsupported { action: "select", .. })
  ));
}

#[test]
fn permissions_name_what_is_missing_and_where_to_grant_it() {
  let granted = NativePermissions {
    accessibility: true,
    screen_recording: true,
  };
  assert_eq!(granted.missing(), []);
  assert_eq!(granted.require(), Ok(()));
  let none = NativePermissions {
    accessibility: false,
    screen_recording: false,
  };
  assert_eq!(
    none.missing(),
    [NativePermission::Accessibility, NativePermission::ScreenRecording]
  );
  let only_capture = NativePermissions {
    accessibility: true,
    screen_recording: false,
  };
  assert!(!only_capture.granted(NativePermission::ScreenRecording));
  let refused = only_capture.require();
  assert_eq!(
    refused,
    Err(NativeUiError::PermissionsMissing(vec![
      NativePermission::ScreenRecording
    ]))
  );
  let text = refused.err().map(|error| error.to_string()).unwrap_or_default();
  assert!(text.starts_with("permission missing:"), "{text}");
  assert!(text.contains("the Screen Recording permission,"), "{text}");
  assert!(
    text.contains("System Settings › Privacy & Security › Screen Recording"),
    "{text}"
  );
  assert!(!text.contains("Accessibility"), "{text}");
  let both = none.require().err().map(|error| error.to_string()).unwrap_or_default();
  assert!(
    both.contains("the Accessibility and Screen Recording permissions"),
    "{both}"
  );
}

#[test]
fn each_permission_links_to_its_pane_of_system_settings() {
  assert_eq!(
    NativePermission::Accessibility.settings_url(),
    "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
  );
  assert_eq!(
    NativePermission::ScreenRecording.settings_url(),
    "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture"
  );
}

#[cfg(windows)]
#[test]
fn windows_needs_no_permission() {
  assert_eq!(orchester_exec::native_ui::permissions(), None);
  assert_eq!(orchester_exec::native_ui::ready(), Ok(()));
}
