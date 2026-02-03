use super::mode::Mode;
use crate::keybindings::{KeyBinding, KeybindingCache};
use crate::plugin::{
    marketplace::PluginEntry, GeneratorInfo, HookDispatcher, PluginActionRegistry, PluginLoadError,
    PluginLoader,
};
use crate::project::{Project, ProjectRegistry};
use crate::storage::file::{load_todo_list_for_project, load_todos_for_viewing_in_project};
use crate::storage::rollover::find_rollover_candidates_for_project;
use crate::storage::UiCache;
use crate::todo::{PriorityCycle, TodoItem, TodoList};
use crate::ui::theme::Theme;
use crate::utils::upgrade::{
    get_asset_download_url, spawn_download, DownloadProgress, PluginUpgradeSubState, UpgradeSubState,
};
use crate::utils::version_check::{spawn_version_checker, PluginUpdateInfo, VersionCheckResult};
use anyhow::Result;
use chrono::{Duration, Local, NaiveDate};
use ratatui::widgets::ListState;
use std::sync::mpsc;
use std::time::Instant;
use totui_plugin_interface::{FfiEvent, FfiFieldChange};
use tracing::{debug, trace};
use uuid::Uuid;

const MAX_UNDO_HISTORY: usize = 50;

/// Tab selection in plugins modal
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginsTab {
    Installed,
    Marketplace,
}

/// State for the tabbed plugins modal
#[derive(Debug, Clone)]
pub enum PluginsModalState {
    /// Tab selection mode with list navigation
    Tabs {
        active_tab: PluginsTab,
        installed_index: usize,
        marketplace_index: usize,
        marketplace_plugins: Option<Vec<PluginEntry>>,
        marketplace_loading: bool,
        marketplace_error: Option<String>,
        /// Name of the marketplace (owner/repo format)
        marketplace_name: String,
    },
    /// Plugin details view (from Marketplace tab)
    Details {
        plugin: PluginEntry,
        /// Cached marketplace state to return to
        marketplace_plugins: Vec<PluginEntry>,
        marketplace_index: usize,
    },
    /// Plugin input prompt (invoking installed plugin)
    Input {
        plugin_name: String,
        input_buffer: String,
        cursor_pos: usize,
    },
    /// Plugin select input (dropdown for Select type config fields)
    SelectInput {
        plugin_name: String,
        field_name: String,
        /// (display text, value) pairs parsed from "display|value" format
        options: Vec<(String, String)>,
        selected_index: usize,
    },
    /// Executing plugin
    Executing {
        plugin_name: String,
    },
    /// Preview generated items
    Preview {
        items: Vec<TodoItem>,
    },
    /// Error display
    Error {
        message: String,
    },
}

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

/// Project modal sub-state
#[derive(Debug, Clone)]
pub enum ProjectSubState {
    Selecting {
        projects: Vec<Project>,
        selected_index: usize,
    },
    CreateInput {
        input_buffer: String,
        cursor_pos: usize,
    },
    RenameInput {
        project_name: String,
        input_buffer: String,
        cursor_pos: usize,
    },
    ConfirmDelete {
        project_name: String,
    },
}

/// Move to project modal sub-state
#[derive(Debug, Clone)]
pub enum MoveToProjectSubState {
    Selecting {
        projects: Vec<Project>,
        selected_index: usize,
        item_index: usize,  // Index of item being moved
    },
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
    pub plugin_state: Option<PluginSubState>,
    /// New tabbed plugins modal state (replaces plugin_state for P key)
    pub plugins_modal_state: Option<PluginsModalState>,
    /// Receiver for marketplace fetch results
    pub marketplace_fetch_rx: Option<mpsc::Receiver<Result<Vec<PluginEntry>, String>>>,
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
    /// Channel receiver for download progress updates (std::sync::mpsc for thread-based download)
    pub download_progress_rx: Option<mpsc::Receiver<DownloadProgress>>,
    /// List of plugins that have updates available
    pub plugin_updates_available: Vec<PluginUpdateInfo>,
    /// Channel receiver for plugin download progress updates
    pub plugin_download_progress_rx: Option<mpsc::Receiver<DownloadProgress>>,
    /// Current active project
    pub current_project: Project,
    /// Project selection modal state
    pub project_state: Option<ProjectSubState>,
    /// Move to project modal state
    pub move_to_project_state: Option<MoveToProjectSubState>,
    /// Whether the mouse cursor is currently showing as pointer (for hover effects)
    pub cursor_is_pointer: bool,
    /// Plugin loader with dynamically loaded plugins
    pub plugin_loader: PluginLoader,
    /// Plugin loading errors to display on first render
    pub pending_plugin_errors: Vec<PluginLoadError>,
    /// Whether to show plugin error popup
    pub show_plugin_error_popup: bool,
    /// Registry of plugin actions with keybinding mappings
    pub plugin_action_registry: PluginActionRegistry,
    /// Hook dispatcher for async event handling.
    pub hook_dispatcher: HookDispatcher,
    /// True when applying hook-returned commands (prevents cascade).
    in_hook_apply: bool,
}

impl AppState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        todo_list: TodoList,
        theme: Theme,
        keybindings: KeybindingCache,
        timeoutlen: u64,
        ui_cache: Option<UiCache>,
        skipped_version: Option<String>,
        current_project: Project,
        plugin_loader: PluginLoader,
        plugin_errors: Vec<PluginLoadError>,
        plugin_action_registry: PluginActionRegistry,
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
            plugin_state: None,
            plugins_modal_state: None,
            marketplace_fetch_rx: None,
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
            plugin_updates_available: Vec::new(),
            plugin_download_progress_rx: None,
            current_project,
            project_state: None,
            move_to_project_state: None,
            cursor_is_pointer: false,
            show_plugin_error_popup: !plugin_errors.is_empty(),
            pending_plugin_errors: plugin_errors,
            plugin_loader,
            plugin_action_registry,
            hook_dispatcher: HookDispatcher::new(),
            in_hook_apply: false,
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

    /// Sync list_state selection for when creating a new item.
    /// This accounts for the temporary edit row that appears during new item creation.
    pub fn sync_list_state_for_new_item(&mut self) {
        // First get the base visible index
        self.sync_list_state();

        if !self.is_creating_new_item {
            return;
        }

        // Get current selection
        let current = match self.list_state.selected() {
            Some(idx) => idx,
            None => return,
        };

        if self.insert_above {
            // New item appears at current position, no change needed
            // The highlight should stay where it is
        } else {
            // New item appears below current item
            // Need to increment selection to point to the new row
            let mut offset = 1;

            // If current item has expanded description, that's another ListItem between
            if let Some(item) = self.todo_list.items.get(self.cursor_position)
                && !item.collapsed
                && item.description.is_some()
            {
                offset += 1;
            }

            self.list_state.select(Some(current + offset));
        }
    }

    pub fn navigate_to_date(&mut self, date: NaiveDate) -> Result<()> {
        if date > self.today {
            return Ok(());
        }
        self.todo_list = load_todos_for_viewing_in_project(&self.current_project.name, date)?;
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
            trace!("Undo stack full ({}), removing oldest entry", MAX_UNDO_HISTORY);
            self.undo_stack.remove(0);
        }
        
        let item_ids: Vec<String> = self.todo_list.items.iter().map(|i| i.id.to_string()).collect();
        debug!(
            stack_depth = self.undo_stack.len() + 1,
            item_count = self.todo_list.items.len(),
            cursor = self.cursor_position,
            ids = ?item_ids,
            "save_undo: pushing state to undo stack"
        );
        
        self.undo_stack
            .push((self.todo_list.clone(), self.cursor_position));
    }

    pub fn undo(&mut self) -> bool {
        if let Some((list, cursor)) = self.undo_stack.pop() {
            let old_ids: Vec<String> = self.todo_list.items.iter().map(|i| i.id.to_string()).collect();
            let new_ids: Vec<String> = list.items.iter().map(|i| i.id.to_string()).collect();
            
            debug!(
                stack_depth_after = self.undo_stack.len(),
                old_item_count = self.todo_list.items.len(),
                new_item_count = list.items.len(),
                old_cursor = self.cursor_position,
                new_cursor = cursor,
                old_ids = ?old_ids,
                new_ids = ?new_ids,
                "undo: restoring previous state"
            );
            
            self.todo_list = list;
            self.cursor_position = cursor;
            self.unsaved_changes = true;
            true
        } else {
            debug!("undo: stack empty, nothing to undo");
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
        // Skip reload if we have unsaved changes - don't overwrite in-memory modifications
        if self.unsaved_changes {
            tracing::debug!(
                project = %self.current_project.name,
                "Skipping database reload - unsaved changes present"
            );
            return Ok(());
        }

        let date = self.todo_list.date;
        let new_list = load_todo_list_for_project(&self.current_project.name, date)?;
        self.todo_list = new_list;
        self.clamp_cursor();
        self.unsaved_changes = false;
        Ok(())
    }

    pub fn close_plugin_menu(&mut self) {
        self.plugin_state = None;
        self.mode = Mode::Navigate;
    }

    /// Open plugins modal with Installed tab selected
    pub fn open_plugins_modal(&mut self) {
        use crate::config::Config;
        use crate::plugin::marketplace::DEFAULT_MARKETPLACE;

        let marketplace_name = Config::load()
            .map(|c| c.marketplaces.default)
            .unwrap_or_else(|_| DEFAULT_MARKETPLACE.to_string());

        self.plugins_modal_state = Some(PluginsModalState::Tabs {
            active_tab: PluginsTab::Installed,
            installed_index: 0,
            marketplace_index: 0,
            marketplace_plugins: None,
            marketplace_loading: false,
            marketplace_error: None,
            marketplace_name,
        });
        self.mode = Mode::Plugin;
    }

    /// Close plugins modal and return to Navigate mode
    pub fn close_plugins_modal(&mut self) {
        self.plugins_modal_state = None;
        self.marketplace_fetch_rx = None;
        self.mode = Mode::Navigate;
    }

    /// Start async marketplace fetch
    pub fn start_marketplace_fetch(&mut self) {
        use crate::config::Config;
        use crate::plugin::marketplace::fetch_marketplace;

        let (tx, rx) = mpsc::channel();
        self.marketplace_fetch_rx = Some(rx);

        // Get marketplace URL from config (format: "owner/repo")
        let marketplace_ref = Config::load()
            .map(|c| c.marketplaces.default.clone())
            .unwrap_or_else(|_| "grimurjonsson/to-tui-plugins".to_string());

        // Parse owner/repo
        let parts: Vec<&str> = marketplace_ref.split('/').collect();
        let (owner, repo) = if parts.len() == 2 {
            (parts[0].to_string(), parts[1].to_string())
        } else {
            ("grimurjonsson".to_string(), "to-tui-plugins".to_string())
        };

        std::thread::spawn(move || {
            let result = fetch_marketplace(&owner, &repo)
                .map(|manifest| manifest.plugins)
                .map_err(|e| e.to_string());
            let _ = tx.send(result);
        });

        // Update state to show loading
        if let Some(PluginsModalState::Tabs {
            marketplace_loading,
            ..
        }) = &mut self.plugins_modal_state
        {
            *marketplace_loading = true;
        }
    }

    /// Check for marketplace fetch results (non-blocking)
    pub fn check_marketplace_fetch(&mut self) {
        if let Some(rx) = &self.marketplace_fetch_rx {
            match rx.try_recv() {
                Ok(Ok(plugins)) => {
                    self.marketplace_fetch_rx = None;
                    if let Some(PluginsModalState::Tabs {
                        marketplace_plugins,
                        marketplace_loading,
                        marketplace_error,
                        ..
                    }) = &mut self.plugins_modal_state
                    {
                        *marketplace_plugins = Some(plugins);
                        *marketplace_loading = false;
                        *marketplace_error = None;
                    }
                }
                Ok(Err(e)) => {
                    self.marketplace_fetch_rx = None;
                    if let Some(PluginsModalState::Tabs {
                        marketplace_loading,
                        marketplace_error,
                        ..
                    }) = &mut self.plugins_modal_state
                    {
                        *marketplace_loading = false;
                        *marketplace_error = Some(e);
                    }
                }
                Err(mpsc::TryRecvError::Empty) => {
                    // Still loading, do nothing
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.marketplace_fetch_rx = None;
                    if let Some(PluginsModalState::Tabs {
                        marketplace_loading,
                        marketplace_error,
                        ..
                    }) = &mut self.plugins_modal_state
                    {
                        *marketplace_loading = false;
                        *marketplace_error = Some("Marketplace fetch failed".to_string());
                    }
                }
            }
        }
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
    /// Checks both app updates and plugin updates.
    pub fn check_version_update(&mut self) {
        if let Ok(result) = self.version_check_rx.try_recv() {
            // Handle app update
            if let Some(app_update) = result.app_update {
                if app_update.is_newer {
                    let new_version = app_update.latest_version.clone();
                    self.new_version_available = Some(new_version.clone());
                }
            }

            // Handle plugin updates
            if !result.plugin_updates.is_empty() {
                self.plugin_updates_available = result.plugin_updates;
            }

            // Auto-show upgrade prompt if:
            // 1. Not already dismissed this session
            // 2. Not in skipped_version list (for app)
            // 3. There are updates available
            let has_app_update = self.new_version_available.is_some();
            let has_plugin_updates = !self.plugin_updates_available.is_empty();

            let should_show = !self.session_dismissed_upgrade
                && (has_plugin_updates
                    || (has_app_update
                        && self.skipped_version.as_ref() != self.new_version_available.as_ref()));

            if should_show && (has_app_update || has_plugin_updates) {
                self.show_upgrade_prompt = true;
                self.mode = Mode::UpgradePrompt;
            }
        }
    }

    /// Open upgrade modal (e.g., when clicking version in status bar)
    pub fn open_upgrade_modal(&mut self) {
        if self.new_version_available.is_some() || !self.plugin_updates_available.is_empty() {
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
        // Download as raw binary (releases are not archived)
        let target_path = std::env::temp_dir().join(format!("totui-{}", version));
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
            Err(mpsc::TryRecvError::Empty) => {
                // No update yet, do nothing
            }
            Err(mpsc::TryRecvError::Disconnected) => {
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

    /// Enter the plugin upgrade flow
    pub fn enter_plugin_upgrades(&mut self) {
        if self.plugin_updates_available.is_empty() {
            return;
        }

        self.upgrade_sub_state = Some(UpgradeSubState::PluginUpgrades(
            PluginUpgradeSubState::PluginList {
                updates: self.plugin_updates_available.clone(),
                selected_index: 0,
            },
        ));
    }

    /// Start downloading a plugin update
    pub fn start_plugin_download(&mut self, plugin: &PluginUpdateInfo) {
        use crate::config::Config;
        use crate::plugin::marketplace::DEFAULT_MARKETPLACE;

        // Get marketplace config to construct download URL
        let marketplace_ref = Config::load()
            .map(|c| c.marketplaces.default.clone())
            .unwrap_or_else(|_| DEFAULT_MARKETPLACE.to_string());

        let (owner, repo) = match marketplace_ref.split_once('/') {
            Some((o, r)) => (o.to_string(), r.to_string()),
            None => {
                self.upgrade_sub_state = Some(UpgradeSubState::PluginUpgrades(
                    PluginUpgradeSubState::Error {
                        plugin_name: plugin.plugin_name.clone(),
                        message: format!("Invalid marketplace format: {}", marketplace_ref),
                        remaining_updates: self.get_remaining_plugin_updates(&plugin.plugin_name),
                    },
                ));
                return;
            }
        };

        // Construct download URL
        let target = crate::utils::upgrade::get_target_triple();
        let url = format!(
            "https://github.com/{}/{}/releases/download/{}-v{}/{}-{}.tar.gz",
            owner, repo, plugin.plugin_name, plugin.latest_version, plugin.plugin_name, target
        );

        // Download to temp directory
        let target_path = std::env::temp_dir().join(format!(
            "{}-{}.tar.gz",
            plugin.plugin_name, plugin.latest_version
        ));
        let rx = spawn_download(url, target_path);

        self.plugin_download_progress_rx = Some(rx);
        self.upgrade_sub_state = Some(UpgradeSubState::PluginUpgrades(
            PluginUpgradeSubState::Downloading {
                plugin_name: plugin.plugin_name.clone(),
                current_version: plugin.current_version.clone(),
                latest_version: plugin.latest_version.clone(),
                progress: 0.0,
                bytes_downloaded: 0,
                total_bytes: None,
            },
        ));
    }

    /// Check for plugin download progress updates (non-blocking)
    pub fn check_plugin_download_progress(&mut self) {
        let rx = match &mut self.plugin_download_progress_rx {
            Some(rx) => rx,
            None => return,
        };

        // Get current plugin info from state
        let (plugin_name, current_version, latest_version) =
            if let Some(UpgradeSubState::PluginUpgrades(PluginUpgradeSubState::Downloading {
                plugin_name,
                current_version,
                latest_version,
                ..
            })) = &self.upgrade_sub_state
            {
                (
                    plugin_name.clone(),
                    current_version.clone(),
                    latest_version.clone(),
                )
            } else {
                return;
            };

        match rx.try_recv() {
            Ok(DownloadProgress::Progress { bytes, total }) => {
                let progress = bytes as f64 / total.unwrap_or(bytes.max(1)) as f64;
                self.upgrade_sub_state = Some(UpgradeSubState::PluginUpgrades(
                    PluginUpgradeSubState::Downloading {
                        plugin_name,
                        current_version,
                        latest_version,
                        progress,
                        bytes_downloaded: bytes,
                        total_bytes: total,
                    },
                ));
            }
            Ok(DownloadProgress::Complete { path }) => {
                self.plugin_download_progress_rx = None;
                // Install the downloaded plugin
                self.install_downloaded_plugin(&plugin_name, &latest_version, &path);
            }
            Ok(DownloadProgress::Error { message }) => {
                self.plugin_download_progress_rx = None;
                self.upgrade_sub_state = Some(UpgradeSubState::PluginUpgrades(
                    PluginUpgradeSubState::Error {
                        plugin_name: plugin_name.clone(),
                        message,
                        remaining_updates: self.get_remaining_plugin_updates(&plugin_name),
                    },
                ));
            }
            Err(mpsc::TryRecvError::Empty) => {
                // No update yet, do nothing
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.plugin_download_progress_rx = None;
                self.upgrade_sub_state = Some(UpgradeSubState::PluginUpgrades(
                    PluginUpgradeSubState::Error {
                        plugin_name: plugin_name.clone(),
                        message: "Download task crashed".to_string(),
                        remaining_updates: self.get_remaining_plugin_updates(&plugin_name),
                    },
                ));
            }
        }
    }

    /// Install a downloaded plugin from the archive path
    fn install_downloaded_plugin(
        &mut self,
        plugin_name: &str,
        new_version: &str,
        archive_path: &std::path::Path,
    ) {
        use crate::utils::paths::get_plugins_dir;
        use flate2::read::GzDecoder;
        use tar::Archive;

        let result = (|| -> anyhow::Result<()> {
            // Get plugins directory
            let plugins_dir = get_plugins_dir()?;
            let plugin_dir = plugins_dir.join(plugin_name);

            // Create temp extraction directory
            let temp_extract = std::env::temp_dir().join(format!("{}-extract", plugin_name));
            if temp_extract.exists() {
                std::fs::remove_dir_all(&temp_extract)?;
            }
            std::fs::create_dir_all(&temp_extract)?;

            // Extract archive
            let tar_gz = std::fs::File::open(archive_path)?;
            let tar = GzDecoder::new(tar_gz);
            let mut archive = Archive::new(tar);
            archive.unpack(&temp_extract)?;

            // Check if archive extracted to a subdirectory or flat
            // Look for plugin.toml to determine structure
            let extracted_dir = if temp_extract.join("plugin.toml").exists() {
                // Flat archive - files are directly in temp_extract
                temp_extract.clone()
            } else {
                // Look for a subdirectory containing plugin.toml
                let mut found_dir = None;
                for entry in std::fs::read_dir(&temp_extract)? {
                    let entry = entry?;
                    if entry.path().is_dir() && entry.path().join("plugin.toml").exists() {
                        found_dir = Some(entry.path());
                        break;
                    }
                }
                found_dir.ok_or_else(|| {
                    anyhow::anyhow!("No plugin.toml found in archive")
                })?
            };

            // Remove old plugin directory if it exists
            if plugin_dir.exists() {
                std::fs::remove_dir_all(&plugin_dir)?;
            }

            // Ensure parent directory exists
            if let Some(parent) = plugin_dir.parent() {
                std::fs::create_dir_all(parent)?;
            }

            // Move extracted plugin to plugins directory
            // For flat archives, extracted_dir == temp_extract, so rename moves everything
            std::fs::rename(&extracted_dir, &plugin_dir)?;

            // Clean up - only remove temp_extract if it still exists (wasn't the extracted_dir)
            if temp_extract.exists() {
                let _ = std::fs::remove_dir_all(&temp_extract);
            }
            let _ = std::fs::remove_file(archive_path);

            Ok(())
        })();

        match result {
            Ok(()) => {
                // Remove from updates list
                self.plugin_updates_available
                    .retain(|p| p.plugin_name != plugin_name);

                let remaining = self.get_remaining_plugin_updates(plugin_name);
                self.upgrade_sub_state = Some(UpgradeSubState::PluginUpgrades(
                    PluginUpgradeSubState::Complete {
                        plugin_name: plugin_name.to_string(),
                        new_version: new_version.to_string(),
                        remaining_updates: remaining,
                    },
                ));
            }
            Err(e) => {
                self.upgrade_sub_state = Some(UpgradeSubState::PluginUpgrades(
                    PluginUpgradeSubState::Error {
                        plugin_name: plugin_name.to_string(),
                        message: format!("Installation failed: {}", e),
                        remaining_updates: self.get_remaining_plugin_updates(plugin_name),
                    },
                ));
            }
        }
    }

    /// Get remaining plugin updates excluding the specified plugin
    pub fn get_remaining_plugin_updates(&self, exclude_plugin: &str) -> Vec<PluginUpdateInfo> {
        self.plugin_updates_available
            .iter()
            .filter(|p| p.plugin_name != exclude_plugin)
            .cloned()
            .collect()
    }

    /// Continue to next plugin update after completing one
    pub fn continue_plugin_upgrades(&mut self) {
        if let Some(UpgradeSubState::PluginUpgrades(
            PluginUpgradeSubState::Complete {
                remaining_updates, ..
            }
            | PluginUpgradeSubState::Error {
                remaining_updates, ..
            },
        )) = &self.upgrade_sub_state
        {
            if remaining_updates.is_empty() {
                // No more updates, return to main upgrade prompt or navigate
                if self.new_version_available.is_some() {
                    self.upgrade_sub_state = Some(UpgradeSubState::Prompt);
                } else {
                    self.dismiss_upgrade_session();
                }
            } else {
                // Show remaining plugins list
                self.upgrade_sub_state = Some(UpgradeSubState::PluginUpgrades(
                    PluginUpgradeSubState::PluginList {
                        updates: remaining_updates.clone(),
                        selected_index: 0,
                    },
                ));
            }
        }
    }

    /// Exit plugin upgrade flow and return to main upgrade prompt
    pub fn exit_plugin_upgrades(&mut self) {
        if self.new_version_available.is_some() || !self.plugin_updates_available.is_empty() {
            self.upgrade_sub_state = Some(UpgradeSubState::Prompt);
        } else {
            self.dismiss_upgrade_session();
        }
    }

    pub fn get_spinner_char(&self) -> char {
        const SPINNER_FRAMES: [char; 8] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧'];
        SPINNER_FRAMES[self.spinner_frame]
    }

    /// Toggle the current item's state (checked/unchecked) AND all descendants with undo support.
    /// Returns true if a change was made.
    pub fn toggle_current_item_state(&mut self) -> bool {
        if self.selected_item().is_none() {
            return false;
        }

        self.save_undo();

        // Get the range including this item and all its children
        let (start, end) = match self.todo_list.get_item_range(self.cursor_position) {
            Ok(range) => range,
            Err(_) => return false,
        };

        // Determine target state based on current item's state
        // If current item is Checked, toggle to Empty; otherwise toggle to Checked
        let target_state = self.todo_list.items[self.cursor_position].state.toggle();

        // Apply the target state to all items in range
        for i in start..end {
            self.todo_list.items[i].state = target_state;
            self.todo_list.items[i].modified_at = chrono::Utc::now();
        }

        self.unsaved_changes = true;

        // Fire event for state change on the main item (not all children)
        if let Some(ffi_item) = self.todo_to_ffi(self.cursor_position) {
            let event = if self.todo_list.items[self.cursor_position].state.is_complete() {
                FfiEvent::OnComplete { todo: ffi_item }
            } else {
                FfiEvent::OnModify {
                    todo: ffi_item,
                    field_changed: FfiFieldChange::State,
                }
            };
            self.fire_event(event);
        }

        true
    }

    /// Cycle the current item's state with undo support.
    /// Returns true if a change was made.
    pub fn cycle_current_item_state(&mut self) -> bool {
        if self.selected_item().is_some() {
            self.save_undo();
            if let Some(item) = self.selected_item_mut() {
                item.cycle_state();
                self.unsaved_changes = true;

                // Fire event for state change
                if let Some(ffi_item) = self.todo_to_ffi(self.cursor_position) {
                    let event = if self.todo_list.items[self.cursor_position].state.is_complete() {
                        FfiEvent::OnComplete { todo: ffi_item }
                    } else {
                        FfiEvent::OnModify {
                            todo: ffi_item,
                            field_changed: FfiFieldChange::State,
                        }
                    };
                    self.fire_event(event);
                }

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

    /// Open the project selection modal
    pub fn open_project_modal(&mut self) {
        let registry = ProjectRegistry::load().unwrap_or_default();
        let projects: Vec<Project> = registry.list_sorted().into_iter().cloned().collect();

        // Find the current project's index in the sorted list
        let selected_index = projects
            .iter()
            .position(|p| p.name == self.current_project.name)
            .unwrap_or(0);

        self.project_state = Some(ProjectSubState::Selecting {
            projects,
            selected_index,
        });
        self.mode = Mode::ProjectSelect;
    }

    /// Close the project modal and return to navigate mode
    /// (unless rollover modal is active)
    pub fn close_project_modal(&mut self) {
        self.project_state = None;
        // Don't override Rollover mode - it may have been triggered by switch_project
        if self.mode != Mode::Rollover {
            self.mode = Mode::Navigate;
        }
    }

    /// Switch to a different project
    pub fn switch_project(&mut self, project: Project) -> Result<()> {
        // Save any unsaved changes first to the CURRENT project before switching
        if self.unsaved_changes {
            crate::storage::file::save_todo_list_for_project(&self.todo_list, &self.current_project.name)?;
            self.unsaved_changes = false;
        }

        // Check for rollover candidates in the new project BEFORE loading the list
        // (same pattern as startup in main.rs)
        let rollover_candidates = find_rollover_candidates_for_project(&project.name);

        // Load the new project's todo list
        let today = Local::now().date_naive();
        let new_list = load_todo_list_for_project(&project.name, today)?;

        self.current_project = project;
        self.todo_list = new_list;
        self.viewing_date = today;
        self.today = today;
        self.cursor_position = 0;
        self.undo_stack.clear();
        self.sync_list_state();

        // Show rollover modal if candidates were found
        if let Ok(Some((source_date, items))) = rollover_candidates {
            self.open_rollover_modal(source_date, items);
        }

        Ok(())
    }

    /// Open the move-to-project modal for the current item
    pub fn open_move_to_project_modal(&mut self) {
        if self.todo_list.items.is_empty() {
            return;
        }

        let registry = ProjectRegistry::load().unwrap_or_default();
        let projects: Vec<Project> = registry
            .list_sorted()
            .into_iter()
            .filter(|p| p.name != self.current_project.name)  // Exclude current
            .cloned()
            .collect();

        if projects.is_empty() {
            self.set_status_message("No other projects to move to".to_string());
            return;
        }

        self.move_to_project_state = Some(MoveToProjectSubState::Selecting {
            projects,
            selected_index: 0,
            item_index: self.cursor_position,
        });
        self.mode = Mode::MoveToProject;
    }

    /// Close the move-to-project modal
    pub fn close_move_to_project_modal(&mut self) {
        self.move_to_project_state = None;
        self.mode = Mode::Navigate;
    }

    /// Dismiss the plugin error popup without clearing the errors.
    /// Errors stay in pending_plugin_errors for `totui plugin status` command.
    pub fn dismiss_plugin_error_popup(&mut self) {
        self.show_plugin_error_popup = false;
        // Note: errors stay in pending_plugin_errors for totui plugin status
    }

    /// Get the count of loaded dynamic plugins.
    pub fn loaded_plugin_count(&self) -> usize {
        self.plugin_loader.loaded_plugins().count()
    }

    /// Handle a plugin panic by adding it to pending errors and showing the popup.
    /// Called when a runtime panic occurs during plugin execution.
    /// Note: Currently only used in tests, will be called from generate workflow in future phases.
    #[cfg(test)]
    pub fn handle_plugin_panic(&mut self, error: PluginLoadError) {
        // Add to pending errors for display
        self.pending_plugin_errors.push(error);
        self.show_plugin_error_popup = true;
    }

    /// Get a mutable reference to the plugin loader.
    /// Used for calling plugin methods safely with panic catching.
    /// Note: Currently only used in tests, will be called from generate workflow in future phases.
    #[cfg(test)]
    pub fn plugin_loader_mut(&mut self) -> &mut PluginLoader {
        &mut self.plugin_loader
    }

    /// Execute the move: extract item+subtree from current list, add to destination
    pub fn execute_move_to_project(&mut self, dest_project: &Project) -> Result<usize> {
        use crate::storage::file::{load_todo_list_for_project, save_todo_list_for_project};

        let item_index = match &self.move_to_project_state {
            Some(MoveToProjectSubState::Selecting { item_index, .. }) => *item_index,
            None => return Err(anyhow::anyhow!("No move in progress")),
        };

        // Get the range of the item and its children
        let (start, end) = self.todo_list.get_item_range(item_index)?;
        let items_to_move: Vec<crate::todo::TodoItem> = self.todo_list.items[start..end].to_vec();
        let count = items_to_move.len();

        // Load destination project's todo list (for today)
        let today = chrono::Local::now().date_naive();
        let mut dest_list = load_todo_list_for_project(&dest_project.name, today)?;

        // Normalize indent levels: make the moved item's root indent 0
        let base_indent = items_to_move[0].indent_level;
        let mut normalized_items: Vec<crate::todo::TodoItem> = items_to_move
            .into_iter()
            .map(|mut item| {
                item.indent_level = item.indent_level.saturating_sub(base_indent);
                item.id = uuid::Uuid::new_v4();  // New IDs for destination
                item.parent_id = None;  // Will be recalculated
                item
            })
            .collect();

        // Append to destination list
        dest_list.items.append(&mut normalized_items);
        dest_list.recalculate_parent_ids();

        // Save destination list
        save_todo_list_for_project(&dest_list, &dest_project.name)?;

        // Remove from source list
        self.save_undo();
        self.todo_list.remove_item_range(start, end)?;
        self.clamp_cursor();
        self.unsaved_changes = true;

        Ok(count)
    }

    /// Fire an event to all subscribed plugins.
    ///
    /// Does nothing if currently applying hook results (cascade prevention).
    pub fn fire_event(&self, event: FfiEvent) {
        if self.in_hook_apply {
            return; // Prevent cascade
        }

        let event_type = event.event_type();
        let subscribed = self.plugin_loader.plugins_for_event(event_type);

        for (plugin, timeout) in subscribed {
            self.hook_dispatcher
                .dispatch_to_plugin(event.clone(), plugin, timeout);
        }
    }

    /// Poll hook results and apply commands.
    ///
    /// Called from UI event loop each frame.
    /// Commands are applied without undo (hooks are secondary effects).
    pub fn apply_pending_hook_results(&mut self) {
        let results = self.hook_dispatcher.poll_results();

        for result in results {
            // Handle errors
            if let Some(error) = result.error {
                self.pending_plugin_errors
                    .push(crate::plugin::loader::PluginLoadError {
                        plugin_name: result.plugin_name.clone(),
                        error_kind: crate::plugin::loader::PluginErrorKind::Panicked {
                            message: error.clone(),
                        },
                        message: format!("Hook failed: {}", error),
                    });
                self.show_plugin_error_popup = true;
                continue;
            }

            if result.commands.is_empty() {
                continue;
            }

            tracing::info!(
                plugin = %result.plugin_name,
                command_count = result.commands.len(),
                "Applying hook commands"
            );

            // Apply commands WITHOUT undo snapshot (intentional design decision).
            // Hook modifications are secondary effects, not user-initiated actions.
            // If user undoes the original action, hook effects would become orphaned.
            // This is consistent with Phase 9's CommandExecutor which provides the
            // execute_batch() method - undo snapshot is caller's responsibility.
            // For hooks, we deliberately skip the snapshot.
            self.in_hook_apply = true;

            let mut executor =
                crate::plugin::command_executor::CommandExecutor::new(result.plugin_name.clone());

            match executor.execute_batch(result.commands, &mut self.todo_list) {
                Ok(_) => {
                    // Save immediately to persist plugin changes
                    if let Err(e) = crate::storage::file::save_todo_list_for_project(
                        &self.todo_list,
                        &self.current_project.name,
                    ) {
                        tracing::warn!(
                            plugin = %result.plugin_name,
                            error = %e,
                            "Failed to save after hook commands"
                        );
                    } else {
                        tracing::debug!(
                            plugin = %result.plugin_name,
                            "Applied and saved hook commands"
                        );
                    }
                    self.unsaved_changes = false;
                }
                Err(e) => {
                    tracing::warn!(
                        plugin = %result.plugin_name,
                        error = %e,
                        "Hook command execution failed"
                    );
                }
            }

            self.in_hook_apply = false;
        }
    }

    /// Convert a TodoItem to FfiTodoItem at the given index.
    fn todo_to_ffi(&self, index: usize) -> Option<totui_plugin_interface::FfiTodoItem> {
        self.todo_list.items.get(index).map(|item| item.into())
    }

    /// Fire OnLoad event to subscribed plugins.
    ///
    /// Called once after todo list is loaded, before first render.
    pub fn fire_on_load_event(&self) {
        let event = FfiEvent::OnLoad {
            project_name: self.current_project.name.clone().into(),
            date: self.todo_list.date.format("%Y-%m-%d").to_string().into(),
        };
        self.fire_event(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_dismiss_upgrade() {
        use crate::keybindings::KeybindingCache;
        use crate::plugin::{PluginActionRegistry, PluginLoader};
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
            None,
            None,
            Project::default_project(),
            PluginLoader::new(),
            vec![],
            PluginActionRegistry::new(),
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

    #[test]
    fn test_toggle_cascades_to_children() {
        use crate::keybindings::KeybindingCache;
        use crate::plugin::{PluginActionRegistry, PluginLoader};
        use crate::todo::{TodoList, TodoState};
        use crate::ui::theme::Theme;
        use chrono::Local;

        let date = Local::now().date_naive();
        let mut todo_list = TodoList {
            date,
            items: vec![],
            file_path: std::path::PathBuf::from("/tmp/test.md"),
        };

        // Create parent with two children
        todo_list.add_item_with_indent("Parent".to_string(), 0);
        todo_list.add_item_with_indent("Child 1".to_string(), 1);
        todo_list.add_item_with_indent("Child 2".to_string(), 1);

        let mut state = AppState::new(
            todo_list,
            Theme::default(),
            KeybindingCache::default(),
            1000,
            None,
            None,
            Project::default_project(),
            PluginLoader::new(),
            vec![],
            PluginActionRegistry::new(),
        );

        // Set cursor to parent (index 0)
        state.cursor_position = 0;

        // Toggle - should affect parent and all children
        state.toggle_current_item_state();

        // All items should now be Checked
        assert_eq!(state.todo_list.items[0].state, TodoState::Checked);
        assert_eq!(state.todo_list.items[1].state, TodoState::Checked);
        assert_eq!(state.todo_list.items[2].state, TodoState::Checked);
    }

    #[test]
    fn test_toggle_cascade_undo_restores_all() {
        use crate::keybindings::KeybindingCache;
        use crate::plugin::{PluginActionRegistry, PluginLoader};
        use crate::todo::{TodoList, TodoState};
        use crate::ui::theme::Theme;
        use chrono::Local;

        let date = Local::now().date_naive();
        let mut todo_list = TodoList {
            date,
            items: vec![],
            file_path: std::path::PathBuf::from("/tmp/test.md"),
        };

        // Create parent (Empty), child1 (already Checked), child2 (Empty)
        todo_list.add_item_with_indent("Parent".to_string(), 0);
        todo_list.add_item_with_indent("Child 1".to_string(), 1);
        todo_list.add_item_with_indent("Child 2".to_string(), 1);

        // Set child1 to Checked before creating state
        todo_list.items[1].state = TodoState::Checked;

        let mut state = AppState::new(
            todo_list,
            Theme::default(),
            KeybindingCache::default(),
            1000,
            None,
            None,
            Project::default_project(),
            PluginLoader::new(),
            vec![],
            PluginActionRegistry::new(),
        );

        // Set cursor to parent
        state.cursor_position = 0;

        // Toggle - all should become Checked
        state.toggle_current_item_state();
        assert_eq!(state.todo_list.items[0].state, TodoState::Checked);
        assert_eq!(state.todo_list.items[1].state, TodoState::Checked);
        assert_eq!(state.todo_list.items[2].state, TodoState::Checked);

        // Undo - should restore all to original states
        state.undo();
        assert_eq!(state.todo_list.items[0].state, TodoState::Empty);
        assert_eq!(state.todo_list.items[1].state, TodoState::Checked); // Was already Checked
        assert_eq!(state.todo_list.items[2].state, TodoState::Empty);
    }

    #[test]
    fn test_toggle_cascade_unchecks_all() {
        use crate::keybindings::KeybindingCache;
        use crate::plugin::{PluginActionRegistry, PluginLoader};
        use crate::todo::{TodoList, TodoState};
        use crate::ui::theme::Theme;
        use chrono::Local;

        let date = Local::now().date_naive();
        let mut todo_list = TodoList {
            date,
            items: vec![],
            file_path: std::path::PathBuf::from("/tmp/test.md"),
        };

        // Create parent and child, both Checked
        todo_list.add_item_with_indent("Parent".to_string(), 0);
        todo_list.add_item_with_indent("Child".to_string(), 1);

        // Set both to Checked
        todo_list.items[0].state = TodoState::Checked;
        todo_list.items[1].state = TodoState::Checked;

        let mut state = AppState::new(
            todo_list,
            Theme::default(),
            KeybindingCache::default(),
            1000,
            None,
            None,
            Project::default_project(),
            PluginLoader::new(),
            vec![],
            PluginActionRegistry::new(),
        );

        // Set cursor to parent
        state.cursor_position = 0;

        // Toggle - both should become Empty
        state.toggle_current_item_state();
        assert_eq!(state.todo_list.items[0].state, TodoState::Empty);
        assert_eq!(state.todo_list.items[1].state, TodoState::Empty);
    }

    #[test]
    fn test_handle_plugin_panic() {
        use crate::keybindings::KeybindingCache;
        use crate::plugin::{
            PluginActionRegistry, PluginErrorKind, PluginLoadError, PluginLoader,
        };
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
            None,
            None,
            Project::default_project(),
            PluginLoader::new(),
            vec![],
            PluginActionRegistry::new(),
        );

        // Initially no errors and popup not shown
        assert!(state.pending_plugin_errors.is_empty());
        assert!(!state.show_plugin_error_popup);

        // Handle a plugin panic
        let error = PluginLoadError {
            plugin_name: "test-plugin".to_string(),
            error_kind: PluginErrorKind::Panicked {
                message: "test panic".to_string(),
            },
            message: "Plugin test-plugin panicked: test panic".to_string(),
        };
        state.handle_plugin_panic(error);

        // Now should have error and popup shown
        assert_eq!(state.pending_plugin_errors.len(), 1);
        assert!(state.show_plugin_error_popup);
        assert_eq!(state.pending_plugin_errors[0].plugin_name, "test-plugin");
    }

    #[test]
    fn test_plugin_loader_mut() {
        use crate::keybindings::KeybindingCache;
        use crate::plugin::{PluginActionRegistry, PluginLoader};
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
            None,
            None,
            Project::default_project(),
            PluginLoader::new(),
            vec![],
            PluginActionRegistry::new(),
        );

        // Should be able to get mutable reference to plugin loader
        let loader = state.plugin_loader_mut();
        // Verify it's the loader (no plugins loaded by default)
        assert_eq!(loader.loaded_plugins().count(), 0);
    }

    #[test]
    fn test_config_error_appears_in_pending_errors() {
        use crate::keybindings::KeybindingCache;
        use crate::plugin::{PluginActionRegistry, PluginErrorKind, PluginLoadError, PluginLoader};
        use crate::todo::TodoList;
        use crate::ui::theme::Theme;
        use chrono::Local;

        let date = Local::now().date_naive();
        let todo_list = TodoList {
            date,
            items: vec![],
            file_path: std::path::PathBuf::from("/tmp/test.md"),
        };

        // Create a config error (simulating what main.rs does when converting ConfigError)
        let config_error = PluginLoadError {
            plugin_name: "jira-plugin".to_string(),
            error_kind: PluginErrorKind::Other(
                "Config: Missing required field 'api_key'".to_string(),
            ),
            message: "Missing required field 'api_key'".to_string(),
        };

        let state = AppState::new(
            todo_list,
            Theme::default(),
            KeybindingCache::default(),
            1000,
            None,
            None,
            Project::default_project(),
            PluginLoader::new(),
            vec![config_error],
            PluginActionRegistry::new(),
        );

        // Config errors passed during construction should appear in pending_plugin_errors
        assert_eq!(state.pending_plugin_errors.len(), 1);
        assert_eq!(state.pending_plugin_errors[0].plugin_name, "jira-plugin");
        assert!(state.pending_plugin_errors[0].message.contains("api_key"));

        // Popup should be shown when there are errors
        assert!(state.show_plugin_error_popup);
    }

    #[test]
    fn test_dismiss_plugin_error_popup() {
        use crate::keybindings::KeybindingCache;
        use crate::plugin::{
            PluginActionRegistry, PluginErrorKind, PluginLoadError, PluginLoader,
        };
        use crate::todo::TodoList;
        use crate::ui::theme::Theme;
        use chrono::Local;

        let date = Local::now().date_naive();
        let todo_list = TodoList {
            date,
            items: vec![],
            file_path: std::path::PathBuf::from("/tmp/test.md"),
        };

        // Create error to trigger popup
        let error = PluginLoadError {
            plugin_name: "test-plugin".to_string(),
            error_kind: PluginErrorKind::LibraryCorrupted,
            message: "Plugin corrupted".to_string(),
        };

        let mut state = AppState::new(
            todo_list,
            Theme::default(),
            KeybindingCache::default(),
            1000,
            None,
            None,
            Project::default_project(),
            PluginLoader::new(),
            vec![error],
            PluginActionRegistry::new(),
        );

        // Popup should be shown initially
        assert!(state.show_plugin_error_popup);

        // Dismiss popup
        state.dismiss_plugin_error_popup();

        // Popup should be hidden but errors still accessible
        assert!(!state.show_plugin_error_popup);
        assert_eq!(state.pending_plugin_errors.len(), 1);
    }
}
