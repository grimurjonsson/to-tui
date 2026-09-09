//! Action response and callback types for interactive plugin flows.
//!
//! Plugins drive multi-step interactive flows by returning `FfiActionResponse`
//! from `invoke_action` / `on_callback` / `on_event`. The host renders the
//! requested UI (picker, confirm modal, …) and feeds the user's reply back
//! through `on_callback`.

use abi_stable::StableAbi;
use abi_stable::std_types::{ROption, RString, RVec};

use crate::host_api::FfiCommand;
use crate::types::FfiTodoItem;

#[repr(C)]
#[derive(StableAbi, Clone, Debug)]
pub struct FfiPickerItem {
    /// Opaque id returned to the plugin in the callback.
    pub id: RString,
    /// Primary line shown to the user.
    pub label: RString,
    /// Optional secondary line (e.g. status, date).
    pub detail: ROption<RString>,
}

#[repr(C)]
#[derive(StableAbi, Clone, Debug)]
pub enum FfiCallbackResult {
    /// User selected an item from a picker.
    PickerSelected { item_id: RString },
    /// User cancelled a picker (Esc).
    PickerCancelled,
    /// User confirmed (Y).
    ConfirmYes,
    /// User declined (N or Esc).
    ConfirmNo,
}

#[repr(C)]
#[derive(StableAbi, Clone, Debug)]
pub enum FfiActionResponse {
    OpenKanban,
    /// Final todos to be shown in Preview and committed on accept.
    Todos {
        items: RVec<FfiTodoItem>,
    },
    /// Open a picker; host calls back via `on_callback(callback_token, ...)`.
    Picker {
        title: RString,
        items: RVec<FfiPickerItem>,
        callback_token: RString,
    },
    /// Open a confirm modal; host calls back via `on_callback(callback_token, ...)`.
    Confirm {
        message: RString,
        callback_token: RString,
    },
    /// Apply commands directly, no further UI.
    Commands {
        commands: RVec<FfiCommand>,
    },
    /// Apply commands and show a status-bar toast.
    Status {
        message: RString,
        commands: RVec<FfiCommand>,
    },
    /// Surface an error in the existing error modal.
    Error {
        message: RString,
    },
}

impl FfiActionResponse {
    /// Convenience: an empty no-op response (zero commands, no UI).
    pub fn noop() -> Self {
        FfiActionResponse::Commands {
            commands: RVec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noop_is_empty_commands() {
        match FfiActionResponse::noop() {
            FfiActionResponse::Commands { commands } => assert!(commands.is_empty()),
            other => panic!("expected Commands, got {:?}", other),
        }
    }

    #[test]
    fn picker_item_carries_all_fields() {
        let item = FfiPickerItem {
            id: "PROJ-1".into(),
            label: "PROJ-1  Some summary".into(),
            detail: ROption::RSome("To Do • 2026-05-11".into()),
        };
        assert_eq!(item.id.as_str(), "PROJ-1");
        assert_eq!(item.label.as_str(), "PROJ-1  Some summary");
        assert!(matches!(item.detail, ROption::RSome(_)));
    }

    #[test]
    fn callback_result_picker_selected_carries_item_id() {
        let cb = FfiCallbackResult::PickerSelected {
            item_id: "x".into(),
        };
        match cb {
            FfiCallbackResult::PickerSelected { item_id } => assert_eq!(item_id.as_str(), "x"),
            other => panic!("unexpected variant: {:?}", other),
        }
    }

    #[test]
    fn action_response_picker_carries_callback_token() {
        let resp = FfiActionResponse::Picker {
            title: "Pick one".into(),
            items: RVec::new(),
            callback_token: "tok-123".into(),
        };
        match resp {
            FfiActionResponse::Picker { callback_token, .. } => {
                assert_eq!(callback_token.as_str(), "tok-123");
            }
            other => panic!("unexpected variant: {:?}", other),
        }
    }
}
