use crossterm::event::KeyCode;

use crate::config::{CustomCommandKeybind, KeyCombo, Keybinds};

use super::TerminalKey;

#[derive(Debug, Clone)]
pub(crate) enum KeybindMatch {
    Action(KeybindAction),
    Command(CustomCommandKeybind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeybindAction {
    NewWorkspace,
    NewWorktree,
    OpenWorktree,
    RemoveWorktree,
    RenameWorkspace,
    CloseWorkspace,
    SwitchWorkspace(usize),
    SwitchTab(usize),
    FocusAgent(usize),
    PreviousWorkspace,
    NextWorkspace,
    PreviousAgent,
    NextAgent,
    NewTab,
    RenameTab,
    PreviousTab,
    NextTab,
    MoveTabPrevious,
    MoveTabNext,
    CloseTab,
    RenamePane,
    FocusPaneLeft,
    FocusPaneDown,
    FocusPaneUp,
    FocusPaneRight,
    SwapPaneLeft,
    SwapPaneDown,
    SwapPaneUp,
    SwapPaneRight,
    SplitVertical,
    SplitHorizontal,
    ClosePane,
    EditScrollback,
    ClearPane,
    CopyMode,
    Zoom,
    EnterResizeMode,
    ResizePaneLeft,
    ResizePaneDown,
    ResizePaneUp,
    ResizePaneRight,
    ToggleSidebar,
    CyclePaneNext,
    CyclePanePrevious,
    LastPane,
    Help,
    Settings,
    ReloadConfig,
    OpenNotificationTarget,
    Detach,
    OpenNavigator,
    OpenMakiSessions,
}

pub(crate) fn resolve_binding(keybinds: &Keybinds, key: &TerminalKey) -> Option<KeybindMatch> {
    resolve_exact_binding(keybinds, key).or_else(|| {
        generated_character_key(key)
            .and_then(|generated| resolve_exact_binding(keybinds, &generated))
    })
}

/// Bindings that stay live while a pane receives typed input: modified chords
/// only, so unmodified printable keys keep typing.
pub(crate) fn resolve_modified_binding(
    keybinds: &Keybinds,
    key: &TerminalKey,
) -> Option<KeybindMatch> {
    let matches = |bindings: &crate::config::ActionKeybinds, _key: &TerminalKey| {
        bindings.matches_modified_key(_key)
    };
    non_indexed_action_with(keybinds, key, &matches)
        .map(KeybindMatch::Action)
        .or_else(|| {
            keybinds
                .custom_commands
                .iter()
                .find(|binding| matches(&binding.bindings, key))
                .cloned()
                .map(KeybindMatch::Command)
        })
        .or_else(|| indexed_action_with(keybinds, key, true).map(KeybindMatch::Action))
}

fn resolve_exact_binding(keybinds: &Keybinds, key: &TerminalKey) -> Option<KeybindMatch> {
    non_indexed_action(keybinds, key)
        .map(KeybindMatch::Action)
        .or_else(|| custom_command(keybinds, key).map(KeybindMatch::Command))
        .or_else(|| indexed_action(keybinds, key).map(KeybindMatch::Action))
}

/// NORMAL-mode key overrides, resolved before the action binding table.
pub(crate) fn resolve_vim_normal_action(
    keys: &crate::config::VimNormalKeys,
    key: &TerminalKey,
) -> Option<KeybindAction> {
    let matches = |combo: Option<KeyCombo>, action: KeybindAction| {
        combo
            .is_some_and(|combo| crate::config::terminal_key_matches_combo(key, combo))
            .then_some(action)
    };
    let k = keys;
    [
        (k.focus_left, KeybindAction::FocusPaneLeft),
        (k.focus_down, KeybindAction::FocusPaneDown),
        (k.focus_up, KeybindAction::FocusPaneUp),
        (k.focus_right, KeybindAction::FocusPaneRight),
        (k.previous_tab, KeybindAction::PreviousTab),
        (k.next_tab, KeybindAction::NextTab),
        (k.previous_workspace, KeybindAction::PreviousWorkspace),
        (k.next_workspace, KeybindAction::NextWorkspace),
        (k.new_tab, KeybindAction::NewTab),
        (k.close_pane, KeybindAction::ClosePane),
        (k.split_down, KeybindAction::SplitHorizontal),
        (k.split_right, KeybindAction::SplitVertical),
        (k.visual_mode, KeybindAction::CopyMode),
        (k.zoom, KeybindAction::Zoom),
        (k.help, KeybindAction::Help),
        (k.maki_sessions, KeybindAction::OpenMakiSessions),
        (k.detach, KeybindAction::Detach),
        (k.rename_tab, KeybindAction::RenameTab),
    ]
    .into_iter()
    .find_map(|(combo, action)| matches(combo, action))
}

fn non_indexed_action(keybinds: &Keybinds, key: &TerminalKey) -> Option<KeybindAction> {
    non_indexed_action_with(keybinds, key, &|bindings, key| bindings.matches_key(key))
}

fn non_indexed_action_with(
    keybinds: &Keybinds,
    key: &TerminalKey,
    matches: &impl Fn(&crate::config::ActionKeybinds, &TerminalKey) -> bool,
) -> Option<KeybindAction> {
    for (bindings, action) in [
        (&keybinds.help, KeybindAction::Help),
        (&keybinds.settings, KeybindAction::Settings),
        (&keybinds.new_workspace, KeybindAction::NewWorkspace),
        (&keybinds.new_worktree, KeybindAction::NewWorktree),
        (&keybinds.open_worktree, KeybindAction::OpenWorktree),
        (&keybinds.remove_worktree, KeybindAction::RemoveWorktree),
        (&keybinds.rename_workspace, KeybindAction::RenameWorkspace),
        (&keybinds.close_workspace, KeybindAction::CloseWorkspace),
        (
            &keybinds.previous_workspace,
            KeybindAction::PreviousWorkspace,
        ),
        (&keybinds.next_workspace, KeybindAction::NextWorkspace),
        (&keybinds.previous_agent, KeybindAction::PreviousAgent),
        (&keybinds.next_agent, KeybindAction::NextAgent),
        (&keybinds.new_tab, KeybindAction::NewTab),
        (&keybinds.rename_tab, KeybindAction::RenameTab),
        (&keybinds.previous_tab, KeybindAction::PreviousTab),
        (&keybinds.next_tab, KeybindAction::NextTab),
        (&keybinds.move_tab_previous, KeybindAction::MoveTabPrevious),
        (&keybinds.move_tab_next, KeybindAction::MoveTabNext),
        (&keybinds.close_tab, KeybindAction::CloseTab),
        (&keybinds.rename_pane, KeybindAction::RenamePane),
        (&keybinds.edit_scrollback, KeybindAction::EditScrollback),
        (&keybinds.clear_pane, KeybindAction::ClearPane),
        (&keybinds.copy_mode, KeybindAction::CopyMode),
        (&keybinds.focus_pane_left, KeybindAction::FocusPaneLeft),
        (&keybinds.focus_pane_down, KeybindAction::FocusPaneDown),
        (&keybinds.focus_pane_up, KeybindAction::FocusPaneUp),
        (&keybinds.focus_pane_right, KeybindAction::FocusPaneRight),
        (&keybinds.swap_pane_left, KeybindAction::SwapPaneLeft),
        (&keybinds.swap_pane_down, KeybindAction::SwapPaneDown),
        (&keybinds.swap_pane_up, KeybindAction::SwapPaneUp),
        (&keybinds.swap_pane_right, KeybindAction::SwapPaneRight),
        (&keybinds.last_pane, KeybindAction::LastPane),
        (&keybinds.cycle_pane_next, KeybindAction::CyclePaneNext),
        (
            &keybinds.cycle_pane_previous,
            KeybindAction::CyclePanePrevious,
        ),
        (&keybinds.split_vertical, KeybindAction::SplitVertical),
        (&keybinds.split_horizontal, KeybindAction::SplitHorizontal),
        (&keybinds.close_pane, KeybindAction::ClosePane),
        (&keybinds.zoom, KeybindAction::Zoom),
        (&keybinds.resize_mode, KeybindAction::EnterResizeMode),
        (&keybinds.resize_pane_left, KeybindAction::ResizePaneLeft),
        (&keybinds.resize_pane_down, KeybindAction::ResizePaneDown),
        (&keybinds.resize_pane_up, KeybindAction::ResizePaneUp),
        (&keybinds.resize_pane_right, KeybindAction::ResizePaneRight),
        (&keybinds.toggle_sidebar, KeybindAction::ToggleSidebar),
        (&keybinds.reload_config, KeybindAction::ReloadConfig),
        (
            &keybinds.open_notification_target,
            KeybindAction::OpenNotificationTarget,
        ),
        (&keybinds.detach, KeybindAction::Detach),
        (&keybinds.goto, KeybindAction::OpenNavigator),
        (&keybinds.maki_sessions, KeybindAction::OpenMakiSessions),
    ] {
        if matches(bindings, key) {
            return Some(action);
        }
    }
    None
}

fn custom_command(keybinds: &Keybinds, key: &TerminalKey) -> Option<CustomCommandKeybind> {
    keybinds
        .custom_commands
        .iter()
        .find(|binding| binding.bindings.matches_key(key))
        .cloned()
}

fn indexed_action(keybinds: &Keybinds, key: &TerminalKey) -> Option<KeybindAction> {
    indexed_action_with(keybinds, key, false)
}

fn indexed_action_with(
    keybinds: &Keybinds,
    key: &TerminalKey,
    modified_only: bool,
) -> Option<KeybindAction> {
    let actual_modifiers = crate::config::normalize_key_combo((key.code, key.modifiers)).1;

    for exact_modifiers in [true, false] {
        let trigger_matches = |binding: &crate::config::IndexedKeybind| {
            let expected_modifiers = crate::config::normalize_key_combo(binding.combo).1;
            (actual_modifiers == expected_modifiers) == exact_modifiers
                && (!modified_only || !crate::config::is_unmodified_printable(binding.combo))
        };

        for binding in &keybinds.switch_tab {
            if trigger_matches(binding) {
                if let Some(index) = binding.matched_index(key) {
                    return Some(KeybindAction::SwitchTab(index));
                }
            }
        }
        for binding in &keybinds.switch_workspace {
            if trigger_matches(binding) {
                if let Some(index) = binding.matched_index(key) {
                    return Some(KeybindAction::SwitchWorkspace(index));
                }
            }
        }
        for binding in &keybinds.focus_agent {
            if trigger_matches(binding) {
                if let Some(index) = binding.matched_index(key) {
                    return Some(KeybindAction::FocusAgent(index));
                }
            }
        }
    }

    None
}

fn generated_character_key(key: &TerminalKey) -> Option<TerminalKey> {
    let mut characters = key.generated_text.as_deref()?.chars();
    let character = characters.next()?;
    if character.is_control() || characters.next().is_some() {
        return None;
    }
    Some(TerminalKey::new(
        KeyCode::Char(character),
        crossterm::event::KeyModifiers::empty(),
    ))
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyModifiers};

    use super::*;

    #[test]
    fn clear_pane_is_unbound_by_default_and_configurable() {
        assert!(crate::config::Config::default()
            .keybinds()
            .clear_pane
            .bindings
            .is_empty());
        let config: crate::config::Config =
            toml::from_str("[keys]\nclear_pane = [\"super+k\", \"ctrl+k\"]").unwrap();
        assert!(config.collect_diagnostics().is_empty());
        let keybinds = config.keybinds();
        assert!(matches!(
            resolve_binding(
                &keybinds,
                &TerminalKey::new(KeyCode::Char('k'), KeyModifiers::SUPER)
            ),
            Some(KeybindMatch::Action(KeybindAction::ClearPane))
        ));
        assert!(matches!(
            resolve_binding(
                &keybinds,
                &TerminalKey::new(KeyCode::Char('k'), KeyModifiers::CONTROL)
            ),
            Some(KeybindMatch::Action(KeybindAction::ClearPane))
        ));
    }

    #[test]
    fn one_resolver_handles_action_command_and_indexed_bindings() {
        let keybinds = Keybinds {
            next_tab: crate::config::ActionKeybinds::direct("ctrl+n"),
            ..Keybinds::default()
        };

        let direct = TerminalKey::new(KeyCode::Char('n'), KeyModifiers::CONTROL);
        assert!(matches!(
            resolve_binding(&keybinds, &direct),
            Some(KeybindMatch::Action(KeybindAction::NextTab))
        ));

        let help = TerminalKey::new(KeyCode::Char('?'), KeyModifiers::empty());
        assert!(matches!(
            resolve_binding(&keybinds, &help),
            Some(KeybindMatch::Action(KeybindAction::Help))
        ));

        let one = TerminalKey::new(KeyCode::Char('1'), KeyModifiers::empty());
        assert!(matches!(
            resolve_binding(&keybinds, &one),
            Some(KeybindMatch::Action(KeybindAction::SwitchTab(0)))
        ));
    }

    #[test]
    fn resolution_uses_generated_character_fallback() {
        let keybinds = Keybinds::default();
        let key = TerminalKey::new(KeyCode::Char('/'), KeyModifiers::SHIFT)
            .with_generated_text(Some("?".to_owned()));

        assert!(matches!(
            resolve_binding(&keybinds, &key),
            Some(KeybindMatch::Action(KeybindAction::Help))
        ));
    }
}
