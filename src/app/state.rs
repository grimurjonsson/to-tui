use super::mode::Mode;
use crate::keybindings::{KeyBinding, KeybindingCache};
use crate::plugin::{GeneratorInfo, PluginRegistry};
use crate::storage::file::load_todo_list;
use crate::storage::load_todos_for_viewing;
use crate::storage::UiCache;
use crate::todo::{PriorityCycle, TodoItem, TodoList};
use crate::ui::theme::Theme;
use crate::utils::upgrade::{get_asset_download_url, spawn_download, DownloadProgress, UpgradeSubState};
use crate::utils::version_check::{spawn_version_checker, VersionCheckResult};
use anyhow::Result;
use chrono::{Duration, Local, NaiveDate};
use ratatui::widgets::ListState;
use std::sync::mpsc;
use std::time::Instant;
use tokio::sync::mpsc as tokio_mpsc;
use uuid::Uuid;

const MAX_UNDO_HISTORY: usize = 50;

#[derive(Debug, Clone)]
pub enum PluginSubState {
    Selecting {
        plugins: Vec<GeneratorInfo>,
        selected_index: usize,
    },
    InputPrompt {
        plugin_name: String,
        input_buffer: String,
        cursor_pos: usize,
    },
    Executing {
        plugin_name: String,
    },
    Error {
        message: String,
    },
    Preview {
        items: Vec<TodoItem>,
    },
}

/// Holds data for pending rollover from a previous day
#[derive(Debug, Clone)]
pub struct PendingRollover {
    pub source_date: NaiveDate,
    pub items: Vec<TodoItem>,
}

pub struct AppState {
    pub todo_list: TodoList,
    pub cursor_position: usize,
    pub mode: Mode,
    pub edit_buffer: String,
    pub edit_cursor_pos: usize,
    pub should_quit: bool,
    pub show_help: bool,
    pub theme: Theme,
    pub keybindings: KeybindingCache,
    pub pending_key: Option<KeyBinding>,
    pub pending_key_time: Option<Instant>,
    pub timeoutlen: u64,
    pub unsaved_changes: bool,
    pub last_save_time: Option<Instant>,
    pub is_creating_new_item: bool,
    pub insert_above: bool,
    pub pending_indent_level: usize,
    pub undo_stack: Vec<(TodoList, usize)>,
    pub selection_anchor: Option<usize>,
    pub viewing_date: NaiveDate,
    pub today: NaiveDate,
    pub pending_delete_subtask_count: Option<usize>,
    pub plugin_registry: PluginRegistry,
    pub plugin_state: Option<PluginSubState>,
    pub status_message: Option<(String, Instant)>,
    pub plugin_result_rx: Option<mpsc::Receiver<Result<Vec<TodoItem>, String>>>,
    pub spinner_frame: usize,
    pub pending_rollover: Option<PendingRollover>,
    pub list_state: ListState,
    /// Terminal width, updated on each render for click calculations
    pub terminal_width: u16,
    /// Terminal height, updated on each render for scroll calculations
    pub terminal_height: u16,
    /// Help overlay scroll offset
    pub help_scroll: u16,
    /// New version available (if any)
    pub new_version_available: Option<String>,
    /// Receiver for version check results
    version_check_rx: mpsc::Receiver<VersionCheckResult>,
    /// Session-only flag: dismiss upgrade prompt until app restart
    pub session_dismissed_upgrade: bool,
    /// Version to skip permanently (loaded from config)
    pub skipped_version: Option<String>,
    /// Flag to show upgrade prompt (set when new version detected)
    pub show_upgrade_prompt: bool,
    /// Release URL to print after quit (when user clicks Y)
    pub pending_release_url: Option<String>,
    /// Current upgrade sub-state when in Mode::UpgradePrompt
    pub upgrade_sub_state: Option<UpgradeSubState>,
    /// Channel receiver for download progress updates
    pub download_progress_rx: Option<tokio_mpsc::Receiver<DownloadProgress>>,
}

impl AppState {
    pub fn new(
        todo_list: TodoList,
        theme: Theme,
        keybindings: KeybindingCache,
        timeoutlen: u64,
        plugin_registry: PluginRegistry,
        ui_cache: Option<UiCache>,
        skipped_version: Option<String>,
    ) -> Self {
        let today = Local::now().date_naive();
        let viewing_date = todo_list.date;

        // Find cursor position from cached selected_todo_id
        let cursor_position = ui_cache
            .as_ref()
            .and_then(|cache| cache.selected_todo_id)
            .and_then(|id| Self::find_item_index_by_id(&todo_list, id))
            .unwrap_or(0);

        let mut state = Self {
            todo_list,
            cursor_position,
            mode: Mode::Navigate,
            edit_buffer: String::new(),
            edit_cursor_pos: 0,
            should_quit: false,
            show_help: false,
            theme,
            keybindings,
            pending_key: None,
            pending_key_time: None,
            timeoutlen,
            unsaved_changes: false,
            last_save_time: None,
            is_creating_new_item: false,
            insert_above: false,
            pending_indent_level: 0,
            undo_stack: Vec::new(),
            selection_anchor: None,
            viewing_date,
            today,
            pending_delete_subtask_count: None,
            plugin_registry,
            plugin_state: None,
            status_message: None,
            plugin_result_rx: None,
            spinner_frame: 0,
            pending_rollover: None,
            list_state: ListState::default(),
            terminal_width: 80,  // Default, updated on first render
            terminal_height: 24, // Default, updated on first render
            help_scroll: 0,
            new_version_available: None,
            version_check_rx: spawn_version_checker(),
            session_dismissed_upgrade: false,
            skipped_version,
            show_upgrade_prompt: false,
            pending_release_url: None,
            upgrade_sub_state: None,
            download_progress_rx: None,
        };
        // Sync list state with cursor position
        state.sync_list_state();
        state
    }

    fn find_item_index_by_id(todo_list: &TodoList, id: Uuid) -> Option<usize> {
        todo_list.items.iter().position(|item| item.id == id)
    }

    /// Get the currently selected todo's ID for caching
    pub fn get_selected_todo_id(&self) -> Option<Uuid> {
        self.todo_list.items.get(self.cursor_position).map(|item| item.id)
    }

    pub fn is_readonly(&self) -> bool {
        self.viewing_date != self.today
    }

    /// Returns the count of list items rendered (excluding hidden collapsed children,
    /// but including expanded description boxes which are separate ListItems).
    /// Used for scroll position indicator and scrollbar.
    pub fn visible_item_count(&self) -> usize {
        let hidden = self.todo_list.build_hidden_indices();
        let mut count = 0;
        for (i, item) in self.todo_list.items.iter().enumerate() {
            if hidden.contains(&i) {
                continue;
            }
            count += 1;
            // Expanded descriptions render as a separate ListItem
            if !item.collapsed && item.description.is_some() {
                count += 1;
            }
        }
        count
    }

    /// Sync list_state selection with cursor_position among visible items only.
    /// This calculates the visible index (excluding hidden collapsed children,
    /// but accounting for expanded description boxes which are separate ListItems).
    /// Also adjusts scroll offset to keep selected item visible.
    pub fn sync_list_state(&mut self) {
        let hidden_indices = self.todo_list.build_hidden_indices();
        let mut visible_index = 0;
        for i in 0..self.cursor_position {
            if hidden_indices.contains(&i) {
                continue;
            }
            visible_index += 1;
            // Expanded descriptions render as a separate ListItem
            if !self.todo_list.items[i].collapsed && self.todo_list.items[i].description.is_some() {
                visible_index += 1;
            }
        }
        // Only set selection if cursor is on a visible item
        if !hidden_indices.contains(&self.cursor_position) {
            self.list_state.select(Some(visible_index));

            // Adjust offset to ensure selected item is visible
            // Viewport height = terminal height - borders (2) - status bar (1)
            let viewport_height = self.terminal_height.saturating_sub(3).max(1) as usize;
            let current_offset = self.list_state.offset();

            // If selected item is above viewport, scroll up
            if visible_index < current_offset {
                *self.list_state.offset_mut() = visible_index;
            }
            // If selected item is below viewport, scroll down
            else if visible_index >= current_offset + viewport_height {
                *self.list_state.offset_mut() = visible_index.saturating_sub(viewport_height - 1);
            }
        }
    }

    pub fn navigate_to_date(&mut self, date: NaiveDate) -> Result<()> {
        if date > self.today {
            return Ok(());
        }
        self.todo_list = load_todos_for_viewing(date)?;
        self.viewing_date = date;
        self.cursor_position = 0;
        self.undo_stack.clear();
        self.unsaved_changes = false;
        self.mode = Mode::Navigate;
        self.edit_buffer.clear();
        self.edit_cursor_pos = 0;
        self.is_creating_new_item = false;
        self.insert_above = false;
        self.sync_list_state();
        Ok(())
    }

    pub fn navigate_prev_day(&mut self) -> Result<()> {
        let prev = self.viewing_date - Duration::days(1);
        self.navigate_to_date(prev)
    }

    pub fn navigate_next_day(&mut self) -> Result<()> {
        let next = self.viewing_date + Duration::days(1);
        self.navigate_to_date(next)
    }

    pub fn navigate_to_today(&mut self) -> Result<()> {
        self.today = Local::now().date_naive();
        self.navigate_to_date(self.today)
    }

    pub fn save_undo(&mut self) {
        if self.undo_stack.len() >= MAX_UNDO_HISTORY {
            self.undo_stack.remove(0);
        }
        self.undo_stack
            .push((self.todo_list.clone(), self.cursor_position));
    }

    pub fn undo(&mut self) -> bool {
        if let Some((list, cursor)) = self.undo_stack.pop() {
            self.todo_list = list;
            self.cursor_position = cursor;
            self.unsaved_changes = true;
            true
        } else {
            false
        }
    }

    pub fn move_cursor_up(&mut self) {
        if self.cursor_position > 0 {
            self.cursor_position -= 1;
            while self.cursor_position > 0 && self.is_item_hidden(self.cursor_position) {
                self.cursor_position -= 1;
            }
        }
        self.sync_list_state();
    }

    pub fn move_cursor_down(&mut self) {
        if !self.todo_list.items.is_empty() && self.cursor_position < self.todo_list.items.len() - 1
        {
            self.cursor_position += 1;
            while self.cursor_position < self.todo_list.items.len() - 1
                && self.is_item_hidden(self.cursor_position)
            {
                self.cursor_position += 1;
            }
            if self.is_item_hidden(self.cursor_position) && self.cursor_position > 0 {
                self.cursor_position -= 1;
                while self.cursor_position > 0 && self.is_item_hidden(self.cursor_position) {
                    self.cursor_position -= 1;
                }
            }
        }
        self.sync_list_state();
    }

    fn is_item_hidden(&self, index: usize) -> bool {
        if index >= self.todo_list.items.len() {
            return false;
        }
        let mut current_indent = self.todo_list.items[index].indent_level;
        if current_indent == 0 {
            return false;
        }
        for i in (0..index).rev() {
            let item = &self.todo_list.items[i];
            if item.indent_level < current_indent {
                if item.collapsed {
                    return true;
                }
                current_indent = item.indent_level;
                if current_indent == 0 {
                    break;
                }
            }
        }
        false
    }

    pub fn selected_item(&self) -> Option<&TodoItem> {
        self.todo_list.items.get(self.cursor_position)
    }

    pub fn selected_item_mut(&mut self) -> Option<&mut TodoItem> {
        self.todo_list.items.get_mut(self.cursor_position)
    }

    pub fn clamp_cursor(&mut self) {
        if !self.todo_list.items.is_empty() {
            self.cursor_position = self.cursor_position.min(self.todo_list.items.len() - 1);
        } else {
            self.cursor_position = 0;
        }
        self.sync_list_state();
    }

    pub fn clear_selection(&mut self) {
        self.selection_anchor = None;
    }

    pub fn start_or_extend_selection(&mut self) {
        if self.selection_anchor.is_none() {
            self.selection_anchor = Some(self.cursor_position);
        }
    }

    pub fn get_selection_range(&self) -> Option<(usize, usize)> {
        self.selection_anchor.map(|anchor| {
            let start = anchor.min(self.cursor_position);
            let end = anchor.max(self.cursor_position);
            (start, end)
        })
    }

    pub fn is_selected(&self, index: usize) -> bool {
        if let Some((start, end)) = self.get_selection_range() {
            index >= start && index <= end
        } else {
            false
        }
    }

    pub fn find_parent_index(&self, index: usize) -> Option<usize> {
        if index >= self.todo_list.items.len() {
            return None;
        }
        let target_indent = self.todo_list.items[index].indent_level;
        if target_indent == 0 {
            return None;
        }
        (0..index)
            .rev()
            .find(|&i| self.todo_list.items[i].indent_level < target_indent)
    }

    pub fn move_to_parent(&mut self) {
        if let Some(parent_idx) = self.find_parent_index(self.cursor_position) {
            self.cursor_position = parent_idx;
        }
    }

    /// Reload the todo list from the database.
    /// Used when external changes are detected (e.g., from API server).
    pub fn reload_from_database(&mut self) -> Result<()> {
        let date = self.todo_list.date;
        let new_list = load_todo_list(date)?;
        self.todo_list = new_list;
        self.clamp_cursor();
        self.unsaved_changes = false;
        Ok(())
    }

    pub fn open_plugin_menu(&mut self) {
        let plugins = self.plugin_registry.list();
        self.plugin_state = Some(PluginSubState::Selecting {
            plugins,
            selected_index: 0,
        });
        self.mode = Mode::Plugin;
    }

    pub fn close_plugin_menu(&mut self) {
        self.plugin_state = None;
        self.mode = Mode::Navigate;
    }

    pub fn set_status_message(&mut self, message: String) {
        self.status_message = Some((message, Instant::now()));
    }

    pub fn clear_expired_status_message(&mut self) {
        if let Some((_, time)) = &self.status_message
            && time.elapsed().as_secs() > 3 {
                self.status_message = None;
            }
    }

    pub fn check_plugin_result(&mut self) {
        if let Some(rx) = &self.plugin_result_rx {
            match rx.try_recv() {
                Ok(Ok(items)) => {
                    self.plugin_result_rx = None;
                    if items.is_empty() {
                        self.plugin_state = Some(PluginSubState::Error {
                            message: "Plugin generated no items".to_string(),
                        });
                    } else {
                        self.plugin_state = Some(PluginSubState::Preview { items });
                    }
                }
                Ok(Err(e)) => {
                    self.plugin_result_rx = None;
                    self.plugin_state = Some(PluginSubState::Error { message: e });
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.plugin_result_rx = None;
                    self.plugin_state = Some(PluginSubState::Error {
                        message: "Plugin execution thread crashed".to_string(),
                    });
                }
            }
        }
    }

    pub fn tick_spinner(&mut self) {
        self.spinner_frame = (self.spinner_frame + 1) % 8;
    }

    /// Check for new version availability (non-blocking)
    pub fn check_version_update(&mut self) {
        if let Ok(result) = self.version_check_rx.try_recv()
            && result.is_newer
        {
            let new_version = result.latest_version.clone();
            self.new_version_available = Some(new_version.clone());

            // Auto-show upgrade prompt if:
            // 1. Not already dismissed this session
            // 2. Not in skipped_version list
            let should_show = !self.session_dismissed_upgrade
                && self.skipped_version.as_ref() != Some(&new_version);

            if should_show {
                self.show_upgrade_prompt = true;
                self.mode = Mode::UpgradePrompt;
            }
        }
    }

    /// Open upgrade modal (e.g., when clicking version in status bar)
    pub fn open_upgrade_modal(&mut self) {
        if self.new_version_available.is_some() {
            self.show_upgrade_prompt = true;
            self.upgrade_sub_state = Some(UpgradeSubState::Prompt);
            self.mode = Mode::UpgradePrompt;
        }
    }

    /// Dismiss upgrade prompt for this session only
    pub fn dismiss_upgrade_session(&mut self) {
        self.session_dismissed_upgrade = true;
        self.show_upgrade_prompt = false;
        self.upgrade_sub_state = None;
        self.download_progress_rx = None;
        self.mode = Mode::Navigate;
    }

    /// Start downloading the new version
    pub fn start_download(&mut self) {
        let version = match &self.new_version_available {
            Some(v) => v.clone(),
            None => return,
        };

        let url = get_asset_download_url(&version);
        let target_path = std::env::temp_dir().join(format!("totui-{}.tar.gz", version));
        let rx = spawn_download(url, target_path);

        self.download_progress_rx = Some(rx);
        self.upgrade_sub_state = Some(UpgradeSubState::Downloading {
            progress: 0.0,
            bytes_downloaded: 0,
            total_bytes: None,
        });
    }

    /// Check for download progress updates (non-blocking)
    pub fn check_download_progress(&mut self) {
        let rx = match &mut self.download_progress_rx {
            Some(rx) => rx,
            None => return,
        };

        match rx.try_recv() {
            Ok(DownloadProgress::Progress { bytes, total }) => {
                let progress = bytes as f64 / total.unwrap_or(bytes.max(1)) as f64;
                self.upgrade_sub_state = Some(UpgradeSubState::Downloading {
                    progress,
                    bytes_downloaded: bytes,
                    total_bytes: total,
                });
            }
            Ok(DownloadProgress::Complete { path }) => {
                self.download_progress_rx = None;
                self.upgrade_sub_state = Some(UpgradeSubState::RestartPrompt {
                    downloaded_path: path,
                });
            }
            Ok(DownloadProgress::Error { message }) => {
                self.download_progress_rx = None;
                self.upgrade_sub_state = Some(UpgradeSubState::Error { message });
            }
            Err(tokio_mpsc::error::TryRecvError::Empty) => {
                // No update yet, do nothing
            }
            Err(tokio_mpsc::error::TryRecvError::Disconnected) => {
                self.download_progress_rx = None;
                self.upgrade_sub_state = Some(UpgradeSubState::Error {
                    message: "Download task crashed".to_string(),
                });
            }
        }
    }

    /// Skip this version permanently (save to config)
    pub fn skip_version_permanently(&mut self, version: String) -> Result<()> {
        use crate::config::Config;

        // Load current config, update skipped_version, and save
        let mut config = Config::load()?;
        config.skipped_version = Some(version.clone());
        config.save()?;

        // Update local state
        self.skipped_version = Some(version);
        self.show_upgrade_prompt = false;
        self.mode = Mode::Navigate;

        Ok(())
    }

    pub fn get_spinner_char(&self) -> char {
        const SPINNER_FRAMES: [char; 8] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧'];
        SPINNER_FRAMES[self.spinner_frame]
    }

    /// Toggle the current item's state (checked/unchecked) with undo support.
    /// Returns true if a change was made.
    pub fn toggle_current_item_state(&mut self) -> bool {
        if self.selected_item().is_some() {
            self.save_undo();
            if let Some(item) = self.selected_item_mut() {
                item.toggle_state();
                self.unsaved_changes = true;
                return true;
            }
        }
        false
    }

    /// Cycle the current item's state with undo support.
    /// Returns true if a change was made.
    pub fn cycle_current_item_state(&mut self) -> bool {
        if self.selected_item().is_some() {
            self.save_undo();
            if let Some(item) = self.selected_item_mut() {
                item.cycle_state();
                self.unsaved_changes = true;
                return true;
            }
        }
        false
    }

    /// Cycle the current item's priority with undo support.
    /// Cycles: None -> P0 -> P1 -> P2 -> None
    pub fn cycle_priority(&mut self) {
        if self.is_readonly() {
            return;
        }

        if self.selected_item().is_some() {
            self.save_undo();
            if let Some(item) = self.selected_item_mut() {
                item.priority = item.priority.cycle_priority();
                item.modified_at = chrono::Utc::now();

                let priority_str = item
                    .priority
                    .map(|p| p.to_string())
                    .unwrap_or_else(|| "None".to_string());
                self.status_message = Some((format!("Priority: {}", priority_str), std::time::Instant::now()));
                self.unsaved_changes = true;
            }
        }
    }

    /// Toggle collapse state of the current item if it's collapsible.
    /// Returns true if a change was made.
    pub fn toggle_current_item_collapse(&mut self) -> bool {
        let has_children = self.todo_list.has_children(self.cursor_position);
        let has_description = self
            .todo_list
            .items
            .get(self.cursor_position)
            .map(|item| item.description.is_some())
            .unwrap_or(false);

        if has_children || has_description {
            self.save_undo();
            if let Some(item) = self.todo_list.items.get_mut(self.cursor_position) {
                item.collapsed = !item.collapsed;
                self.unsaved_changes = true;
                return true;
            }
        }
        false
    }

    /// Expand the current item if it's collapsed and collapsible.
    /// Returns true if a change was made.
    pub fn expand_current_item(&mut self) -> bool {
        let has_children = self.todo_list.has_children(self.cursor_position);
        let item_info = self
            .todo_list
            .items
            .get(self.cursor_position)
            .map(|item| (item.collapsed, item.description.is_some()));

        let should_expand = match item_info {
            Some((true, has_desc)) => has_children || has_desc,
            _ => false,
        };

        if should_expand {
            self.save_undo();
            if let Some(item) = self.todo_list.items.get_mut(self.cursor_position) {
                item.collapsed = false;
                self.unsaved_changes = true;
                return true;
            }
        }
        false
    }

    /// Collapse the current item or move to parent if already collapsed/not collapsible.
    /// Returns true if collapsed (false means moved to parent or no action).
    pub fn collapse_or_move_to_parent(&mut self) -> bool {
        let has_children = self.todo_list.has_children(self.cursor_position);
        let item_info = self
            .todo_list
            .items
            .get(self.cursor_position)
            .map(|item| (item.collapsed, item.description.is_some()));

        let (is_collapsed, has_description) = item_info.unwrap_or((false, false));
        let is_collapsible = has_children || has_description;

        if is_collapsible && !is_collapsed {
            self.save_undo();
            if let Some(item) = self.todo_list.items.get_mut(self.cursor_position) {
                item.collapsed = true;
                self.unsaved_changes = true;
                return true;
            }
        } else {
            self.move_to_parent();
        }
        false
    }

    /// Sort todos by priority (P0 first, then P1, P2, None last).
    /// Children remain grouped under their parent.
    pub fn sort_by_priority(&mut self) {
        if self.is_readonly() {
            return;
        }

        self.save_undo();
        self.todo_list.sort_by_priority();
        self.cursor_position = 0; // Reset cursor to top after sort
        self.sync_list_state();
        self.status_message = Some(("Sorted by priority".to_string(), std::time::Instant::now()));
    }

    /// Open the rollover modal with the given pending items
    pub fn open_rollover_modal(&mut self, source_date: NaiveDate, items: Vec<TodoItem>) {
        self.pending_rollover = Some(PendingRollover { source_date, items });
        self.mode = Mode::Rollover;
    }

    /// Close the rollover modal without executing rollover
    pub fn close_rollover_modal(&mut self) {
        self.mode = Mode::Navigate;
        // Note: we keep pending_rollover so user can re-trigger with R key
    }

    /// Check if there's pending rollover data available
    pub fn has_pending_rollover(&self) -> bool {
        self.pending_rollover.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_dismiss_upgrade() {
        use crate::keybindings::KeybindingCache;
        use crate::plugin::PluginRegistry;
        use crate::todo::TodoList;
        use crate::ui::theme::Theme;
        use chrono::Local;

        let date = Local::now().date_naive();
        let todo_list = TodoList {
            date,
            items: vec![],
            file_path: std::path::PathBuf::from("/tmp/test.md"),
        };

        let mut state = AppState::new(
            todo_list,
            Theme::default(),
            KeybindingCache::default(),
            1000,
            PluginRegistry::new(),
            None,
            None,
        );

        // Simulate new version detected
        state.new_version_available = Some("0.4.0".to_string());
        state.show_upgrade_prompt = true;
        state.mode = Mode::UpgradePrompt;

        // Dismiss for session
        state.dismiss_upgrade_session();

        // Verify state after dismissal
        assert_eq!(state.session_dismissed_upgrade, true);
        assert_eq!(state.show_upgrade_prompt, false);
        assert_eq!(state.mode, Mode::Navigate);

        // Simulate another check - should NOT auto-show because session dismissed
        state.mode = Mode::Navigate;
        let should_show = !state.session_dismissed_upgrade
            && state.skipped_version.as_ref() != state.new_version_available.as_ref();
        assert_eq!(should_show, false, "Should not auto-show after session dismiss");
    }
}
