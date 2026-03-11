//! Interactive keyboard builder for QQ bot messages.
//!
//! Build row-based button layouts using [`KeyboardBuilder`] and attach them
//! to messages via [`MessageBuilder::keyboard`](crate::MessageBuilder::keyboard).

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Button action type
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ButtonAction {
    /// Open a URL
    Jump = 0,
    /// Trigger a callback
    Callback = 1,
    /// Send a command to the input box
    Command = 2,
}

/// Button visual style
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ButtonStyle {
    Grey = 0,
    Blue = 1,
}

/// Who can see/click the button
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ButtonPermission {
    /// Specified users only
    SpecifiedUsers = 0,
    /// Group managers/admins only
    Manager = 1,
    /// Everyone
    All = 2,
    /// Specified roles
    SpecifiedRoles = 3,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Button {
    pub id: Option<String>,
    pub label: String,
    pub visited_label: Option<String>,
    pub action_type: u8,
    pub action_data: String,
    pub style: u8,
    pub permission_type: u8,
    pub specified_user_ids: Vec<String>,
    pub specified_role_ids: Vec<String>,
    pub unsupport_tips: Option<String>,
}

impl Button {
    fn new(label: &str, action: ButtonAction, data: &str) -> Self {
        Self {
            id: None,
            label: label.to_string(),
            visited_label: None,
            action_type: action as u8,
            action_data: data.to_string(),
            style: ButtonStyle::Grey as u8,
            permission_type: ButtonPermission::All as u8,
            specified_user_ids: Vec::new(),
            specified_role_ids: Vec::new(),
            unsupport_tips: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyboardRow {
    pub buttons: Vec<Button>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Keyboard {
    pub rows: Vec<KeyboardRow>,
}

impl Keyboard {
    /// Convert to a message Segment
    pub fn to_segment(&self) -> crate::Segment {
        let json_value = serde_json::to_value(self).unwrap_or(Value::Null);
        let mut data = Map::new();
        data.insert("content".to_string(), json_value);
        crate::Segment {
            kind: "keyboard".to_string(),
            data,
        }
    }
}

/// Builder for creating keyboard layouts
pub struct KeyboardBuilder {
    rows: Vec<KeyboardRow>,
    current_row: Vec<Button>,
}

impl KeyboardBuilder {
    pub fn new() -> Self {
        Self {
            rows: Vec::new(),
            current_row: Vec::new(),
        }
    }

    /// Add a button to the current row
    pub fn button(mut self, label: &str, action: ButtonAction, data: &str) -> Self {
        self.current_row.push(Button::new(label, action, data));
        self
    }

    /// Add a jump (URL) button
    pub fn jump_button(self, label: &str, url: &str) -> Self {
        self.button(label, ButtonAction::Jump, url)
    }

    /// Add a callback button
    pub fn callback_button(self, label: &str, data: &str) -> Self {
        self.button(label, ButtonAction::Callback, data)
    }

    /// Add a command button (sends text to input)
    pub fn command_button(self, label: &str, command: &str) -> Self {
        self.button(label, ButtonAction::Command, command)
    }

    /// Set style on the last added button
    pub fn style(mut self, style: ButtonStyle) -> Self {
        if let Some(btn) = self.current_row.last_mut() {
            btn.style = style as u8;
        }
        self
    }

    /// Set permission on the last added button
    pub fn permission(mut self, perm: ButtonPermission) -> Self {
        if let Some(btn) = self.current_row.last_mut() {
            btn.permission_type = perm as u8;
        }
        self
    }

    /// Finish current row and start a new one
    pub fn row(mut self) -> Self {
        if !self.current_row.is_empty() {
            self.rows.push(KeyboardRow {
                buttons: std::mem::take(&mut self.current_row),
            });
        }
        self
    }

    /// Build the final Keyboard
    pub fn build(mut self) -> Keyboard {
        if !self.current_row.is_empty() {
            self.rows.push(KeyboardRow {
                buttons: self.current_row,
            });
        }
        Keyboard { rows: self.rows }
    }
}

impl Default for KeyboardBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_simple_keyboard() {
        let kb = KeyboardBuilder::new()
            .jump_button("Open", "https://example.com")
            .callback_button("Click", "cb_data")
            .row()
            .command_button("Say Hi", "/hello")
            .style(ButtonStyle::Blue)
            .permission(ButtonPermission::Manager)
            .build();

        assert_eq!(kb.rows.len(), 2);
        assert_eq!(kb.rows[0].buttons.len(), 2);
        assert_eq!(kb.rows[1].buttons.len(), 1);
        assert_eq!(kb.rows[0].buttons[0].action_type, 0);
        assert_eq!(kb.rows[0].buttons[1].action_type, 1);
        assert_eq!(kb.rows[1].buttons[0].action_type, 2);
        assert_eq!(kb.rows[1].buttons[0].style, 1);
        assert_eq!(kb.rows[1].buttons[0].permission_type, 1);
    }

    #[test]
    fn to_segment() {
        let kb = KeyboardBuilder::new()
            .callback_button("OK", "ok")
            .build();

        let seg = kb.to_segment();
        assert_eq!(seg.kind, "keyboard");
        assert!(seg.data.contains_key("content"));
    }
}
