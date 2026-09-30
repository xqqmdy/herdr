use crossterm::event::{KeyCode, KeyModifiers};
use serde::{Deserialize, Serialize};
use tracing::warn;

use super::Config;
use crate::input::TerminalKey;
use crate::popup_size::PopupSize;

pub type KeyCombo = (KeyCode, KeyModifiers);

/// NORMAL-mode action keys layered over the action binding table.
/// `None` disables the override.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct VimNormalKeys {
    pub focus_left: Option<KeyCombo>,
    pub focus_down: Option<KeyCombo>,
    pub focus_up: Option<KeyCombo>,
    pub focus_right: Option<KeyCombo>,
    pub previous_tab: Option<KeyCombo>,
    pub next_tab: Option<KeyCombo>,
    pub previous_workspace: Option<KeyCombo>,
    pub next_workspace: Option<KeyCombo>,
    pub new_tab: Option<KeyCombo>,
    pub close_pane: Option<KeyCombo>,
    pub split_down: Option<KeyCombo>,
    pub split_right: Option<KeyCombo>,
    pub visual_mode: Option<KeyCombo>,
    pub zoom: Option<KeyCombo>,
    pub help: Option<KeyCombo>,
    pub maki_sessions: Option<KeyCombo>,
    pub detach: Option<KeyCombo>,
    pub rename_tab: Option<KeyCombo>,
}

/// Parsed vim mode layer. `insert` leaves NORMAL mode for terminal input,
/// `normal` is the key sequence that returns from terminal input to NORMAL
/// mode (usually a single key, or a typed chord like `jj`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VimKeyConfig {
    pub insert: KeyCombo,
    pub normal: Vec<KeyCombo>,
    pub normal_keys: VimNormalKeys,
}

pub(crate) const DEFAULT_VIM_INSERT: KeyCombo = (KeyCode::Char('i'), KeyModifiers::empty());
pub(crate) const DEFAULT_VIM_NORMAL: [KeyCombo; 2] = [
    (KeyCode::Char('j'), KeyModifiers::empty()),
    (KeyCode::Char('j'), KeyModifiers::empty()),
];

/// One key combo, or a typed character sequence like `jj` for chord exits.
fn parse_key_sequence(
    raw: &str,
    what: &str,
    diagnostics: &mut Vec<String>,
) -> Option<Vec<KeyCombo>> {
    let raw = raw.trim();
    if let Some(combo) = parse_key_combo(raw) {
        return Some(vec![combo]);
    }
    let sequence: Option<Vec<KeyCombo>> = raw
        .chars()
        .map(|c| {
            (!c.is_control() && !c.is_whitespace())
                .then_some((KeyCode::Char(c), KeyModifiers::empty()))
        })
        .collect();
    sequence.or_else(|| {
        diagnostics.push(format!(
            "invalid keybinding: {what} = {raw:?}; ignoring binding"
        ));
        None
    })
}

impl Default for VimKeyConfig {
    fn default() -> Self {
        Self {
            insert: DEFAULT_VIM_INSERT,
            normal: DEFAULT_VIM_NORMAL.to_vec(),
            normal_keys: VimNormalKeys::default(),
        }
    }
}

impl VimKeyConfig {
    pub(crate) fn from_keys(keys: &super::KeysConfig) -> (Self, Vec<String>) {
        let mut diagnostics = Vec::new();
        let normal = keys
            .vim_normal
            .trim()
            .is_empty()
            .then(|| DEFAULT_VIM_NORMAL.to_vec())
            .or_else(|| parse_key_sequence(&keys.vim_normal, "keys.vim_normal", &mut diagnostics))
            .unwrap_or_else(|| DEFAULT_VIM_NORMAL.to_vec());
        let mut combo = |raw: &str, what: &str| -> Option<KeyCombo> {
            let raw = raw.trim();
            if raw.is_empty() {
                return None;
            }
            match parse_key_combo(raw) {
                Some(combo) => Some(combo),
                None => {
                    diagnostics.push(format!(
                        "invalid keybinding: {what} = {raw:?}; ignoring binding"
                    ));
                    None
                }
            }
        };
        let insert = keys
            .vim_insert
            .trim()
            .is_empty()
            .then_some(DEFAULT_VIM_INSERT)
            .or_else(|| combo(&keys.vim_insert, "keys.vim_insert"))
            .unwrap_or(DEFAULT_VIM_INSERT);
        let n = &keys.normal;
        let normal_keys = VimNormalKeys {
            focus_left: combo(&n.focus_left, "keys.normal.focus_left"),
            focus_down: combo(&n.focus_down, "keys.normal.focus_down"),
            focus_up: combo(&n.focus_up, "keys.normal.focus_up"),
            focus_right: combo(&n.focus_right, "keys.normal.focus_right"),
            previous_tab: combo(&n.previous_tab, "keys.normal.previous_tab"),
            next_tab: combo(&n.next_tab, "keys.normal.next_tab"),
            previous_workspace: combo(&n.previous_workspace, "keys.normal.previous_workspace"),
            next_workspace: combo(&n.next_workspace, "keys.normal.next_workspace"),
            new_tab: combo(&n.new_tab, "keys.normal.new_tab"),
            close_pane: combo(&n.close_pane, "keys.normal.close_pane"),
            split_down: combo(&n.split_down, "keys.normal.split_down"),
            split_right: combo(&n.split_right, "keys.normal.split_right"),
            visual_mode: combo(&n.visual_mode, "keys.normal.visual_mode"),
            zoom: combo(&n.zoom, "keys.normal.zoom"),
            help: combo(&n.help, "keys.normal.help"),
            maki_sessions: combo(&n.maki_sessions, "keys.normal.maki_sessions"),
            detach: combo(&n.detach, "keys.normal.detach"),
            rename_tab: combo(&n.rename_tab, "keys.normal.rename_tab"),
        };
        (
            Self {
                insert,
                normal,
                normal_keys,
            },
            diagnostics,
        )
    }
}

#[derive(Debug, Clone)]
pub struct LiveKeybindConfig {
    pub keybinds: Keybinds,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum BindingConfig {
    One(String),
    Many(Vec<String>),
}

impl Default for BindingConfig {
    fn default() -> Self {
        Self::One(String::new())
    }
}

impl BindingConfig {
    pub fn one(value: impl Into<String>) -> Self {
        Self::One(value.into())
    }

    pub fn empty() -> Self {
        Self::One(String::new())
    }

    fn values(&self) -> Vec<&str> {
        match self {
            Self::One(value) => vec![value.as_str()],
            Self::Many(values) => values.iter().map(String::as_str).collect(),
        }
    }

    pub(crate) fn has_values(&self) -> bool {
        self.values().iter().any(|value| !value.trim().is_empty())
    }

    pub(crate) fn indexed_labels(&self) -> Vec<String> {
        let mut labels = Vec::new();
        for raw in self.values() {
            let raw = raw.trim();
            if raw.is_empty() {
                continue;
            }
            match parse_binding_string(raw) {
                Some(ParsedBinding::Single(binding)) => {
                    if matches!(binding.combo.0, KeyCode::Char('1'..='9')) {
                        labels.push(binding.label);
                    }
                }
                Some(ParsedBinding::Range(range)) => {
                    labels.extend(range.into_iter().filter_map(|binding| {
                        matches!(binding.combo.0, KeyCode::Char('1'..='9')).then_some(binding.label)
                    }));
                }
                None => {}
            }
        }
        labels
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CommandKeybindType {
    #[default]
    Shell,
    Pane,
    Popup,
    PluginAction,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct CommandKeybindConfig {
    /// Key that runs a command. Use `prefix+g` for prefix mode or a modified chord for direct mode.
    pub key: BindingConfig,
    /// Command executed either in the background shell or inside a pane.
    pub command: String,
    /// Command execution mode. Default: "shell".
    #[serde(rename = "type")]
    pub action_type: CommandKeybindType,
    /// Optional user-defined description for this custom command.
    pub description: Option<String>,
    /// Optional popup width as cells or a percentage string when type = "popup".
    pub width: Option<PopupSize>,
    /// Optional popup height as cells or a percentage string when type = "popup".
    pub height: Option<PopupSize>,
}

impl Default for CommandKeybindConfig {
    fn default() -> Self {
        Self {
            key: BindingConfig::empty(),
            command: String::new(),
            action_type: CommandKeybindType::Shell,
            description: None,
            width: None,
            height: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomCommandAction {
    Shell,
    Pane,
    Popup,
    PluginAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedBinding {
    pub combo: KeyCombo,
    pub label: String,
}

impl ResolvedBinding {
    fn matches_terminal_key(&self, key: &TerminalKey) -> bool {
        terminal_key_matches_combo(key, self.combo)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ActionKeybinds {
    pub bindings: Vec<ResolvedBinding>,
}

impl ActionKeybinds {
    pub(crate) fn from_labels(labels: &[String]) -> Result<Self, String> {
        let mut bindings = Vec::new();
        for label in labels {
            match parse_binding_string(label) {
                Some(ParsedBinding::Single(binding)) => bindings.push(binding),
                Some(ParsedBinding::Range(range)) => bindings.extend(range),
                None => return Err(format!("invalid endpoint command binding: {label}")),
            }
        }
        Ok(Self { bindings })
    }

    #[cfg(test)]
    pub fn direct(label: &str) -> Self {
        let binding = parse_binding_string(label)
            .and_then(|parsed| match parsed {
                ParsedBinding::Single(binding) => Some(binding),
                ParsedBinding::Range(_) => None,
            })
            .expect("binding should parse");
        Self {
            bindings: vec![binding],
        }
    }

    pub fn matches_key(&self, key: &TerminalKey) -> bool {
        self.bindings
            .iter()
            .any(|binding| binding.matches_terminal_key(key))
    }

    /// Bindings safe to match while a pane receives typed input: modified
    /// chords only, so unmodified printable keys keep typing.
    pub fn matches_modified_key(&self, key: &TerminalKey) -> bool {
        self.bindings.iter().any(|binding| {
            !is_unmodified_printable(binding.combo) && binding.matches_terminal_key(key)
        })
    }

    pub fn labels(&self) -> Vec<String> {
        self.bindings
            .iter()
            .map(|binding| binding.label.clone())
            .collect()
    }

    pub fn label(&self) -> Option<String> {
        let labels = self.labels();
        if labels.is_empty() {
            None
        } else {
            Some(labels.join(" / "))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedKeybind {
    pub combo: KeyCombo,
    pub label: String,
}

impl IndexedKeybind {
    pub fn matched_index(&self, key: &TerminalKey) -> Option<usize> {
        let combo = self.combo;
        let (expected_code, _) = normalize_key_combo(combo);
        let KeyCode::Char(key_number @ '1'..='9') = expected_code else {
            return None;
        };
        let legacy_shifted_number = matches!(key.code, KeyCode::Char(c)
            if shifted_number_symbol(c) == Some(key_number)
                && indexed_shifted_number_matches(key, combo, key_number));
        if terminal_key_matches_combo(key, combo) || legacy_shifted_number {
            Some((key_number as usize) - ('1' as usize))
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
pub struct CustomCommandKeybind {
    pub bindings: ActionKeybinds,
    pub label: String,
    pub command: String,
    pub action: CustomCommandAction,
    pub description: Option<String>,
    pub width: Option<PopupSize>,
    pub height: Option<PopupSize>,
}

/// Parsed keybinds for Herdr actions.
#[derive(Debug, Clone)]
pub struct Keybinds {
    pub help: ActionKeybinds,
    pub settings: ActionKeybinds,
    pub new_workspace: ActionKeybinds,
    pub new_worktree: ActionKeybinds,
    pub open_worktree: ActionKeybinds,
    pub remove_worktree: ActionKeybinds,
    pub rename_workspace: ActionKeybinds,
    pub close_workspace: ActionKeybinds,
    pub goto: ActionKeybinds,
    pub maki_sessions: ActionKeybinds,
    pub detach: ActionKeybinds,
    pub reload_config: ActionKeybinds,
    pub open_notification_target: ActionKeybinds,
    pub previous_workspace: ActionKeybinds,
    pub next_workspace: ActionKeybinds,
    pub previous_agent: ActionKeybinds,
    pub next_agent: ActionKeybinds,
    pub focus_agent: Vec<IndexedKeybind>,
    pub new_tab: ActionKeybinds,
    pub rename_tab: ActionKeybinds,
    pub previous_tab: ActionKeybinds,
    pub next_tab: ActionKeybinds,
    pub move_tab_previous: ActionKeybinds,
    pub move_tab_next: ActionKeybinds,
    pub switch_tab: Vec<IndexedKeybind>,
    pub switch_workspace: Vec<IndexedKeybind>,
    pub close_tab: ActionKeybinds,
    pub rename_pane: ActionKeybinds,
    pub edit_scrollback: ActionKeybinds,
    pub clear_pane: ActionKeybinds,
    pub copy_mode: ActionKeybinds,
    pub focus_pane_left: ActionKeybinds,
    pub focus_pane_down: ActionKeybinds,
    pub focus_pane_up: ActionKeybinds,
    pub focus_pane_right: ActionKeybinds,
    pub swap_pane_left: ActionKeybinds,
    pub swap_pane_down: ActionKeybinds,
    pub swap_pane_up: ActionKeybinds,
    pub swap_pane_right: ActionKeybinds,
    pub cycle_pane_next: ActionKeybinds,
    pub cycle_pane_previous: ActionKeybinds,
    pub last_pane: ActionKeybinds,
    pub split_vertical: ActionKeybinds,
    pub split_horizontal: ActionKeybinds,
    pub close_pane: ActionKeybinds,
    pub zoom: ActionKeybinds,
    pub resize_mode: ActionKeybinds,
    pub resize_pane_left: ActionKeybinds,
    pub resize_pane_down: ActionKeybinds,
    pub resize_pane_up: ActionKeybinds,
    pub resize_pane_right: ActionKeybinds,
    pub toggle_sidebar: ActionKeybinds,
    pub custom_commands: Vec<CustomCommandKeybind>,
}

impl Default for Keybinds {
    fn default() -> Self {
        Config::default().keybinds()
    }
}

#[derive(Clone)]
enum ParsedBinding {
    Single(ResolvedBinding),
    Range(Vec<ResolvedBinding>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BindingSource {
    Default,
    User,
}

struct RegisteredBinding {
    field: String,
    source: BindingSource,
}

struct BindingRegistry {
    bindings: std::collections::HashMap<KeyCombo, RegisteredBinding>,
}

impl BindingRegistry {
    fn new() -> Self {
        Self {
            bindings: std::collections::HashMap::new(),
        }
    }

    fn conflict(&self, binding: &ResolvedBinding) -> Option<&RegisteredBinding> {
        self.bindings.get(&normalize_key_combo(binding.combo))
    }

    fn register(&mut self, binding: &ResolvedBinding, field: &str, source: BindingSource) {
        self.bindings.insert(
            normalize_key_combo(binding.combo),
            RegisteredBinding {
                field: field.to_string(),
                source,
            },
        );
    }
}

impl Config {
    pub(super) fn validated_keybinds(&self) -> (Vec<String>, Keybinds) {
        let mut diagnostics = Vec::new();
        for diag in &diagnostics {
            warn!(message = %diag, "config diagnostic");
        }

        let mut registry = BindingRegistry::new();

        macro_rules! empty_action {
            () => {
                ActionKeybinds::default()
            };
        }

        let mut keybinds = Keybinds {
            help: empty_action!(),
            settings: empty_action!(),
            new_workspace: empty_action!(),
            new_worktree: empty_action!(),
            open_worktree: empty_action!(),
            remove_worktree: empty_action!(),
            rename_workspace: empty_action!(),
            close_workspace: empty_action!(),
            goto: empty_action!(),
            maki_sessions: empty_action!(),
            detach: empty_action!(),
            reload_config: empty_action!(),
            open_notification_target: empty_action!(),
            previous_workspace: empty_action!(),
            next_workspace: empty_action!(),
            previous_agent: empty_action!(),
            next_agent: empty_action!(),
            focus_agent: Vec::new(),
            new_tab: empty_action!(),
            rename_tab: empty_action!(),
            previous_tab: empty_action!(),
            next_tab: empty_action!(),
            move_tab_previous: empty_action!(),
            move_tab_next: empty_action!(),
            switch_tab: Vec::new(),
            switch_workspace: Vec::new(),
            close_tab: empty_action!(),
            rename_pane: empty_action!(),
            edit_scrollback: empty_action!(),
            clear_pane: empty_action!(),
            copy_mode: empty_action!(),
            focus_pane_left: empty_action!(),
            focus_pane_down: empty_action!(),
            focus_pane_up: empty_action!(),
            focus_pane_right: empty_action!(),
            swap_pane_left: empty_action!(),
            swap_pane_down: empty_action!(),
            swap_pane_up: empty_action!(),
            swap_pane_right: empty_action!(),
            cycle_pane_next: empty_action!(),
            cycle_pane_previous: empty_action!(),
            last_pane: empty_action!(),
            split_vertical: empty_action!(),
            split_horizontal: empty_action!(),
            close_pane: empty_action!(),
            zoom: empty_action!(),
            resize_mode: empty_action!(),
            resize_pane_left: empty_action!(),
            resize_pane_down: empty_action!(),
            resize_pane_up: empty_action!(),
            resize_pane_right: empty_action!(),
            toggle_sidebar: empty_action!(),
            custom_commands: Vec::new(),
        };

        macro_rules! field_source {
            ($field:ident) => {
                if self.keys.key_field_is_user_configured(stringify!($field)) {
                    BindingSource::User
                } else {
                    BindingSource::Default
                }
            };
        }
        macro_rules! apply_action {
            ($target:expr, $field:ident, $source:expr) => {
                if field_source!($field) == $source {
                    $target = parse_action_bindings(
                        concat!("keys.", stringify!($field)),
                        &self.keys.$field,
                        &mut registry,
                        &mut diagnostics,
                        $source,
                    );
                }
            };
        }
        macro_rules! apply_indexed {
            (
                $target:expr,
                $field:ident,
                $legacy_config:expr,
                $source:expr
            ) => {
                if field_source!($field) == $source {
                    if $source == BindingSource::Default && !$legacy_config.trim().is_empty() {
                        // A legacy [keys.indexed] entry is user configuration for
                        // this target and should displace the modern default.
                    } else {
                        $target = parse_indexed_bindings(
                            concat!("keys.", stringify!($field)),
                            &self.keys.$field,
                            &mut registry,
                            &mut diagnostics,
                            $source,
                        );
                    }
                }
            };
        }

        for source in [BindingSource::User, BindingSource::Default] {
            apply_action!(keybinds.help, help, source);
            apply_action!(keybinds.settings, settings, source);
            apply_action!(keybinds.new_workspace, new_workspace, source);
            apply_action!(keybinds.new_worktree, new_worktree, source);
            apply_action!(keybinds.open_worktree, open_worktree, source);
            apply_action!(keybinds.remove_worktree, remove_worktree, source);
            apply_action!(keybinds.rename_workspace, rename_workspace, source);
            apply_action!(keybinds.close_workspace, close_workspace, source);
            apply_action!(keybinds.goto, goto, source);
            apply_action!(keybinds.maki_sessions, maki_sessions, source);
            apply_action!(keybinds.detach, detach, source);
            apply_action!(keybinds.reload_config, reload_config, source);
            apply_action!(
                keybinds.open_notification_target,
                open_notification_target,
                source
            );
            apply_action!(keybinds.previous_workspace, previous_workspace, source);
            apply_action!(keybinds.next_workspace, next_workspace, source);
            apply_action!(keybinds.previous_agent, previous_agent, source);
            apply_action!(keybinds.next_agent, next_agent, source);
            apply_indexed!(
                keybinds.focus_agent,
                focus_agent,
                &self.keys.indexed.agents,
                source
            );
            apply_action!(keybinds.new_tab, new_tab, source);
            apply_action!(keybinds.rename_tab, rename_tab, source);
            apply_action!(keybinds.previous_tab, previous_tab, source);
            apply_action!(keybinds.next_tab, next_tab, source);
            apply_action!(keybinds.move_tab_previous, move_tab_previous, source);
            apply_action!(keybinds.move_tab_next, move_tab_next, source);
            apply_indexed!(
                keybinds.switch_tab,
                switch_tab,
                &self.keys.indexed.tabs,
                source
            );
            apply_indexed!(
                keybinds.switch_workspace,
                switch_workspace,
                &self.keys.indexed.workspaces,
                source
            );
            apply_action!(keybinds.close_tab, close_tab, source);
            apply_action!(keybinds.rename_pane, rename_pane, source);
            apply_action!(keybinds.edit_scrollback, edit_scrollback, source);
            apply_action!(keybinds.clear_pane, clear_pane, source);
            apply_action!(keybinds.copy_mode, copy_mode, source);
            apply_action!(keybinds.focus_pane_left, focus_pane_left, source);
            apply_action!(keybinds.focus_pane_down, focus_pane_down, source);
            apply_action!(keybinds.focus_pane_up, focus_pane_up, source);
            apply_action!(keybinds.focus_pane_right, focus_pane_right, source);
            apply_action!(keybinds.swap_pane_left, swap_pane_left, source);
            apply_action!(keybinds.swap_pane_down, swap_pane_down, source);
            apply_action!(keybinds.swap_pane_up, swap_pane_up, source);
            apply_action!(keybinds.swap_pane_right, swap_pane_right, source);
            apply_action!(keybinds.last_pane, last_pane, source);
            apply_action!(keybinds.cycle_pane_next, cycle_pane_next, source);
            apply_action!(keybinds.cycle_pane_previous, cycle_pane_previous, source);
            apply_action!(keybinds.split_vertical, split_vertical, source);
            apply_action!(keybinds.split_horizontal, split_horizontal, source);
            apply_action!(keybinds.close_pane, close_pane, source);
            apply_action!(keybinds.zoom, zoom, source);
            apply_action!(keybinds.resize_mode, resize_mode, source);
            apply_action!(keybinds.resize_pane_left, resize_pane_left, source);
            apply_action!(keybinds.resize_pane_down, resize_pane_down, source);
            apply_action!(keybinds.resize_pane_up, resize_pane_up, source);
            apply_action!(keybinds.resize_pane_right, resize_pane_right, source);
            apply_action!(keybinds.toggle_sidebar, toggle_sidebar, source);

            if source == field_source!(indexed) {
                append_legacy_indexed_bindings(
                    &mut keybinds.switch_tab,
                    "keys.indexed.tabs",
                    &self.keys.indexed.tabs,
                    &mut registry,
                    &mut diagnostics,
                    source,
                );
                append_legacy_indexed_bindings(
                    &mut keybinds.switch_workspace,
                    "keys.indexed.workspaces",
                    &self.keys.indexed.workspaces,
                    &mut registry,
                    &mut diagnostics,
                    source,
                );
                append_legacy_indexed_bindings(
                    &mut keybinds.focus_agent,
                    "keys.indexed.agents",
                    &self.keys.indexed.agents,
                    &mut registry,
                    &mut diagnostics,
                    source,
                );
            }

            if source == BindingSource::User {
                append_custom_command_bindings(
                    self,
                    &mut keybinds,
                    &mut registry,
                    &mut diagnostics,
                );
            }
        }

        (diagnostics, keybinds)
    }
}

fn append_custom_command_bindings(
    config: &Config,
    keybinds: &mut Keybinds,
    registry: &mut BindingRegistry,
    diagnostics: &mut Vec<String>,
) {
    for (index, command) in config.keys.command.iter().enumerate() {
        let key_field = format!("keys.command[{index}].key");
        let command_field = format!("keys.command[{index}].command");

        if command.command.trim().is_empty() {
            let diag = format!("empty custom command: {command_field}; disabling custom command");
            warn!(message = %diag, "config diagnostic");
            diagnostics.push(diag);
            continue;
        }

        let bindings = parse_action_bindings(
            &key_field,
            &command.key,
            registry,
            diagnostics,
            BindingSource::User,
        );
        if bindings.bindings.is_empty() {
            continue;
        }

        let action = match command.action_type {
            CommandKeybindType::Shell => CustomCommandAction::Shell,
            CommandKeybindType::Pane => CustomCommandAction::Pane,
            CommandKeybindType::Popup => CustomCommandAction::Popup,
            CommandKeybindType::PluginAction => CustomCommandAction::PluginAction,
        };
        let (width, height) = if action == CustomCommandAction::Popup {
            (command.width, command.height)
        } else {
            if command.width.is_some() || command.height.is_some() {
                let diag = format!(
                    "popup size on non-popup custom command: keys.command[{index}]; ignoring width and height"
                );
                warn!(message = %diag, "config diagnostic");
                diagnostics.push(diag);
            }
            (None, None)
        };
        let label = bindings.label().unwrap_or_else(|| "unset".to_string());
        keybinds.custom_commands.push(CustomCommandKeybind {
            bindings,
            label,
            command: command.command.clone(),
            action,
            description: command.description.clone(),
            width,
            height,
        });
    }
}

fn parse_action_bindings(
    field: &str,
    config: &BindingConfig,
    registry: &mut BindingRegistry,
    diagnostics: &mut Vec<String>,
    source: BindingSource,
) -> ActionKeybinds {
    let mut bindings = Vec::new();
    for raw in config.values() {
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }
        match parse_binding_string(raw) {
            Some(ParsedBinding::Single(binding)) => {
                if reject_binding(field, &binding, registry, diagnostics, source) {
                    continue;
                }
                registry.register(&binding, field, source);
                bindings.push(binding);
            }
            Some(ParsedBinding::Range(_)) => {
                let diag = format!("range keybinding is only valid for indexed actions: {field} = {raw:?}; disabling binding");
                warn!(message = %diag, "config diagnostic");
                diagnostics.push(diag);
            }
            None => {
                let diag = format!("invalid keybinding: {field} = {raw:?}; disabling binding");
                warn!(message = %diag, "config diagnostic");
                diagnostics.push(diag);
            }
        }
    }
    ActionKeybinds { bindings }
}

fn parse_indexed_bindings(
    field: &'static str,
    config: &BindingConfig,
    registry: &mut BindingRegistry,
    diagnostics: &mut Vec<String>,
    source: BindingSource,
) -> Vec<IndexedKeybind> {
    let mut bindings = Vec::new();
    for raw in config.values() {
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }
        match parse_binding_string(raw) {
            Some(ParsedBinding::Single(binding)) => {
                push_indexed_binding(field, binding, registry, diagnostics, source, &mut bindings);
            }
            Some(ParsedBinding::Range(range)) => {
                for binding in range {
                    push_indexed_binding(
                        field,
                        binding,
                        registry,
                        diagnostics,
                        source,
                        &mut bindings,
                    );
                }
            }
            None => {
                let diag = format!("invalid keybinding: {field} = {raw:?}; disabling binding");
                warn!(message = %diag, "config diagnostic");
                diagnostics.push(diag);
            }
        }
    }
    bindings
}

fn push_indexed_binding(
    field: &str,
    binding: ResolvedBinding,
    registry: &mut BindingRegistry,
    diagnostics: &mut Vec<String>,
    source: BindingSource,
    bindings: &mut Vec<IndexedKeybind>,
) {
    if !matches!(binding.combo.0, KeyCode::Char('1'..='9')) {
        let diag = format!(
            "indexed keybinding must use 1..9: {field} = {:?}; disabling binding",
            binding.label
        );
        warn!(message = %diag, "config diagnostic");
        diagnostics.push(diag);
        return;
    }
    if reject_binding(field, &binding, registry, diagnostics, source) {
        return;
    }
    registry.register(&binding, field, source);
    bindings.push(IndexedKeybind {
        combo: binding.combo,
        label: binding.label,
    });
}

fn append_legacy_indexed_bindings(
    target: &mut Vec<IndexedKeybind>,
    field: &'static str,
    configured_label: &str,
    registry: &mut BindingRegistry,
    diagnostics: &mut Vec<String>,
    source: BindingSource,
) {
    if configured_label.trim().is_empty() {
        return;
    }
    let Some(modifiers) = parse_modifier_combo(configured_label) else {
        let diag = format!(
            "invalid indexed keybinding: {field} = {configured_label:?}; disabling binding"
        );
        warn!(message = %diag, "config diagnostic");
        diagnostics.push(diag);
        return;
    };

    for idx in 1..=9 {
        let combo = (
            KeyCode::Char(char::from_digit(idx, 10).unwrap_or('1')),
            modifiers,
        );
        let binding = ResolvedBinding {
            combo,
            label: format!("{}+{idx}", configured_label.trim()),
        };
        if reject_binding(field, &binding, registry, diagnostics, source) {
            continue;
        }
        registry.register(&binding, field, source);
        target.push(IndexedKeybind {
            combo: binding.combo,
            label: binding.label,
        });
    }
}

fn reject_binding(
    field: &str,
    binding: &ResolvedBinding,
    registry: &BindingRegistry,
    diagnostics: &mut Vec<String>,
    source: BindingSource,
) -> bool {
    if let Some(first_binding) = registry.conflict(binding) {
        if source == BindingSource::Default && first_binding.source == BindingSource::User {
            return true;
        }
        let first_field = &first_binding.field;
        let diag = format!("{}: kept {first_field}, disabled {field}", binding.label);
        warn!(message = %diag, "config diagnostic");
        diagnostics.push(diag);
        return true;
    }

    false
}

fn parse_binding_string(raw: &str) -> Option<ParsedBinding> {
    // Legacy `prefix+X` configs predate vim mode; the prefix concept is gone,
    // so the trigger is the bare key.
    let body = raw.trim().strip_prefix("prefix+").unwrap_or(raw.trim());

    if let Some(range_modifiers) = parse_range_modifiers(body) {
        let bindings = (1..=9)
            .map(|idx| {
                let combo = (
                    KeyCode::Char(char::from_digit(idx, 10).unwrap_or('1')),
                    range_modifiers,
                );
                ResolvedBinding {
                    combo,
                    label: format_key_combo(combo),
                }
            })
            .collect();
        return Some(ParsedBinding::Range(bindings));
    }

    let combo = parse_key_combo(body)?;
    Some(ParsedBinding::Single(ResolvedBinding {
        combo,
        label: format_key_combo(combo),
    }))
}

pub fn format_key_combo(binding: KeyCombo) -> String {
    let (code, modifiers) = binding;
    let mut parts = Vec::new();
    if modifiers.contains(KeyModifiers::CONTROL) {
        parts.push("ctrl".to_string());
    }
    if modifiers.contains(KeyModifiers::ALT) {
        parts.push("alt".to_string());
    }
    if modifiers.contains(KeyModifiers::SHIFT) && !matches!(code, KeyCode::BackTab) {
        parts.push("shift".to_string());
    }
    if modifiers.contains(KeyModifiers::SUPER) {
        parts.push(super_modifier_label().to_string());
    }
    if modifiers.contains(KeyModifiers::HYPER) {
        parts.push("hyper".to_string());
    }
    if modifiers.contains(KeyModifiers::META) {
        parts.push("meta".to_string());
    }

    let key = match code {
        KeyCode::Char(' ') => "space".to_string(),
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Enter => "enter".to_string(),
        KeyCode::Esc => "esc".to_string(),
        KeyCode::Tab => "tab".to_string(),
        KeyCode::BackTab => "shift+tab".to_string(),
        KeyCode::Backspace => "backspace".to_string(),
        KeyCode::Left => "left".to_string(),
        KeyCode::Right => "right".to_string(),
        KeyCode::Up => "up".to_string(),
        KeyCode::Down => "down".to_string(),
        KeyCode::F(n) => format!("f{n}"),
        _ => format!("{:?}", code).to_lowercase(),
    };

    if matches!(code, KeyCode::BackTab) {
        return if parts.is_empty() {
            key
        } else {
            format!("{}+{key}", parts.join("+"))
        };
    }

    parts.push(key);
    parts.join("+")
}

/// Render a key sequence like `jj` for display in the mode bar and help.
pub fn format_key_sequence(sequence: &[KeyCombo]) -> String {
    match sequence {
        [single] => format_key_combo(*single),
        sequence => sequence
            .iter()
            .map(|combo| format_key_combo(*combo))
            .collect(),
    }
}

fn super_modifier_label() -> &'static str {
    if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "super"
    }
}

fn parse_modifier_token(token: &str) -> Option<KeyModifiers> {
    match token.to_lowercase().as_str() {
        "ctrl" | "control" => Some(KeyModifiers::CONTROL),
        "shift" => Some(KeyModifiers::SHIFT),
        "alt" | "option" | "meta" => Some(KeyModifiers::ALT),
        "cmd" | "command" | "super" => Some(KeyModifiers::SUPER),
        "hyper" => Some(KeyModifiers::HYPER),
        _ => None,
    }
}

fn parse_range_modifiers(s: &str) -> Option<KeyModifiers> {
    let mut modifiers = KeyModifiers::empty();
    let mut saw_range = false;
    for part in s.split('+') {
        let trimmed = part.trim();
        if trimmed == "1..9" {
            if saw_range {
                return None;
            }
            saw_range = true;
        } else {
            modifiers |= parse_modifier_token(trimmed)?;
        }
    }
    saw_range.then_some(modifiers)
}

fn parse_modifier_combo(s: &str) -> Option<KeyModifiers> {
    let mut modifiers = KeyModifiers::empty();
    let parts: Vec<&str> = s.split('+').collect();
    if parts.is_empty() {
        return None;
    }

    for part in &parts {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            return None;
        }
        modifiers |= parse_modifier_token(trimmed)?;
    }

    if modifiers.is_empty() {
        None
    } else {
        Some(modifiers)
    }
}

pub(crate) fn parse_key_combo(s: &str) -> Option<KeyCombo> {
    let parts: Vec<&str> = s.split('+').collect();
    let mut modifiers = KeyModifiers::empty();
    let mut key_str: Option<&str> = None;

    for part in &parts {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            return None;
        }
        if let Some(modifier) = parse_modifier_token(trimmed) {
            modifiers |= modifier;
        } else if key_str.is_some() {
            return None;
        } else {
            key_str = Some(trimmed);
        }
    }

    let key_str = key_str?;
    let single_char = single_key_char(key_str);
    let lower = key_str.to_lowercase();
    let code = match lower.as_str() {
        "space" | " " => KeyCode::Char(' '),
        "enter" | "return" => KeyCode::Enter,
        "esc" | "escape" => KeyCode::Esc,
        "tab" if modifiers.contains(KeyModifiers::SHIFT) => {
            modifiers.remove(KeyModifiers::SHIFT);
            KeyCode::BackTab
        }
        "tab" => KeyCode::Tab,
        "backspace" | "bs" => KeyCode::Backspace,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "minus" => KeyCode::Char('-'),
        "comma" => KeyCode::Char(','),
        "period" => KeyCode::Char('.'),
        "slash" => KeyCode::Char('/'),
        "backslash" => KeyCode::Char('\\'),
        "quote" => KeyCode::Char('\''),
        "double_quote" | "double-quote" => KeyCode::Char('"'),
        "semicolon" => KeyCode::Char(';'),
        "colon" => KeyCode::Char(':'),
        "percent" => KeyCode::Char('%'),
        "ampersand" => KeyCode::Char('&'),
        "backtick" => KeyCode::Char('`'),
        "plus" => KeyCode::Char('+'),
        _ if single_char.is_some() => {
            let ch = single_char?;
            if ch.is_ascii_uppercase() {
                modifiers |= KeyModifiers::SHIFT;
                KeyCode::Char(ch.to_ascii_lowercase())
            } else {
                KeyCode::Char(ch)
            }
        }
        s if s.starts_with('f') => s[1..].parse::<u8>().ok().map(KeyCode::F)?,
        _ => return None,
    };

    Some(normalize_key_combo((code, modifiers)))
}

fn single_key_char(s: &str) -> Option<char> {
    let mut chars = s.chars();
    let ch = chars.next()?;
    if chars.next().is_none() {
        Some(ch)
    } else {
        None
    }
}

/// Whether `combo` is a bare printable key (no modifiers beyond shift), i.e.
/// a key a pane would receive as typed input.
pub fn is_unmodified_printable(combo: KeyCombo) -> bool {
    matches!(combo.0, KeyCode::Char(ch) if !ch.is_control())
        && combo.1.difference(KeyModifiers::SHIFT).is_empty()
}

pub fn normalize_key_combo((mut code, mut modifiers): KeyCombo) -> KeyCombo {
    if matches!(code, KeyCode::Tab) && modifiers.contains(KeyModifiers::SHIFT) {
        code = KeyCode::BackTab;
        modifiers.remove(KeyModifiers::SHIFT);
    } else if matches!(code, KeyCode::BackTab) {
        modifiers.remove(KeyModifiers::SHIFT);
    }
    (code, modifiers)
}

pub fn terminal_key_matches_combo(key: &TerminalKey, combo: KeyCombo) -> bool {
    key_parts_match_combo(key.code, key.modifiers, key.shifted_codepoint, combo)
}

fn key_parts_match_combo(
    actual_code: KeyCode,
    actual_modifiers: KeyModifiers,
    shifted_codepoint: Option<u32>,
    combo: KeyCombo,
) -> bool {
    let (actual_code, actual_modifiers) = normalize_key_combo((actual_code, actual_modifiers));
    let (expected_code, expected_modifiers) = normalize_key_combo(combo);

    if actual_modifiers == expected_modifiers
        && key_codes_match(
            actual_code,
            actual_modifiers,
            expected_code,
            expected_modifiers,
            shifted_codepoint,
        )
    {
        return true;
    }

    let actual_without_shift = actual_modifiers.difference(KeyModifiers::SHIFT);
    actual_modifiers.contains(KeyModifiers::SHIFT)
        && actual_without_shift == expected_modifiers
        && shifted_char_matches_expected(actual_code, shifted_codepoint, expected_code)
        || legacy_shifted_ascii_letter_matches(
            actual_code,
            actual_modifiers,
            expected_code,
            expected_modifiers,
        )
}

fn key_codes_match(
    actual: KeyCode,
    actual_modifiers: KeyModifiers,
    expected: KeyCode,
    expected_modifiers: KeyModifiers,
    shifted_codepoint: Option<u32>,
) -> bool {
    match (actual, expected) {
        (KeyCode::Char(actual), KeyCode::Char(expected))
            if actual.is_ascii_alphabetic() && expected.is_ascii_alphabetic() =>
        {
            actual == expected
                || actual_modifiers.contains(KeyModifiers::SHIFT)
                    && expected_modifiers.contains(KeyModifiers::SHIFT)
                    && actual.eq_ignore_ascii_case(&expected)
        }
        (KeyCode::Char(actual), KeyCode::Char(expected)) => {
            actual == expected
                || shifted_char_matches_expected(
                    KeyCode::Char(actual),
                    shifted_codepoint,
                    KeyCode::Char(expected),
                )
        }
        (actual, expected) => actual == expected,
    }
}

fn legacy_shifted_ascii_letter_matches(
    actual_code: KeyCode,
    actual_modifiers: KeyModifiers,
    expected_code: KeyCode,
    expected_modifiers: KeyModifiers,
) -> bool {
    if actual_modifiers.contains(KeyModifiers::SHIFT) {
        return false;
    }
    let (KeyCode::Char(actual), KeyCode::Char(expected)) = (actual_code, expected_code) else {
        return false;
    };
    actual.is_ascii_uppercase()
        && expected.is_ascii_lowercase()
        && actual.to_ascii_lowercase() == expected
        && actual_modifiers | KeyModifiers::SHIFT == expected_modifiers
}

const SHIFTED_NUMBER_SYMBOLS: [(char, char); 9] = [
    ('1', '!'),
    ('2', '@'),
    ('3', '#'),
    ('4', '$'),
    ('5', '%'),
    ('6', '^'),
    ('7', '&'),
    ('8', '*'),
    ('9', '('),
];

fn shifted_number_symbol(ch: char) -> Option<char> {
    SHIFTED_NUMBER_SYMBOLS
        .iter()
        .find_map(|(number, symbol)| (*symbol == ch).then_some(*number))
}

fn indexed_shifted_number_matches(key: &TerminalKey, combo: KeyCombo, number: char) -> bool {
    let (expected_code, expected_modifiers) = normalize_key_combo(combo);
    matches!(expected_code, KeyCode::Char(expected) if expected == number)
        && expected_modifiers.contains(KeyModifiers::SHIFT)
        && key.modifiers == expected_modifiers.difference(KeyModifiers::SHIFT)
}

fn shifted_char_matches_expected(
    actual_code: KeyCode,
    shifted_codepoint: Option<u32>,
    expected_code: KeyCode,
) -> bool {
    let KeyCode::Char(expected) = expected_code else {
        return false;
    };
    if let Some(shifted) = shifted_codepoint.and_then(char::from_u32) {
        return shifted == expected;
    }
    matches!(actual_code, KeyCode::Char(actual) if actual == expected && is_shifted_punctuation(expected))
}

fn is_shifted_punctuation(ch: char) -> bool {
    matches!(
        ch,
        '!' | '@'
            | '#'
            | '$'
            | '%'
            | '^'
            | '&'
            | '*'
            | '('
            | ')'
            | '_'
            | '+'
            | '{'
            | '}'
            | '|'
            | ':'
            | '"'
            | '<'
            | '>'
            | '?'
            | '~'
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config::Config, input::TerminalKey};

    fn binding_combos(bindings: &ActionKeybinds) -> Vec<KeyCombo> {
        bindings
            .bindings
            .iter()
            .map(|binding| binding.combo)
            .collect()
    }

    #[test]
    fn parse_simple_char_combo() {
        assert_eq!(
            parse_key_combo("v"),
            Some((KeyCode::Char('v'), KeyModifiers::empty()))
        );
    }

    #[test]
    fn parse_unicode_char_combo() {
        assert_eq!(
            parse_key_combo("ö"),
            Some((KeyCode::Char('ö'), KeyModifiers::empty()))
        );
        assert_eq!(
            parse_key_combo("alt+é"),
            Some((KeyCode::Char('é'), KeyModifiers::ALT))
        );
    }

    #[test]
    fn parse_shift_tab_as_backtab() {
        assert_eq!(
            parse_key_combo("shift+tab"),
            Some((KeyCode::BackTab, KeyModifiers::empty()))
        );
    }

    #[test]
    fn parse_named_punctuation() {
        assert_eq!(
            parse_key_combo("minus"),
            Some((KeyCode::Char('-'), KeyModifiers::empty()))
        );
        assert_eq!(
            parse_key_combo("comma"),
            Some((KeyCode::Char(','), KeyModifiers::empty()))
        );
        assert_eq!(
            parse_key_combo("ampersand"),
            Some((KeyCode::Char('&'), KeyModifiers::empty()))
        );
    }

    #[test]
    fn legacy_prefix_syntax_parses_to_bare_key() {
        let config: Config = toml::from_str(
            r#"
[keys]
next_tab = "prefix+n"
"#,
        )
        .unwrap();
        let kb = config.keybinds();
        assert_eq!(
            binding_combos(&kb.next_tab),
            vec![(KeyCode::Char('n'), KeyModifiers::empty())]
        );
        assert_eq!(kb.next_tab.labels(), vec!["n".to_string()]);
    }

    #[test]
    fn new_worktree_defaults_to_shift_g() {
        let kb = Config::default().keybinds();
        assert_eq!(
            binding_combos(&kb.new_worktree),
            vec![(KeyCode::Char('g'), KeyModifiers::SHIFT)]
        );
    }

    #[test]
    fn goto_defaults_to_g() {
        let kb = Config::default().keybinds();
        assert_eq!(
            binding_combos(&kb.goto),
            vec![(KeyCode::Char('g'), KeyModifiers::empty())]
        );
    }

    #[test]
    fn open_and_remove_worktree_keybinds_are_unset_by_default() {
        let kb = Config::default().keybinds();
        assert!(kb.open_worktree.bindings.is_empty());
        assert!(kb.remove_worktree.bindings.is_empty());
    }

    #[test]
    fn copy_mode_uses_bracket_by_default() {
        let kb = Config::default().keybinds();
        assert_eq!(
            binding_combos(&kb.copy_mode),
            vec![(KeyCode::Char('['), KeyModifiers::empty())]
        );
    }

    #[test]
    fn back_and_forth_keybinds_are_unset_by_default() {
        let kb = Config::default().keybinds();
        assert!(kb.last_pane.bindings.is_empty());
    }

    #[test]
    fn array_bindings_allow_bare_and_modified_keys() {
        let config: Config = toml::from_str(
            r#"
[keys]
next_tab = ["n", "ctrl+alt+]"]
"#,
        )
        .unwrap();
        let kb = config.keybinds();
        assert_eq!(
            binding_combos(&kb.next_tab),
            vec![
                (KeyCode::Char('n'), KeyModifiers::empty()),
                (
                    KeyCode::Char(']'),
                    KeyModifiers::CONTROL | KeyModifiers::ALT
                ),
            ]
        );
        assert_eq!(kb.next_tab.label().as_deref(), Some("n / ctrl+alt+]"));
    }

    #[test]
    fn printable_bare_bindings_are_allowed_in_vim_mode() {
        let config: Config = toml::from_str(
            r#"
[keys]
new_tab = "c"
close_tab = "X"
"#,
        )
        .unwrap();
        assert!(config.collect_diagnostics().is_empty());
        let kb = config.keybinds();
        assert!(!kb.new_tab.bindings.is_empty());
        assert!(!kb.close_tab.bindings.is_empty());
    }

    #[test]
    fn unicode_bindings_match_non_us_keys() {
        for ch in ['ğ', 'ç', 'ş', 'ı', 'é', 'ø'] {
            let bindings = ActionKeybinds::direct(&ch.to_string());
            assert!(
                bindings.matches_key(&TerminalKey::new(KeyCode::Char(ch), KeyModifiers::empty(),))
            );
        }
    }

    #[test]
    fn shifted_unicode_bindings_match_layout_aware_input() {
        for (base, shifted) in [('ğ', 'Ğ'), ('ç', 'Ç'), ('ş', 'Ş'), ('ı', 'I'), ('ø', 'Ø')]
        {
            let bindings = ActionKeybinds::direct(&format!("shift+{base}"));
            assert!(bindings.matches_key(
                &TerminalKey::new(KeyCode::Char(base), KeyModifiers::SHIFT)
                    .with_shifted_codepoint(shifted as u32)
            ));
        }
    }

    #[test]
    fn shifted_letter_binding_matches_uppercase_key_event() {
        let bindings = ActionKeybinds::direct("shift+n");
        assert!(bindings.matches_key(&TerminalKey::new(KeyCode::Char('N'), KeyModifiers::SHIFT)));
    }

    #[test]
    fn shifted_letter_binding_matches_legacy_uppercase_key_event() {
        let bindings = ActionKeybinds::direct("shift+n");
        assert!(bindings.matches_key(&TerminalKey::new(KeyCode::Char('N'), KeyModifiers::empty(),)));
    }

    #[test]
    fn binding_matches_modern_modified_key_event() {
        let bindings = ActionKeybinds::direct("cmd+shift+j");
        assert!(bindings.matches_key(&TerminalKey::new(
            KeyCode::Char('J'),
            KeyModifiers::SUPER | KeyModifiers::SHIFT,
        )));
    }

    #[test]
    fn legacy_uppercase_key_event_does_not_match_unshifted_letter_binding() {
        let bindings = ActionKeybinds::direct("n");
        assert!(
            !bindings.matches_key(&TerminalKey::new(KeyCode::Char('N'), KeyModifiers::empty(),))
        );
    }

    #[test]
    fn legacy_uppercase_shift_fallback_is_limited_to_ascii_letters() {
        let shifted_number = ActionKeybinds::direct("shift+1");
        assert!(!shifted_number
            .matches_key(&TerminalKey::new(KeyCode::Char('!'), KeyModifiers::empty(),)));

        let shifted_non_ascii = ActionKeybinds::direct("shift+ö");
        assert!(!shifted_non_ascii
            .matches_key(&TerminalKey::new(KeyCode::Char('Ö'), KeyModifiers::empty(),)));
    }

    #[test]
    fn shifted_tab_inputs_match_backtab_canonical_binding() {
        let bindings = ActionKeybinds::direct("shift+tab");
        assert!(bindings.matches_key(&TerminalKey::new(KeyCode::BackTab, KeyModifiers::empty())));
        assert!(bindings.matches_key(&TerminalKey::new(KeyCode::BackTab, KeyModifiers::SHIFT)));
        assert!(bindings.matches_key(&TerminalKey::new(KeyCode::Tab, KeyModifiers::SHIFT)));
        assert!(!ActionKeybinds::direct("tab")
            .matches_key(&TerminalKey::new(KeyCode::Tab, KeyModifiers::SHIFT)));
        assert_eq!(
            normalize_key_combo((KeyCode::Tab, KeyModifiers::CONTROL | KeyModifiers::SHIFT)),
            (KeyCode::BackTab, KeyModifiers::CONTROL)
        );
    }

    #[test]
    fn format_modified_backtab_keeps_shift_label() {
        assert_eq!(
            format_key_combo((KeyCode::BackTab, KeyModifiers::CONTROL)),
            "ctrl+shift+tab"
        );
        assert_eq!(
            format_key_combo((KeyCode::BackTab, KeyModifiers::CONTROL | KeyModifiers::ALT)),
            "ctrl+alt+shift+tab"
        );
    }

    #[test]
    fn shifted_punctuation_matches_enhanced_input() {
        let help = ActionKeybinds::direct("?");
        assert!(help.matches_key(&TerminalKey::new(KeyCode::Char('?'), KeyModifiers::SHIFT)));
        assert!(help.matches_key(
            &TerminalKey::new(KeyCode::Char('/'), KeyModifiers::SHIFT)
                .with_shifted_codepoint('?' as u32)
        ));

        let bang = ActionKeybinds::direct("!");
        assert!(bang.matches_key(
            &TerminalKey::new(KeyCode::Char('1'), KeyModifiers::SHIFT)
                .with_shifted_codepoint('!' as u32)
        ));
    }

    #[test]
    fn custom_command_bare_key_is_allowed() {
        let config: Config = toml::from_str(
            r#"
[keys]

[[keys.command]]
key = "g"
command = "echo hi"
"#,
        )
        .unwrap();
        assert!(config.collect_diagnostics().is_empty());
        assert_eq!(config.keybinds().custom_commands.len(), 1);
    }

    #[test]
    fn custom_binding_conflicting_with_builtin_is_disabled() {
        let config: Config = toml::from_str(
            r#"
[keys]
new_tab = "ctrl+alt+g"

[[keys.command]]
key = "ctrl+alt+g"
command = "echo no"
"#,
        )
        .unwrap();
        let diagnostics = config.collect_diagnostics();
        let keybinds = config.keybinds();
        assert!(!keybinds.new_tab.bindings.is_empty());
        assert!(keybinds.custom_commands.is_empty());
        assert!(diagnostics.iter().any(|diag| {
            diag.contains("kept keys.new_tab") && diag.contains("disabled keys.command[0].key")
        }));
    }

    #[test]
    fn indexed_range_bindings_support_modifiers() {
        let config: Config = toml::from_str(
            r#"
[keys]
switch_workspace = "shift+1..9"
"#,
        )
        .unwrap();
        let kb = config.keybinds();
        assert_eq!(kb.switch_workspace.len(), 9);
        assert_eq!(
            kb.switch_workspace[0].combo,
            (KeyCode::Char('1'), KeyModifiers::SHIFT)
        );
        assert_eq!(kb.switch_workspace[0].label, "shift+1");
    }

    #[test]
    fn legacy_indexed_user_bindings_displace_modern_defaults() {
        let config: Config = toml::from_str(
            r#"
[keys.indexed]
workspaces = "ctrl"
"#,
        )
        .unwrap();

        let diagnostics = config.collect_diagnostics();
        let kb = config.keybinds();

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert_eq!(kb.switch_workspace.len(), 9);
        assert_eq!(
            kb.switch_workspace[0].combo,
            (KeyCode::Char('1'), KeyModifiers::CONTROL)
        );
        assert_eq!(kb.switch_workspace[0].label, "ctrl+1");
    }

    #[test]
    fn invalid_legacy_indexed_user_binding_displaces_modern_default() {
        let config: Config = toml::from_str(
            r#"
[keys.indexed]
tabs = "bogus"
"#,
        )
        .unwrap();

        let diagnostics = config.collect_diagnostics();
        let kb = config.keybinds();

        assert!(kb.switch_tab.is_empty());
        assert!(diagnostics.iter().any(|diag| {
            diag.contains("invalid indexed keybinding") && diag.contains("keys.indexed.tabs")
        }));
    }

    #[test]
    fn invalid_indexed_binding_does_not_displace_default_binding() {
        let config: Config = toml::from_str(
            r#"
[keys]
switch_tab = "?"
"#,
        )
        .unwrap();

        let diagnostics = config.collect_diagnostics();
        let kb = config.keybinds();

        assert!(kb.switch_tab.is_empty());
        assert_eq!(
            binding_combos(&kb.help),
            vec![(KeyCode::Char('?'), KeyModifiers::empty())]
        );
        assert!(diagnostics.iter().any(|diag| {
            diag.contains("indexed keybinding must use 1..9") && diag.contains("keys.switch_tab")
        }));
        assert!(!diagnostics.iter().any(|diag| {
            diag.contains("kept keys.switch_tab") && diag.contains("disabled keys.help")
        }));
    }

    #[test]
    fn default_keymap_is_bare_and_tab_centered() {
        let kb = Config::default().keybinds();
        assert_eq!(
            binding_combos(&kb.next_tab),
            vec![(KeyCode::Char('n'), KeyModifiers::empty())]
        );
        assert_eq!(
            binding_combos(&kb.previous_tab),
            vec![(KeyCode::Char('p'), KeyModifiers::empty())]
        );
        assert_eq!(kb.switch_tab.len(), 9);
        assert_eq!(
            binding_combos(&kb.new_tab),
            vec![(KeyCode::Char('c'), KeyModifiers::empty())]
        );
        assert_eq!(
            binding_combos(&kb.swap_pane_left),
            vec![(KeyCode::Char('h'), KeyModifiers::SHIFT)]
        );
        assert_eq!(
            binding_combos(&kb.swap_pane_down),
            vec![(KeyCode::Char('j'), KeyModifiers::SHIFT)]
        );
        assert_eq!(
            binding_combos(&kb.swap_pane_up),
            vec![(KeyCode::Char('k'), KeyModifiers::SHIFT)]
        );
        assert_eq!(
            binding_combos(&kb.swap_pane_right),
            vec![(KeyCode::Char('l'), KeyModifiers::SHIFT)]
        );
    }

    #[test]
    fn duplicate_binding_disables_later_binding() {
        let config: Config = toml::from_str(
            r#"
[keys]
next_tab = "n"
new_workspace = "n"
"#,
        )
        .unwrap();
        let diagnostics = config.collect_diagnostics();
        let kb = config.keybinds();
        assert!(kb.next_tab.bindings.is_empty() || kb.new_workspace.bindings.is_empty());
        assert!(diagnostics.iter().any(|diag| {
            diag.contains("kept keys.new_workspace") && diag.contains("disabled keys.next_tab")
        }));
    }

    #[test]
    fn user_binding_silently_displaces_default_binding() {
        let config: Config = toml::from_str(
            r#"
[keys]
previous_workspace = "shift+l"
"#,
        )
        .unwrap();

        let diagnostics = config.collect_diagnostics();
        let kb = config.keybinds();

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert_eq!(
            binding_combos(&kb.previous_workspace),
            vec![(KeyCode::Char('l'), KeyModifiers::SHIFT)]
        );
        assert!(kb.swap_pane_right.bindings.is_empty());
    }

    #[test]
    fn duplicate_user_binding_still_reports_conflict() {
        let config: Config = toml::from_str(
            r#"
[keys]
previous_workspace = "shift+l"
swap_pane_right = "shift+l"
"#,
        )
        .unwrap();

        let diagnostics = config.collect_diagnostics();
        let kb = config.keybinds();

        assert_eq!(
            binding_combos(&kb.previous_workspace),
            vec![(KeyCode::Char('l'), KeyModifiers::SHIFT)]
        );
        assert!(kb.swap_pane_right.bindings.is_empty());
        assert!(diagnostics.iter().any(|diag| {
            diag.contains("kept keys.previous_workspace")
                && diag.contains("disabled keys.swap_pane_right")
        }));
    }

    #[test]
    fn custom_command_with_description_parses() {
        let config: Config = toml::from_str(
            r#"
[[keys.command]]
key = "y"
command = "echo hello"
description = "say hello"
"#,
        )
        .unwrap();
        let keybinds = config.keybinds();
        assert_eq!(keybinds.custom_commands.len(), 1);
        assert_eq!(
            keybinds.custom_commands[0].description,
            Some("say hello".to_string())
        );
    }

    #[test]
    fn custom_popup_command_parses() {
        let config: Config = toml::from_str(
            r#"
[[keys.command]]
key = "g"
command = "lazygit"
type = "popup"
width = 90
height = "80%"
"#,
        )
        .unwrap();
        let keybinds = config.keybinds();
        assert_eq!(keybinds.custom_commands.len(), 1);
        assert_eq!(
            keybinds.custom_commands[0].action,
            CustomCommandAction::Popup
        );
        assert_eq!(
            keybinds.custom_commands[0].width,
            Some(PopupSize::Cells(90))
        );
        assert_eq!(
            keybinds.custom_commands[0].height,
            Some(PopupSize::Percent(80))
        );
    }

    #[test]
    fn non_popup_custom_command_ignores_popup_size_with_diagnostic() {
        let config: Config = toml::from_str(
            r#"
[[keys.command]]
key = "g"
command = "lazygit"
type = "pane"
width = "80%"
"#,
        )
        .unwrap();

        let keybinds = config.keybinds();
        assert_eq!(keybinds.custom_commands[0].width, None);
        assert!(config
            .collect_diagnostics()
            .iter()
            .any(|diag| diag.contains("popup size on non-popup custom command")));
    }
}
