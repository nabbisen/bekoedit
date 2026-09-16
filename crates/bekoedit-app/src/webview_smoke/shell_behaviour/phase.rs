//! The second run's phase table (RFC-044 slice 3 handoff §4.4): every phase,
//! its order, its milestone, and the terminal stage, kept together so they
//! cannot be extended one list at a time.

use crate::webview_smoke::transport::PhaseKind;

pub(super) const EXPECTED_MILESTONES: [&str; 21] = [
    "down_up_moved",
    "expand_entered",
    "collapse_ascended",
    "home_end_reached",
    "non_openable_reachable",
    "enter_opened_editor_focused",
    "search_result_editor_focused",
    "new_file_editor_focused",
    "tree_enter_refocused_after_new_file",
    "form_search_restored_to_trigger",
    "app_menu_mouse_open_kept_focus",
    "app_menu_keys_verified",
    "app_menu_escape_restored",
    "app_menu_focus_leave_kept",
    "tools_menu_keys_verified",
    "tools_menu_escape_restored",
    "tools_menu_focus_leave_kept",
    "tabs_arrows_moved_focus_only",
    "tabs_click_focused_editor",
    "menu_closed_into_editor_kept",
    "authority_released_editor_refocused",
];

/// The phase whose success is the whole run's terminal result.
pub(super) const TERMINAL_STAGE: &str = "authority_released_after_editor_focus";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::webview_smoke) enum ShellBehaviourPhase {
    DownUp,
    ExpandEnter,
    CollapseAscend,
    HomeEnd,
    NonOpenable,
    /// Contract 7 (task 014).
    EnterOpens,
    /// Task 016 §5.2 (a).
    SearchResultOpens,
    /// Task 016 §5.2 (b), first assertion.
    NewFileFocuses,
    /// Task 016 §5.2 (b), second assertion.
    TreeEnterAfterNewFile,
    /// Task 016 re-review §2: a Form-mode search result restores to the
    /// search trigger.
    FormSearchRestores,
    /// Task 017 §2's mouse rows, app menu.
    AppMenuMouseOpen,
    /// Slice 2, RFC-044 §8 B contracts 1-5, app menu.
    AppMenuKeys,
    /// Contract 6, app menu.
    AppMenuEscape,
    /// Contract 7, app menu.
    AppMenuFocusLeave,
    /// Contracts 1-5, editor-tools menu.
    ToolsMenuKeys,
    /// Contract 6, editor-tools menu.
    ToolsMenuEscape,
    /// Contract 7, editor-tools menu.
    ToolsMenuFocusLeave,
    /// Slice 3, RFC-044 §8 C1: arrow keys on the mode tablist move focus only.
    TabsArrowsFocusOnly,
    /// §8 C2: activating the Text tab selects it and focuses the editor.
    TabsClickActivates,
    /// §8 D1: focus entering the editor closes the menu, and stays.
    MenuClosesIntoEditor,
    /// §8 D2: that close released shell authority; the terminal phase.
    AuthorityReleasedAfterEditorFocus,
}

impl ShellBehaviourPhase {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::DownUp => "down_up",
            Self::ExpandEnter => "expand_enter",
            Self::CollapseAscend => "collapse_ascend",
            Self::HomeEnd => "home_end",
            Self::NonOpenable => "non_openable",
            Self::EnterOpens => "enter_opens",
            Self::SearchResultOpens => "search_result_opens",
            Self::NewFileFocuses => "new_file_focuses",
            Self::TreeEnterAfterNewFile => "tree_enter_after_new_file",
            Self::FormSearchRestores => "form_search_restores",
            Self::AppMenuMouseOpen => "app_menu_mouse_open",
            Self::AppMenuKeys => "app_menu_keys",
            Self::AppMenuEscape => "app_menu_escape",
            Self::AppMenuFocusLeave => "app_menu_focus_leave",
            Self::ToolsMenuKeys => "tools_menu_keys",
            Self::ToolsMenuEscape => "tools_menu_escape",
            Self::ToolsMenuFocusLeave => "tools_menu_focus_leave",
            Self::TabsArrowsFocusOnly => "tabs_arrows_focus_only",
            Self::TabsClickActivates => "tabs_click_activates",
            Self::MenuClosesIntoEditor => "menu_closes_into_editor",
            Self::AuthorityReleasedAfterEditorFocus => TERMINAL_STAGE,
        }
    }

    pub(super) const fn next(self) -> Option<Self> {
        match self {
            Self::DownUp => Some(Self::ExpandEnter),
            Self::ExpandEnter => Some(Self::CollapseAscend),
            Self::CollapseAscend => Some(Self::HomeEnd),
            Self::HomeEnd => Some(Self::NonOpenable),
            Self::NonOpenable => Some(Self::EnterOpens),
            Self::EnterOpens => Some(Self::SearchResultOpens),
            Self::SearchResultOpens => Some(Self::NewFileFocuses),
            Self::NewFileFocuses => Some(Self::TreeEnterAfterNewFile),
            Self::TreeEnterAfterNewFile => Some(Self::FormSearchRestores),
            Self::FormSearchRestores => Some(Self::AppMenuMouseOpen),
            Self::AppMenuMouseOpen => Some(Self::AppMenuKeys),
            Self::AppMenuKeys => Some(Self::AppMenuEscape),
            Self::AppMenuEscape => Some(Self::AppMenuFocusLeave),
            Self::AppMenuFocusLeave => Some(Self::ToolsMenuKeys),
            Self::ToolsMenuKeys => Some(Self::ToolsMenuEscape),
            Self::ToolsMenuEscape => Some(Self::ToolsMenuFocusLeave),
            Self::ToolsMenuFocusLeave => Some(Self::TabsArrowsFocusOnly),
            Self::TabsArrowsFocusOnly => Some(Self::TabsClickActivates),
            Self::TabsClickActivates => Some(Self::MenuClosesIntoEditor),
            Self::MenuClosesIntoEditor => Some(Self::AuthorityReleasedAfterEditorFocus),
            Self::AuthorityReleasedAfterEditorFocus => None,
        }
    }

    /// The `milestone` a `Progress` report from this phase must carry --
    /// one-to-one with `EXPECTED_MILESTONES`. The terminal phase reports its
    /// milestone via `DriverResult.milestones`.
    pub(super) const fn expected_milestone(self) -> &'static str {
        match self {
            Self::DownUp => "down_up_moved",
            Self::ExpandEnter => "expand_entered",
            Self::CollapseAscend => "collapse_ascended",
            Self::HomeEnd => "home_end_reached",
            Self::NonOpenable => "non_openable_reachable",
            Self::EnterOpens => "enter_opened_editor_focused",
            Self::SearchResultOpens => "search_result_editor_focused",
            Self::NewFileFocuses => "new_file_editor_focused",
            Self::TreeEnterAfterNewFile => "tree_enter_refocused_after_new_file",
            Self::FormSearchRestores => "form_search_restored_to_trigger",
            Self::AppMenuMouseOpen => "app_menu_mouse_open_kept_focus",
            Self::AppMenuKeys => "app_menu_keys_verified",
            Self::AppMenuEscape => "app_menu_escape_restored",
            Self::AppMenuFocusLeave => "app_menu_focus_leave_kept",
            Self::ToolsMenuKeys => "tools_menu_keys_verified",
            Self::ToolsMenuEscape => "tools_menu_escape_restored",
            Self::ToolsMenuFocusLeave => "tools_menu_focus_leave_kept",
            Self::TabsArrowsFocusOnly => "tabs_arrows_moved_focus_only",
            Self::TabsClickActivates => "tabs_click_focused_editor",
            Self::MenuClosesIntoEditor => "menu_closed_into_editor_kept",
            Self::AuthorityReleasedAfterEditorFocus => "authority_released_editor_refocused",
        }
    }
}

impl PhaseKind for ShellBehaviourPhase {
    fn as_str(self) -> &'static str {
        Self::as_str(self)
    }
}
