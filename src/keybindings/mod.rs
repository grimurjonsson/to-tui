use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::str::FromStr;

/// All bindable actions in the application
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    // Navigation
    MoveUp,
    MoveDown,

    // Visual mode
    ToggleVisual,
    ExitVisual,

    // Item manipulation
    ToggleState,
    CycleState,
    Delete,
    NewItem,
    NewItemSameLevel,
    InsertItemAbove,

    // Editing
    EnterEditMode,

    // Indentation (single item)
    Indent,
    Outdent,

    // Indentation with children
    IndentWithChildren,
    OutdentWithChildren,

    // Move items
    MoveItemUp,
    MoveItemDown,

    // Collapse/expand
    ToggleCollapse,
    Expand,
    CollapseOrParent,

    // Undo
    Undo,

    // UI
    ToggleHelp,
    CloseHelp,
    Quit,

    // Day navigation
    PrevDay,
    NextDay,
    GoToToday,

    // Edit mode specific
    EditCancel,
    EditConfirm,
    EditBackspace,
    EditLeft,
    EditRight,
    EditHome,
    EditEnd,
    EditIndent,
    EditOutdent,
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Action::MoveUp => "move_up",
            Action::MoveDown => "move_down",
            Action::ToggleVisual => "toggle_visual",
            Action::ExitVisual => "exit_visual",
            Action::ToggleState => "toggle_state",
            Action::CycleState => "cycle_state",
            Action::Delete => "delete",
            Action::NewItem => "new_item",
            Action::NewItemSameLevel => "new_item_same_level",
            Action::InsertItemAbove => "insert_item_above",
            Action::EnterEditMode => "enter_edit_mode",
            Action::Indent => "indent",
            Action::Outdent => "outdent",
            Action::IndentWithChildren => "indent_with_children",
            Action::OutdentWithChildren => "outdent_with_children",
            Action::MoveItemUp => "move_item_up",
            Action::MoveItemDown => "move_item_down",
            Action::ToggleCollapse => "toggle_collapse",
            Action::Expand => "expand",
            Action::CollapseOrParent => "collapse_or_parent",
            Action::Undo => "undo",
            Action::ToggleHelp => "toggle_help",
            Action::CloseHelp => "close_help",
            Action::Quit => "quit",
            Action::PrevDay => "prev_day",
            Action::NextDay => "next_day",
            Action::GoToToday => "go_to_today",
            Action::EditCancel => "edit_cancel",
            Action::EditConfirm => "edit_confirm",
            Action::EditBackspace => "edit_backspace",
            Action::EditLeft => "edit_left",
            Action::EditRight => "edit_right",
            Action::EditHome => "edit_home",
            Action::EditEnd => "edit_end",
            Action::EditIndent => "edit_indent",
            Action::EditOutdent => "edit_outdent",
        };
        write!(f, "{s}")
    }
}

impl FromStr for Action {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "move_up" => Ok(Action::MoveUp),
            "move_down" => Ok(Action::MoveDown),
            "toggle_visual" => Ok(Action::ToggleVisual),
            "exit_visual" => Ok(Action::ExitVisual),
            "toggle_state" => Ok(Action::ToggleState),
            "cycle_state" => Ok(Action::CycleState),
            "delete" => Ok(Action::Delete),
            "new_item" => Ok(Action::NewItem),
            "new_item_same_level" => Ok(Action::NewItemSameLevel),
            "insert_item_above" => Ok(Action::InsertItemAbove),
            "enter_edit_mode" => Ok(Action::EnterEditMode),
            "indent" => Ok(Action::Indent),
            "outdent" => Ok(Action::Outdent),
            "indent_with_children" => Ok(Action::IndentWithChildren),
            "outdent_with_children" => Ok(Action::OutdentWithChildren),
            "move_item_up" => Ok(Action::MoveItemUp),
            "move_item_down" => Ok(Action::MoveItemDown),
            "toggle_collapse" => Ok(Action::ToggleCollapse),
            "expand" => Ok(Action::Expand),
            "collapse_or_parent" => Ok(Action::CollapseOrParent),
            "undo" => Ok(Action::Undo),
            "toggle_help" => Ok(Action::ToggleHelp),
            "close_help" => Ok(Action::CloseHelp),
            "quit" => Ok(Action::Quit),
            "prev_day" => Ok(Action::PrevDay),
            "next_day" => Ok(Action::NextDay),
            "go_to_today" => Ok(Action::GoToToday),
            "edit_cancel" => Ok(Action::EditCancel),
            "edit_confirm" => Ok(Action::EditConfirm),
            "edit_backspace" => Ok(Action::EditBackspace),
            "edit_left" => Ok(Action::EditLeft),
            "edit_right" => Ok(Action::EditRight),
            "edit_home" => Ok(Action::EditHome),
            "edit_end" => Ok(Action::EditEnd),
            "edit_indent" => Ok(Action::EditIndent),
            "edit_outdent" => Ok(Action::EditOutdent),
            _ => Err(format!("Unknown action: {s}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyBinding {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

impl KeyBinding {
    pub fn new(code: KeyCode, modifiers: KeyModifiers) -> Self {
        Self { code, modifiers }
    }

    pub fn from_event(event: &KeyEvent) -> Self {
        match event.code {
            KeyCode::BackTab => Self {
                code: event.code,
                modifiers: event.modifiers - KeyModifiers::SHIFT,
            },
            KeyCode::Char(c) if event.modifiers.contains(KeyModifiers::SHIFT) => {
                let normalized = match c {
                    ',' => '<',
                    '.' => '>',
                    '<' => '>',
                    c if c.is_ascii_lowercase() => c.to_ascii_uppercase(),
                    c => c,
                };
                Self {
                    code: KeyCode::Char(normalized),
                    modifiers: event.modifiers - KeyModifiers::SHIFT,
                }
            }
            KeyCode::Char(c) if c.is_ascii_uppercase() || c == '<' || c == '>' => Self {
                code: event.code,
                modifiers: event.modifiers - KeyModifiers::SHIFT,
            },
            _ => Self {
                code: event.code,
                modifiers: event.modifiers,
            },
        }
    }
}

impl fmt::Display for KeyBinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();

        if self.modifiers.contains(KeyModifiers::CONTROL) {
            parts.push("C");
        }
        if self.modifiers.contains(KeyModifiers::ALT) {
            parts.push("A");
        }
        if self.modifiers.contains(KeyModifiers::SHIFT) {
            parts.push("S");
        }

        let key_str = match self.code {
            KeyCode::Char(' ') => "Space".to_string(),
            KeyCode::Char(c) => c.to_string(),
            KeyCode::Up => "Up".to_string(),
            KeyCode::Down => "Down".to_string(),
            KeyCode::Left => "Left".to_string(),
            KeyCode::Right => "Right".to_string(),
            KeyCode::Tab => "Tab".to_string(),
            KeyCode::BackTab => "S-Tab".to_string(),
            KeyCode::Enter => "Enter".to_string(),
            KeyCode::Esc => "Esc".to_string(),
            KeyCode::Backspace => "BS".to_string(),
            KeyCode::Home => "Home".to_string(),
            KeyCode::End => "End".to_string(),
            KeyCode::Delete => "Del".to_string(),
            KeyCode::F(n) => format!("F{n}"),
            _ => format!("{:?}", self.code),
        };

        parts.push(&key_str);

        if parts.len() > 1 || key_str.len() > 1 {
            write!(f, "<{}>", parts.join("-"))
        } else {
            write!(f, "{key_str}")
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeySequence(pub Vec<KeyBinding>);

impl KeySequence {
    pub fn is_single(&self) -> bool {
        self.0.len() == 1
    }
}

/// Parse key sequence: "d", "dd", "<C-d>", "<C-d><C-d>", "g g", etc.
impl FromStr for KeySequence {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        let mut keys = Vec::new();
        let mut chars = s.chars().peekable();

        while chars.peek().is_some() {
            while chars.peek() == Some(&' ') {
                chars.next();
            }

            if chars.peek().is_none() {
                break;
            }

            if chars.peek() == Some(&'<') {
                let mut bracket_content = String::new();
                bracket_content.push(chars.next().unwrap());

                while let Some(&c) = chars.peek() {
                    bracket_content.push(chars.next().unwrap());
                    if c == '>' {
                        break;
                    }
                }

                keys.push(bracket_content.parse::<KeyBinding>()?);
            } else {
                let c = chars.next().unwrap();
                keys.push(KeyBinding::new(KeyCode::Char(c), KeyModifiers::NONE));
            }
        }

        if keys.is_empty() {
            return Err("Empty key sequence".to_string());
        }

        if keys.len() > 2 {
            return Err("Key sequences longer than 2 are not supported".to_string());
        }

        Ok(KeySequence(keys))
    }
}

impl FromStr for KeyBinding {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();

        if s.starts_with('<') && s.ends_with('>') {
            let inner = &s[1..s.len() - 1];
            return parse_bracket_notation(inner);
        }

        if s.len() == 1 {
            let c = s.chars().next().unwrap();
            return Ok(KeyBinding::new(KeyCode::Char(c), KeyModifiers::NONE));
        }

        Err(format!("Invalid key binding: {s}"))
    }
}

fn parse_bracket_notation(s: &str) -> Result<KeyBinding, String> {
    let parts: Vec<&str> = s.split('-').collect();

    let mut modifiers = KeyModifiers::NONE;
    let mut key_part = "";

    for (i, part) in parts.iter().enumerate() {
        let part_upper = part.to_uppercase();
        if i == parts.len() - 1 {
            key_part = part;
        } else {
            match part_upper.as_str() {
                "C" | "CTRL" | "CONTROL" => modifiers |= KeyModifiers::CONTROL,
                "A" | "ALT" | "M" | "META" => modifiers |= KeyModifiers::ALT,
                "S" | "SHIFT" => modifiers |= KeyModifiers::SHIFT,
                _ => return Err(format!("Unknown modifier: {part}")),
            }
        }
    }

    let code = parse_key_code(key_part)?;

    Ok(KeyBinding::new(code, modifiers))
}

fn parse_key_code(s: &str) -> Result<KeyCode, String> {
    let s_lower = s.to_lowercase();

    match s_lower.as_str() {
        "space" => Ok(KeyCode::Char(' ')),
        "tab" => Ok(KeyCode::Tab),
        "backtab" => Ok(KeyCode::BackTab),
        "enter" | "return" | "cr" => Ok(KeyCode::Enter),
        "esc" | "escape" => Ok(KeyCode::Esc),
        "bs" | "backspace" => Ok(KeyCode::Backspace),
        "up" => Ok(KeyCode::Up),
        "down" => Ok(KeyCode::Down),
        "left" => Ok(KeyCode::Left),
        "right" => Ok(KeyCode::Right),
        "home" => Ok(KeyCode::Home),
        "end" => Ok(KeyCode::End),
        "del" | "delete" => Ok(KeyCode::Delete),
        "pageup" | "pgup" => Ok(KeyCode::PageUp),
        "pagedown" | "pgdn" => Ok(KeyCode::PageDown),
        s if s.starts_with('f') && s.len() > 1 => {
            let n: u8 = s[1..].parse().map_err(|_| format!("Invalid F key: {s}"))?;
            Ok(KeyCode::F(n))
        }
        s if s.len() == 1 => {
            let c = s.chars().next().unwrap();
            Ok(KeyCode::Char(c))
        }
        _ => Err(format!("Unknown key: {s}")),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyLookupResult {
    Action(Action),
    Pending,
    None,
}

#[derive(Debug, Clone)]
pub struct KeybindingCache {
    navigate_single: HashMap<KeyBinding, Action>,
    navigate_sequences: HashMap<KeyBinding, HashMap<KeyBinding, Action>>,
    navigate_sequence_starters: HashSet<KeyBinding>,

    edit_single: HashMap<KeyBinding, Action>,

    visual_single: HashMap<KeyBinding, Action>,
}

impl KeybindingCache {
    pub fn from_config(config: &KeybindingsConfig) -> Self {
        let mut navigate_single = HashMap::new();
        let mut navigate_sequences: HashMap<KeyBinding, HashMap<KeyBinding, Action>> =
            HashMap::new();
        let mut navigate_sequence_starters = HashSet::new();
        let mut edit_single = HashMap::new();

        for (key_str, action_str) in &config.navigate {
            if let (Ok(seq), Ok(action)) =
                (key_str.parse::<KeySequence>(), action_str.parse::<Action>())
            {
                if seq.is_single() {
                    navigate_single.insert(seq.0[0], action);
                } else {
                    let first = seq.0[0];
                    let second = seq.0[1];
                    navigate_sequence_starters.insert(first);
                    navigate_sequences
                        .entry(first)
                        .or_default()
                        .insert(second, action);
                }
            }
        }

        for (key_str, action_str) in &config.edit {
            if let (Ok(seq), Ok(action)) =
                (key_str.parse::<KeySequence>(), action_str.parse::<Action>())
            {
                if seq.is_single() {
                    edit_single.insert(seq.0[0], action);
                }
            }
        }

        let mut visual_single = HashMap::new();
        for (key_str, action_str) in &config.visual {
            if let (Ok(seq), Ok(action)) =
                (key_str.parse::<KeySequence>(), action_str.parse::<Action>())
            {
                if seq.is_single() {
                    visual_single.insert(seq.0[0], action);
                }
            }
        }

        Self {
            navigate_single,
            navigate_sequences,
            navigate_sequence_starters,
            edit_single,
            visual_single,
        }
    }

    pub fn lookup_navigate(
        &self,
        event: &KeyEvent,
        pending: Option<KeyBinding>,
    ) -> KeyLookupResult {
        let binding = KeyBinding::from_event(event);

        if let Some(first_key) = pending {
            if let Some(second_map) = self.navigate_sequences.get(&first_key) {
                if let Some(&action) = second_map.get(&binding) {
                    return KeyLookupResult::Action(action);
                }
            }
            return KeyLookupResult::None;
        }

        if self.navigate_sequence_starters.contains(&binding) {
            if !self.navigate_single.contains_key(&binding) {
                return KeyLookupResult::Pending;
            }
            return KeyLookupResult::Pending;
        }

        if let Some(&action) = self.navigate_single.get(&binding) {
            return KeyLookupResult::Action(action);
        }

        KeyLookupResult::None
    }

    pub fn get_edit_action(&self, event: &KeyEvent) -> Option<Action> {
        let binding = KeyBinding::from_event(event);
        self.edit_single.get(&binding).copied()
    }

    pub fn get_visual_action(&self, event: &KeyEvent) -> Option<Action> {
        let binding = KeyBinding::from_event(event);
        self.visual_single.get(&binding).copied()
    }
}

impl Default for KeybindingCache {
    fn default() -> Self {
        Self::from_config(&KeybindingsConfig::default())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeybindingsConfig {
    #[serde(default)]
    pub navigate: HashMap<String, String>,

    #[serde(default)]
    pub edit: HashMap<String, String>,

    #[serde(default)]
    pub visual: HashMap<String, String>,
}

impl KeybindingsConfig {
    pub fn merge_with_defaults(mut self) -> Self {
        let defaults = Self::default();

        for (key, value) in defaults.navigate {
            self.navigate.entry(key).or_insert(value);
        }
        for (key, value) in defaults.edit {
            self.edit.entry(key).or_insert(value);
        }
        for (key, value) in defaults.visual {
            self.visual.entry(key).or_insert(value);
        }

        self
    }
}

impl Default for KeybindingsConfig {
    fn default() -> Self {
        Self {
            navigate: default_navigate_bindings(),
            edit: default_edit_bindings(),
            visual: default_visual_bindings(),
        }
    }
}

fn default_navigate_bindings() -> HashMap<String, String> {
    let mut m = HashMap::new();

    m.insert("k".to_string(), "move_up".to_string());
    m.insert("j".to_string(), "move_down".to_string());
    m.insert("<Up>".to_string(), "move_up".to_string());
    m.insert("<Down>".to_string(), "move_down".to_string());
    m.insert("v".to_string(), "toggle_visual".to_string());
    m.insert("x".to_string(), "toggle_state".to_string());
    m.insert("<Space>".to_string(), "cycle_state".to_string());
    m.insert("dd".to_string(), "delete".to_string());
    m.insert("n".to_string(), "new_item".to_string());
    m.insert("o".to_string(), "new_item".to_string());
    m.insert("<Enter>".to_string(), "new_item_same_level".to_string());
    m.insert("O".to_string(), "insert_item_above".to_string());
    m.insert("<S-Enter>".to_string(), "insert_item_above".to_string());
    m.insert("<C-j>".to_string(), "insert_item_above".to_string());
    m.insert("i".to_string(), "enter_edit_mode".to_string());
    m.insert("<Tab>".to_string(), "indent".to_string());
    m.insert("<BackTab>".to_string(), "outdent".to_string());
    m.insert(
        "<S-A-Right>".to_string(),
        "indent_with_children".to_string(),
    );
    m.insert(
        "<S-A-Left>".to_string(),
        "outdent_with_children".to_string(),
    );
    m.insert("<S-A-Up>".to_string(), "move_item_up".to_string());
    m.insert("<S-A-Down>".to_string(), "move_item_down".to_string());
    m.insert("c".to_string(), "toggle_collapse".to_string());
    m.insert("<Right>".to_string(), "expand".to_string());
    m.insert("l".to_string(), "expand".to_string());
    m.insert("<Left>".to_string(), "collapse_or_parent".to_string());
    m.insert("h".to_string(), "collapse_or_parent".to_string());
    m.insert("u".to_string(), "undo".to_string());
    m.insert("?".to_string(), "toggle_help".to_string());
    m.insert("<Esc>".to_string(), "close_help".to_string());
    m.insert("q".to_string(), "quit".to_string());
    m.insert("<".to_string(), "prev_day".to_string());
    m.insert(">".to_string(), "next_day".to_string());
    m.insert("T".to_string(), "go_to_today".to_string());

    m
}

fn default_edit_bindings() -> HashMap<String, String> {
    let mut m = HashMap::new();

    m.insert("<Esc>".to_string(), "edit_cancel".to_string());
    m.insert("<Enter>".to_string(), "edit_confirm".to_string());
    m.insert("<BS>".to_string(), "edit_backspace".to_string());
    m.insert("<Left>".to_string(), "edit_left".to_string());
    m.insert("<Right>".to_string(), "edit_right".to_string());
    m.insert("<Home>".to_string(), "edit_home".to_string());
    m.insert("<End>".to_string(), "edit_end".to_string());
    m.insert("<Tab>".to_string(), "edit_indent".to_string());
    m.insert("<BackTab>".to_string(), "edit_outdent".to_string());

    m
}

fn default_visual_bindings() -> HashMap<String, String> {
    let mut m = HashMap::new();

    m.insert("k".to_string(), "move_up".to_string());
    m.insert("j".to_string(), "move_down".to_string());
    m.insert("<Up>".to_string(), "move_up".to_string());
    m.insert("<Down>".to_string(), "move_down".to_string());
    m.insert("<Tab>".to_string(), "indent".to_string());
    m.insert("<BackTab>".to_string(), "outdent".to_string());
    m.insert("u".to_string(), "undo".to_string());
    m.insert("v".to_string(), "exit_visual".to_string());
    m.insert("<Esc>".to_string(), "exit_visual".to_string());
    m.insert("q".to_string(), "exit_visual".to_string());

    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_key() {
        let binding: KeyBinding = "j".parse().unwrap();
        assert_eq!(binding.code, KeyCode::Char('j'));
        assert_eq!(binding.modifiers, KeyModifiers::NONE);
    }

    #[test]
    fn test_parse_special_key() {
        let binding: KeyBinding = "<Space>".parse().unwrap();
        assert_eq!(binding.code, KeyCode::Char(' '));
        assert_eq!(binding.modifiers, KeyModifiers::NONE);
    }

    #[test]
    fn test_parse_modifier_key() {
        let binding: KeyBinding = "<C-d>".parse().unwrap();
        assert_eq!(binding.code, KeyCode::Char('d'));
        assert!(binding.modifiers.contains(KeyModifiers::CONTROL));
    }

    #[test]
    fn test_parse_multi_modifier() {
        let binding: KeyBinding = "<S-A-Up>".parse().unwrap();
        assert_eq!(binding.code, KeyCode::Up);
        assert!(binding.modifiers.contains(KeyModifiers::SHIFT));
        assert!(binding.modifiers.contains(KeyModifiers::ALT));
    }

    #[test]
    fn test_parse_sequence_single() {
        let seq: KeySequence = "j".parse().unwrap();
        assert!(seq.is_single());
        assert_eq!(seq.0[0].code, KeyCode::Char('j'));
    }

    #[test]
    fn test_parse_sequence_double() {
        let seq: KeySequence = "dd".parse().unwrap();
        assert!(!seq.is_single());
        assert_eq!(seq.0.len(), 2);
        assert_eq!(seq.0[0].code, KeyCode::Char('d'));
        assert_eq!(seq.0[1].code, KeyCode::Char('d'));
    }

    #[test]
    fn test_parse_sequence_with_space() {
        let seq: KeySequence = "g g".parse().unwrap();
        assert!(!seq.is_single());
        assert_eq!(seq.0[0].code, KeyCode::Char('g'));
        assert_eq!(seq.0[1].code, KeyCode::Char('g'));
    }

    #[test]
    fn test_parse_sequence_brackets() {
        let seq: KeySequence = "<C-d><C-d>".parse().unwrap();
        assert!(!seq.is_single());
        assert_eq!(seq.0[0].code, KeyCode::Char('d'));
        assert!(seq.0[0].modifiers.contains(KeyModifiers::CONTROL));
        assert_eq!(seq.0[1].code, KeyCode::Char('d'));
        assert!(seq.0[1].modifiers.contains(KeyModifiers::CONTROL));
    }

    #[test]
    fn test_cache_single_lookup() {
        let cache = KeybindingCache::default();

        let event = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
        let result = cache.lookup_navigate(&event, None);
        assert_eq!(result, KeyLookupResult::Action(Action::MoveDown));
    }

    #[test]
    fn test_cache_sequence_lookup() {
        let cache = KeybindingCache::default();

        let d_event = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE);
        let d_binding = KeyBinding::from_event(&d_event);

        let result1 = cache.lookup_navigate(&d_event, None);
        assert_eq!(result1, KeyLookupResult::Pending);

        let result2 = cache.lookup_navigate(&d_event, Some(d_binding));
        assert_eq!(result2, KeyLookupResult::Action(Action::Delete));
    }

    #[test]
    fn test_action_roundtrip() {
        let action = Action::MoveUp;
        let s = action.to_string();
        let parsed: Action = s.parse().unwrap();
        assert_eq!(action, parsed);
    }

    #[test]
    fn test_parse_angle_brackets() {
        let seq: KeySequence = "<".parse().unwrap();
        assert!(seq.is_single());
        assert_eq!(seq.0[0].code, KeyCode::Char('<'));
        assert_eq!(seq.0[0].modifiers, KeyModifiers::NONE);

        let seq: KeySequence = ">".parse().unwrap();
        assert!(seq.is_single());
        assert_eq!(seq.0[0].code, KeyCode::Char('>'));
        assert_eq!(seq.0[0].modifiers, KeyModifiers::NONE);
    }

    #[test]
    fn test_cache_angle_bracket_lookup() {
        let cache = KeybindingCache::default();

        let event = KeyEvent::new(KeyCode::Char('<'), KeyModifiers::NONE);
        let result = cache.lookup_navigate(&event, None);
        assert_eq!(result, KeyLookupResult::Action(Action::PrevDay));

        let event = KeyEvent::new(KeyCode::Char('>'), KeyModifiers::NONE);
        let result = cache.lookup_navigate(&event, None);
        assert_eq!(result, KeyLookupResult::Action(Action::NextDay));
    }

    #[test]
    fn test_cache_shifted_comma_period_lookup() {
        let cache = KeybindingCache::default();

        let event = KeyEvent::new(KeyCode::Char(','), KeyModifiers::SHIFT);
        let result = cache.lookup_navigate(&event, None);
        assert_eq!(result, KeyLookupResult::Action(Action::PrevDay));

        let event = KeyEvent::new(KeyCode::Char('.'), KeyModifiers::SHIFT);
        let result = cache.lookup_navigate(&event, None);
        assert_eq!(result, KeyLookupResult::Action(Action::NextDay));

        let event = KeyEvent::new(KeyCode::Char('<'), KeyModifiers::SHIFT);
        let result = cache.lookup_navigate(&event, None);
        assert_eq!(result, KeyLookupResult::Action(Action::NextDay));

        let event = KeyEvent::new(KeyCode::Char('>'), KeyModifiers::SHIFT);
        let result = cache.lookup_navigate(&event, None);
        assert_eq!(result, KeyLookupResult::Action(Action::NextDay));
    }
}
