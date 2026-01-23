---
phase: quick
plan: 006
type: execute
wave: 1
depends_on: []
files_modified:
  - src/app/state.rs
  - src/app/mode.rs
  - src/app/event.rs
  - src/keybindings/mod.rs
  - src/storage/file.rs
  - src/ui/components/mod.rs
autonomous: true
must_haves:
  truths:
    - "User can press a key to initiate move-to-project for current item"
    - "User sees a modal listing available projects (excluding current project)"
    - "User can select a destination project with j/k and Enter"
    - "Item and all children are removed from current list and added to destination"
    - "Both source and destination files are saved"
  artifacts:
    - path: "src/app/state.rs"
      provides: "MoveToProjectSubState enum, open_move_to_project_modal(), execute_move_to_project() methods"
    - path: "src/app/mode.rs"
      provides: "Mode::MoveToProject variant"
    - path: "src/app/event.rs"
      provides: "handle_move_to_project_mode() function and Action::MoveToProject handling"
    - path: "src/keybindings/mod.rs"
      provides: "Action::MoveToProject variant and default keybinding (m)"
  key_links:
    - from: "src/app/event.rs"
      to: "src/app/state.rs"
      via: "execute_move_to_project() call"
      pattern: "execute_move_to_project"
    - from: "src/app/state.rs"
      to: "src/storage/file.rs"
      via: "save_todo_list_for_project() calls for both source and destination"
      pattern: "save_todo_list_for_project"
---

<objective>
Implement the ability to move a todo item (and its entire subtree of children) to another project/date file.

Purpose: Allow users to reorganize todos between projects without manual copy/paste, preserving hierarchy.

Output: New "Move to Project" feature accessible via keybinding, with modal UI for project selection.
</objective>

<execution_context>
@~/.claude/get-shit-done/workflows/execute-plan.md
@~/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@src/app/state.rs (AppState, ProjectSubState pattern to follow)
@src/app/mode.rs (Mode enum)
@src/app/event.rs (event handling patterns, handle_project_select_mode as reference)
@src/keybindings/mod.rs (Action enum, keybinding registration)
@src/storage/file.rs (load_todo_list_for_project, save_todo_list_for_project)
@src/todo/hierarchy.rs (get_item_range for extracting subtrees)
@src/project/registry.rs (ProjectRegistry, Project)
</context>

<tasks>

<task type="auto">
  <name>Task 1: Add MoveToProject mode, sub-state, and core methods</name>
  <files>
    src/app/mode.rs
    src/app/state.rs
  </files>
  <action>
1. In `src/app/mode.rs`:
   - Add `MoveToProject` variant to the `Mode` enum (after `ProjectSelect`)
   - Add Display impl case: `Mode::MoveToProject => write!(f, "MOVE")`

2. In `src/app/state.rs`:
   - Add new sub-state enum after `ProjectSubState`:
     ```rust
     /// Move to project modal sub-state
     #[derive(Debug, Clone)]
     pub enum MoveToProjectSubState {
         Selecting {
             projects: Vec<Project>,
             selected_index: usize,
             item_index: usize,  // Index of item being moved
         },
     }
     ```
   - Add field to `AppState` struct (after `project_state`):
     ```rust
     /// Move to project modal state
     pub move_to_project_state: Option<MoveToProjectSubState>,
     ```
   - Initialize `move_to_project_state: None` in `AppState::new()`

3. Add methods to `impl AppState`:
   ```rust
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

   /// Execute the move: extract item+subtree from current list, add to destination
   pub fn execute_move_to_project(&mut self, dest_project: &Project) -> Result<usize> {
       use crate::storage::file::{load_todo_list_for_project, save_todo_list_for_project};

       let item_index = match &self.move_to_project_state {
           Some(MoveToProjectSubState::Selecting { item_index, .. }) => *item_index,
           None => return Err(anyhow::anyhow!("No move in progress")),
       };

       // Get the range of the item and its children
       let (start, end) = self.todo_list.get_item_range(item_index)?;
       let items_to_move: Vec<TodoItem> = self.todo_list.items[start..end].to_vec();
       let count = items_to_move.len();

       // Load destination project's todo list (for today)
       let today = chrono::Local::now().date_naive();
       let mut dest_list = load_todo_list_for_project(&dest_project.name, today)?;

       // Normalize indent levels: make the moved item's root indent 0
       let base_indent = items_to_move[0].indent_level;
       let mut normalized_items: Vec<TodoItem> = items_to_move
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
   ```
  </action>
  <verify>
    `cargo check` passes with no errors related to the new types and methods.
  </verify>
  <done>
    - Mode::MoveToProject exists in mode.rs
    - MoveToProjectSubState enum exists in state.rs
    - AppState has move_to_project_state field
    - open_move_to_project_modal(), close_move_to_project_modal(), execute_move_to_project() methods exist
  </done>
</task>

<task type="auto">
  <name>Task 2: Add keybinding action and event handler</name>
  <files>
    src/keybindings/mod.rs
    src/app/event.rs
  </files>
  <action>
1. In `src/keybindings/mod.rs`:
   - Add `MoveToProject` variant to the `Action` enum (after `OpenProjectModal`)
   - Add to `Action::as_str()`: `Action::MoveToProject => "move_to_project",`
   - Add to `Action::from_str()`: `"move_to_project" => Ok(Action::MoveToProject),`
   - Add default keybinding in `default_navigate()` bindings: `(KeyBinding::from_char('m'), Action::MoveToProject),`
   - Add to help text in `default_navigate()` keybindings map: include 'm' for move_to_project

2. In `src/app/event.rs`:
   - Import `MoveToProjectSubState` in the use statement with other state imports
   - Add `Mode::MoveToProject => handle_move_to_project_mode(key, state)?` to `handle_key_event` match
   - Add `Action::MoveToProject` to the `dominated_by_readonly` matches block (it should be blocked in readonly mode)
   - Add handling in `execute_navigate_action`:
     ```rust
     Action::MoveToProject => {
         state.open_move_to_project_modal();
     }
     ```
   - Add new handler function (pattern after `handle_project_select_mode`):
     ```rust
     fn handle_move_to_project_mode(key: KeyEvent, state: &mut AppState) -> Result<()> {
         let move_state = match state.move_to_project_state.take() {
             Some(ms) => ms,
             None => {
                 state.close_move_to_project_modal();
                 return Ok(());
             }
         };

         match move_state {
             MoveToProjectSubState::Selecting {
                 projects,
                 mut selected_index,
                 item_index,
             } => {
                 match key.code {
                     KeyCode::Esc | KeyCode::Char('q') => {
                         state.close_move_to_project_modal();
                     }
                     KeyCode::Up | KeyCode::Char('k') => {
                         selected_index = selected_index.saturating_sub(1);
                         state.move_to_project_state = Some(MoveToProjectSubState::Selecting {
                             projects,
                             selected_index,
                             item_index,
                         });
                     }
                     KeyCode::Down | KeyCode::Char('j') => {
                         if selected_index < projects.len().saturating_sub(1) {
                             selected_index += 1;
                         }
                         state.move_to_project_state = Some(MoveToProjectSubState::Selecting {
                             projects,
                             selected_index,
                             item_index,
                         });
                     }
                     KeyCode::Enter => {
                         if let Some(dest_project) = projects.get(selected_index) {
                             let dest_project = dest_project.clone();
                             // Re-set state temporarily so execute_move_to_project can read item_index
                             state.move_to_project_state = Some(MoveToProjectSubState::Selecting {
                                 projects: projects.clone(),
                                 selected_index,
                                 item_index,
                             });

                             match state.execute_move_to_project(&dest_project) {
                                 Ok(count) => {
                                     state.set_status_message(format!(
                                         "Moved {} item(s) to '{}'",
                                         count,
                                         dest_project.name
                                     ));
                                     // Save source list
                                     save_todo_list_for_project(&state.todo_list, &state.current_project.name)?;
                                     state.unsaved_changes = false;
                                     state.last_save_time = Some(std::time::Instant::now());
                                 }
                                 Err(e) => {
                                     state.set_status_message(format!("Move failed: {}", e));
                                 }
                             }
                             state.close_move_to_project_modal();
                         }
                     }
                     _ => {
                         state.move_to_project_state = Some(MoveToProjectSubState::Selecting {
                             projects,
                             selected_index,
                             item_index,
                         });
                     }
                 }
             }
         }
         Ok(())
     }
     ```
  </action>
  <verify>
    `cargo check` passes. `cargo test` passes (existing tests should not break).
  </verify>
  <done>
    - Action::MoveToProject exists with as_str/from_str implementations
    - Default keybinding 'm' mapped to MoveToProject
    - handle_move_to_project_mode function handles Esc, j/k navigation, and Enter selection
    - MoveToProject is blocked in readonly mode
  </done>
</task>

<task type="auto">
  <name>Task 3: Add UI rendering for move-to-project modal</name>
  <files>
    src/ui/components/mod.rs
  </files>
  <action>
1. In `src/ui/components/mod.rs`, add a new rendering function for the move-to-project modal.
   Follow the pattern of `render_project_modal()` but simpler (just selection, no create/rename/delete):

   ```rust
   pub fn render_move_to_project_modal(frame: &mut Frame, state: &AppState) {
       use crate::app::state::MoveToProjectSubState;

       let move_state = match &state.move_to_project_state {
           Some(s) => s,
           None => return,
       };

       let MoveToProjectSubState::Selecting {
           projects,
           selected_index,
           item_index,
       } = move_state;

       // Get the item being moved for display
       let item_name = state
           .todo_list
           .items
           .get(*item_index)
           .map(|i| i.content.as_str())
           .unwrap_or("(unknown)");

       let area = frame.area();
       let width = 50.min(area.width.saturating_sub(4));
       let height = (projects.len() + 6).min(area.height.saturating_sub(4) as usize) as u16;

       let x = (area.width.saturating_sub(width)) / 2;
       let y = (area.height.saturating_sub(height)) / 2;
       let modal_area = Rect::new(x, y, width, height);

       // Clear background
       frame.render_widget(Clear, modal_area);

       // Build title with truncated item name
       let max_title_len = (width as usize).saturating_sub(10);
       let truncated_name = if item_name.len() > max_title_len {
           format!("{}...", &item_name[..max_title_len.saturating_sub(3)])
       } else {
           item_name.to_string()
       };
       let title = format!(" Move '{}' to ", truncated_name);

       let block = Block::default()
           .title(title)
           .borders(Borders::ALL)
           .border_style(Style::default().fg(state.theme.border));

       let inner = block.inner(modal_area);
       frame.render_widget(block, modal_area);

       // Render project list
       let items: Vec<ListItem> = projects
           .iter()
           .enumerate()
           .map(|(i, project)| {
               let style = if i == *selected_index {
                   Style::default()
                       .fg(state.theme.selected_fg)
                       .bg(state.theme.selected_bg)
               } else {
                   Style::default().fg(state.theme.text)
               };
               ListItem::new(Line::from(Span::styled(&project.name, style)))
           })
           .collect();

       let list = List::new(items);
       frame.render_widget(list, inner);
   }
   ```

2. In `src/ui/mod.rs`, in the `render_app` or equivalent main render function, add a call to render the modal when `state.mode == Mode::MoveToProject`:
   ```rust
   if state.mode == Mode::MoveToProject {
       components::render_move_to_project_modal(frame, state);
   }
   ```

   (Place this near the other modal renders like project_modal, rollover_modal, etc.)
  </action>
  <verify>
    `cargo build --release` compiles successfully. Run the TUI with `cargo run` and:
    1. Create a second project (press 'p' for project modal, 'n' for new, enter a name)
    2. Add a todo item with children (use 'o' to add, Tab to indent)
    3. Press 'm' on the parent item - should see a modal with project list
    4. Press Enter to move - item should disappear from current list
    5. Switch to the other project ('p', select it) - moved items should be there at root level
  </verify>
  <done>
    - render_move_to_project_modal function exists and renders a project selection modal
    - Modal shows the item name being moved in the title
    - Modal is rendered when mode is MoveToProject
    - Full flow works: 'm' opens modal, j/k navigate, Enter moves, Esc cancels
  </done>
</task>

</tasks>

<verification>
1. `cargo check` - all new types and methods compile
2. `cargo test` - existing tests pass (no regressions)
3. `cargo clippy` - no new warnings
4. Manual test: create two projects, add nested todos, move parent to other project, verify hierarchy preserved
</verification>

<success_criteria>
- User can press 'm' on any todo item to open move-to-project modal
- Modal shows list of projects (excluding current project)
- User can navigate with j/k and select with Enter
- Item and all children are moved to destination project's today file
- Source and destination lists are both saved
- Undo works for the source list (restores moved items)
- Modal can be dismissed with Esc without making changes
</success_criteria>

<output>
After completion, create `.planning/quick/006-move-item-and-subtree-to-another-project/006-SUMMARY.md`
</output>
