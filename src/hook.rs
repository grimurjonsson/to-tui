//! Claude Code `Stop` hook: keep a totui tree honest without the model having to
//! remember to update it.
//!
//! A skill is a prompt, so over a long task the model drifts and stops updating
//! the tree. This hook runs at every turn end, inspects the tree the session is
//! working against, and injects a factual reminder **only when an invariant is
//! broken**. The harness runs it, so it cannot be forgotten.
//!
//! Two properties matter more than the checks themselves:
//!
//! - **Silence by default.** It fires on every turn end of every session on the
//!   machine, including the vast majority that never touch totui. Anything other
//!   than "no output" must be earned.
//! - **No model cooperation.** The session-to-tree association is discovered and
//!   maintained here, not written by the model, because "the model remembers to
//!   do a thing" is the exact failure this hook exists to fix.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::mcp::schemas::TodoItemResponse;

/// Turns an active leaf may sit unchanged before it is called stalled.
/// Deliberately generous: a single milestone legitimately spans many turns, and
/// a false "you are stalled" is worse than a late one.
const STALE_AFTER_TURNS: u64 = 12;

/// States that mean a leaf needs no further work.
fn is_terminal(state: &str) -> bool {
    state == "x" || state == "-"
}

fn is_active(state: &str) -> bool {
    state == "*"
}

/// The JSON Claude Code writes to the hook's stdin. Only the fields this hook
/// uses are declared; unknown fields are ignored so new harness versions do not
/// break it.
#[derive(Debug, Deserialize)]
pub struct StopPayload {
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub cwd: String,
    #[serde(default)]
    pub turn_number: u64,
}

/// The JSON for a `SessionStart` event.
#[derive(Debug, Deserialize)]
pub struct SessionStartPayload {
    #[serde(default)]
    pub session_id: String,
    /// `startup` | `resume` | `clear` | `compact` | `fork`
    #[serde(default)]
    pub source: String,
}

/// The JSON for a `SessionEnd` event.
#[derive(Debug, Deserialize)]
pub struct SessionEndPayload {
    #[serde(default)]
    pub session_id: String,
    /// `clear` | `resume` | `logout` | `prompt_input_exit` | `bypass_permissions_disabled` | `other`
    #[serde(default)]
    pub reason: String,
}

/// Whether a session ending for this reason should give up its claim.
///
/// `resume` means the session is paused and coming back with the same id, so
/// releasing there would drop a claim that is still live.
pub fn should_release_claim(reason: &str) -> bool {
    reason != "resume"
}

/// Sources where the model has lost its context and needs telling what tree it
/// is driving. `startup` is excluded: a brand-new session has no history to
/// have forgotten, and the skill will establish its own tree.
pub fn needs_context_reinjection(source: &str) -> bool {
    matches!(source, "resume" | "compact" | "fork")
}

/// What the hook prints on stdout. Absent output means "nothing to say".
#[derive(Debug, Serialize)]
pub struct HookOutput {
    #[serde(rename = "hookSpecificOutput")]
    pub hook_specific_output: HookSpecificOutput,
}

#[derive(Debug, Serialize)]
pub struct HookSpecificOutput {
    #[serde(rename = "hookEventName")]
    pub hook_event_name: &'static str,
    #[serde(rename = "additionalContext")]
    pub additional_context: String,
}

impl HookOutput {
    pub fn stop(context: String) -> Self {
        Self::for_event("Stop", context)
    }

    pub fn session_start(context: String) -> Self {
        Self::for_event("SessionStart", context)
    }

    fn for_event(event: &'static str, context: String) -> Self {
        Self {
            hook_specific_output: HookSpecificOutput {
                hook_event_name: event,
                additional_context: context,
            },
        }
    }
}

/// Release a session's claim. Missing file is success — the goal is that no
/// claim remains, not that one was there.
pub fn release_session(session_id: &str) -> anyhow::Result<()> {
    let path = session_path(session_id)?;
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

/// Delete claim files untouched for `max_age_days`.
///
/// A session that dies without a SessionEnd — crash, SIGKILL, machine reboot —
/// leaves its claim behind forever, which would silently block that tree from
/// ever being re-claimed. Returns how many were removed.
pub fn sweep_stale_claims(max_age_days: u64) -> usize {
    let Ok(dir) = sessions_dir() else {
        return 0;
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    let max_age = std::time::Duration::from_secs(max_age_days * 24 * 60 * 60);
    let now = std::time::SystemTime::now();

    entries
        .flatten()
        .filter(|e| {
            e.metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| now.duration_since(t).ok())
                .is_some_and(|age| age > max_age)
        })
        .filter(|e| std::fs::remove_file(e.path()).is_ok())
        .count()
}

/// A human-readable summary of where a tree stands, for re-injection after the
/// model has lost its context.
pub fn summarize(items: &[TodoItemResponse], root_id: &str) -> Option<String> {
    let root_idx = items.iter().position(|i| i.id == root_id)?;
    let (start, end) = subtree_range(items, root_idx);
    if end - start <= 1 {
        return None;
    }

    let leaves = leaves(items, start + 1, end);
    let total = leaves.len();
    let done = leaves.iter().filter(|l| is_terminal(&l.state)).count();
    let active: Vec<&str> = leaves
        .iter()
        .filter(|l| is_active(&l.state))
        .map(|l| l.content.as_str())
        .collect();

    let where_now = match active.as_slice() {
        [] => "no item is currently marked in progress".to_string(),
        [one] => format!("\"{one}\" is marked in progress"),
        many => format!("{} items are marked in progress", many.len()),
    };

    Some(format!(
        "This session is tracking the totui tree \"{}\": {done} of {total} items done, and {where_now}.",
        items[root_idx].content
    ))
}

/// Per-session association between a Claude Code session and the tree it drives.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SessionState {
    pub root_id: String,
    #[serde(default)]
    pub project: String,
    #[serde(default)]
    pub cwd: String,
    /// Active leaf at the last observation, used to detect a stalled leaf.
    #[serde(default)]
    pub last_active_leaf: Option<String>,
    /// Turn number when `last_active_leaf` last changed.
    #[serde(default)]
    pub last_change_turn: u64,
    /// Whether the "nothing is tracked" note has already been raised. Hooks have
    /// no way to prompt, so this fires once and then stays quiet for the session
    /// regardless of what the user decides.
    #[serde(default)]
    pub prompted: bool,
}

impl SessionState {
    /// A session that exists only to remember that the note was already raised.
    pub fn prompted_only() -> Self {
        Self {
            prompted: true,
            ..Default::default()
        }
    }

    pub fn has_claim(&self) -> bool {
        !self.root_id.is_empty()
    }
}

pub fn sessions_dir() -> anyhow::Result<PathBuf> {
    Ok(crate::utils::paths::get_to_tui_dir()?.join("hook-sessions"))
}

fn session_path(session_id: &str) -> anyhow::Result<PathBuf> {
    // Session ids come from the harness, but this value builds a filesystem path,
    // so refuse anything that could escape the directory.
    let safe: String = session_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    if safe.is_empty() {
        anyhow::bail!("empty session id");
    }
    Ok(sessions_dir()?.join(format!("{safe}.json")))
}

pub fn load_session(session_id: &str) -> Option<SessionState> {
    let path = session_path(session_id).ok()?;
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

pub fn save_session(session_id: &str, state: &SessionState) -> anyhow::Result<()> {
    let path = session_path(session_id)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(state)?)?;
    Ok(())
}

/// Root ids already claimed by a different session, so two Claude Codes working
/// at once never nudge each other about the same tree.
pub fn roots_claimed_by_others(current_session: &str) -> Vec<String> {
    let Ok(dir) = sessions_dir() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let current_file = format!("{current_session}.json");
    entries
        .flatten()
        .filter(|e| e.file_name() != std::ffi::OsStr::new(&current_file))
        .filter_map(|e| std::fs::read_to_string(e.path()).ok())
        .filter_map(|raw| serde_json::from_str::<SessionState>(&raw).ok())
        .map(|s| s.root_id)
        .collect()
}

/// Index range of a root's subtree: the root itself plus the contiguous run of
/// deeper-indented items that follow it.
fn subtree_range(items: &[TodoItemResponse], root_idx: usize) -> (usize, usize) {
    let base = items[root_idx].indent_level;
    let mut end = root_idx + 1;
    while end < items.len() && items[end].indent_level > base {
        end += 1;
    }
    (root_idx, end)
}

/// Items in a subtree that have no children of their own.
fn leaves(items: &[TodoItemResponse], start: usize, end: usize) -> Vec<&TodoItemResponse> {
    (start..end)
        .filter(|&i| {
            let next_is_child = items
                .get(i + 1)
                .filter(|_| i + 1 < end)
                .is_some_and(|n| n.indent_level > items[i].indent_level);
            !next_is_child
        })
        .map(|i| &items[i])
        .collect()
}

/// Top-level item that agent-created trees live under. Scoping candidates to its
/// children keeps the hook away from the user's own lists entirely.
pub const AGENT_ROOT: &str = "Claude Code";

/// Index of the agent root, if the list has one.
fn agent_root_idx(items: &[TodoItemResponse]) -> Option<usize> {
    items
        .iter()
        .position(|i| i.indent_level == 0 && i.content.trim() == AGENT_ROOT)
}

/// Trees a session may claim: the children of the agent root, or — when no agent
/// root exists — top-level items, so lists predating the convention still work.
///
/// Only trees containing an active leaf qualify. A tree the model built but never
/// started is invisible here; the ask-once note is what surfaces that case.
pub fn candidate_roots(items: &[TodoItemResponse]) -> Vec<String> {
    let (scope_start, scope_end, depth) = match agent_root_idx(items) {
        Some(idx) => {
            let (s, e) = subtree_range(items, idx);
            (s + 1, e, items[idx].indent_level + 1)
        }
        None => (0, items.len(), 0),
    };

    (scope_start..scope_end)
        .filter(|&i| items[i].indent_level == depth)
        .filter(|&i| {
            let (s, e) = subtree_range(items, i);
            items[s..e].iter().any(|it| is_active(&it.state))
        })
        .map(|i| items[i].id.clone())
        .collect()
}

/// Whether the list has anything a session could plausibly be tracking. Used to
/// decide if the "nothing is tracked" note is worth raising at all.
pub fn has_trackable_work(items: &[TodoItemResponse]) -> bool {
    let Some(idx) = agent_root_idx(items) else {
        return false;
    };
    let (s, e) = subtree_range(items, idx);
    items[s + 1..e].iter().any(|i| !is_terminal(&i.state))
}

/// The one-time note raised when a session has no tree to watch.
pub fn untracked_note() -> String {
    format!(
        "No totui tree is being tracked for this session, so progress is not visible in the user's TUI. If this session is doing multi-step work, it belongs under the \"{AGENT_ROOT}\" root; if not, no action is needed. This note is raised once per session."
    )
}

/// The single active leaf of a tree, when there is exactly one. Used to notice
/// when the active leaf changes, which resets the staleness clock.
pub fn active_leaf_id(items: &[TodoItemResponse], root_id: &str) -> Option<String> {
    let root_idx = items.iter().position(|i| i.id == root_id)?;
    let (start, end) = subtree_range(items, root_idx);
    let active: Vec<_> = leaves(items, start + 1, end)
        .into_iter()
        .filter(|l| is_active(&l.state))
        .collect();
    match active.as_slice() {
        [one] => Some(one.id.clone()),
        _ => None,
    }
}

/// Content of the tree's root, for message text.
pub fn root_content(items: &[TodoItemResponse], root_id: &str) -> Option<String> {
    items
        .iter()
        .find(|i| i.id == root_id)
        .map(|i| i.content.clone())
}

/// The outcome of checking one tree. `None` from [`evaluate`] means stay silent.
#[derive(Debug, PartialEq, Eq)]
pub enum Finding {
    /// A leaf finished but nothing was started after it.
    NoActiveLeaf { remaining: usize },
    /// More than one leaf claims to be in progress.
    MultipleActive { count: usize },
    /// Every leaf is terminal but the root was never closed.
    TreeUnclosed,
    /// The same leaf has been active for a long time.
    Stalled { content: String, turns: u64 },
}

impl Finding {
    /// Rendered as factual statements, never imperatives — per the hooks guidance
    /// that `additionalContext` should state what is true and let the model decide.
    pub fn message(&self, root_content: &str) -> String {
        match self {
            Self::NoActiveLeaf { remaining } => format!(
                "totui tree \"{root_content}\": no item is marked in progress, and {remaining} item(s) remain unfinished. The convention is that exactly one leaf carries '*' while work is in flight."
            ),
            Self::MultipleActive { count } => format!(
                "totui tree \"{root_content}\": {count} items are marked in progress at once. The convention is exactly one."
            ),
            Self::TreeUnclosed => format!(
                "totui tree \"{root_content}\": every item is finished but the tree itself is still open."
            ),
            Self::Stalled { content, turns } => format!(
                "totui tree \"{root_content}\": \"{content}\" has been marked in progress for {turns} turns."
            ),
        }
    }
}

/// Decide whether a tree warrants a nudge. Pure: all I/O happens in the caller.
///
/// `active_for_turns` is how long the currently active leaf has held that state,
/// or `None` when it just changed.
pub fn evaluate(
    items: &[TodoItemResponse],
    root_id: &str,
    active_for_turns: Option<u64>,
) -> Option<Finding> {
    let root_idx = items.iter().position(|i| i.id == root_id)?;
    let (start, end) = subtree_range(items, root_idx);

    // A bare root with no children is not a tracked tree.
    if end - start <= 1 {
        return None;
    }

    let leaves = leaves(items, start + 1, end);
    let active: Vec<_> = leaves.iter().filter(|l| is_active(&l.state)).collect();
    let unfinished = leaves.iter().filter(|l| !is_terminal(&l.state)).count();

    match active.len() {
        1 => {
            // Healthy — unless it has sat there too long.
            match active_for_turns {
                Some(turns) if turns >= STALE_AFTER_TURNS => Some(Finding::Stalled {
                    content: active[0].content.clone(),
                    turns,
                }),
                _ => None,
            }
        }
        0 if unfinished > 0 => Some(Finding::NoActiveLeaf {
            remaining: unfinished,
        }),
        0 => {
            // Everything is terminal; the only thing left to check is the root.
            if is_terminal(&items[root_idx].state) {
                None
            } else {
                Some(Finding::TreeUnclosed)
            }
        }
        n => Some(Finding::MultipleActive { count: n }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, content: &str, indent: usize, state: &str) -> TodoItemResponse {
        TodoItemResponse {
            id: id.to_string(),
            content: content.to_string(),
            state: state.to_string(),
            state_description: String::new(),
            indent_level: indent,
            parent_id: None,
            due_date: None,
            description: None,
        }
    }

    /// root + three leaves, with the given leaf states
    fn tree(states: [&str; 3], root_state: &str) -> Vec<TodoItemResponse> {
        vec![
            item("root", "Do the thing", 0, root_state),
            item("a", "First", 1, states[0]),
            item("b", "Second", 1, states[1]),
            item("c", "Third", 1, states[2]),
        ]
    }

    #[test]
    fn silent_when_exactly_one_leaf_is_active() {
        assert_eq!(evaluate(&tree(["x", "*", " "], "*"), "root", Some(1)), None);
    }

    #[test]
    fn nudges_when_a_leaf_finished_but_none_was_started() {
        // The "start" case: leaf marked done, next one never picked up.
        assert_eq!(
            evaluate(&tree(["x", " ", " "], "*"), "root", None),
            Some(Finding::NoActiveLeaf { remaining: 2 })
        );
    }

    #[test]
    fn nudges_when_two_leaves_are_active() {
        assert_eq!(
            evaluate(&tree(["*", "*", " "], "*"), "root", None),
            Some(Finding::MultipleActive { count: 2 })
        );
    }

    #[test]
    fn nudges_when_all_leaves_done_but_root_still_open() {
        assert_eq!(
            evaluate(&tree(["x", "x", "x"], "*"), "root", None),
            Some(Finding::TreeUnclosed)
        );
    }

    #[test]
    fn silent_when_the_whole_tree_is_closed() {
        assert_eq!(evaluate(&tree(["x", "x", "x"], "x"), "root", None), None);
    }

    #[test]
    fn cancelled_counts_as_finished() {
        // '-' is terminal, so a cancelled leaf must not read as outstanding work.
        assert_eq!(evaluate(&tree(["x", "-", "x"], "x"), "root", None), None);
    }

    #[test]
    fn question_and_important_still_count_as_unfinished() {
        // '?' means blocked awaiting a decision — real remaining work.
        assert_eq!(
            evaluate(&tree(["x", "?", "x"], "*"), "root", None),
            Some(Finding::NoActiveLeaf { remaining: 1 })
        );
    }

    #[test]
    fn stalls_only_after_the_threshold() {
        let t = tree(["x", "*", " "], "*");
        assert_eq!(evaluate(&t, "root", Some(STALE_AFTER_TURNS - 1)), None);
        assert_eq!(
            evaluate(&t, "root", Some(STALE_AFTER_TURNS)),
            Some(Finding::Stalled {
                content: "Second".to_string(),
                turns: STALE_AFTER_TURNS
            })
        );
    }

    #[test]
    fn a_root_with_no_children_is_not_a_tracked_tree() {
        let items = vec![item("root", "Lone item", 0, "*")];
        assert_eq!(evaluate(&items, "root", None), None);
    }

    #[test]
    fn unknown_root_is_silent_rather_than_an_error() {
        // The tree may have been deleted in the TUI mid-session.
        assert_eq!(evaluate(&tree(["*", " ", " "], "*"), "gone", None), None);
    }

    #[test]
    fn a_parent_with_children_is_not_itself_a_leaf() {
        // Only the deepest items count, so a '*' on a grouping node does not
        // satisfy the one-active-leaf rule on its own.
        let items = vec![
            item("root", "Root", 0, "*"),
            item("group", "Group", 1, "*"),
            item("g1", "Child one", 2, "x"),
            item("g2", "Child two", 2, " "),
        ];
        assert_eq!(
            evaluate(&items, "root", None),
            Some(Finding::NoActiveLeaf { remaining: 1 })
        );
    }

    #[test]
    fn subtree_stops_at_the_next_root() {
        // A second tree's items must not leak into the first tree's evaluation.
        let items = vec![
            item("root", "First tree", 0, "*"),
            item("a", "Leaf", 1, "*"),
            item("other", "Second tree", 0, " "),
            item("b", "Other leaf", 1, " "),
        ];
        assert_eq!(evaluate(&items, "root", Some(0)), None);
    }

    #[test]
    fn candidates_are_scoped_to_the_agent_root_when_one_exists() {
        // The user's own top-level lists must never become claim candidates,
        // even when they contain active items.
        let items = vec![
            item("vko", "VKO", 0, "*"),
            item("vko1", "Some user task", 1, "*"),
            item("cc", AGENT_ROOT, 0, "*"),
            item("t1", "Agent tree", 1, "*"),
            item("t1a", "Leaf", 2, "*"),
        ];
        assert_eq!(candidate_roots(&items), vec!["t1".to_string()]);
    }

    #[test]
    fn falls_back_to_top_level_when_there_is_no_agent_root() {
        // Lists predating the convention still work.
        let items = vec![item("r", "Old tree", 0, "*"), item("a", "Leaf", 1, "*")];
        assert_eq!(candidate_roots(&items), vec!["r".to_string()]);
    }

    #[test]
    fn trackable_work_means_unfinished_items_under_the_agent_root() {
        let idle = vec![
            item("cc", AGENT_ROOT, 0, " "),
            item("t", "Finished tree", 1, "x"),
        ];
        assert!(!has_trackable_work(&idle));

        let live = vec![
            item("cc", AGENT_ROOT, 0, " "),
            item("t", "Unstarted tree", 1, " "),
        ];
        assert!(has_trackable_work(&live));
    }

    #[test]
    fn no_agent_root_means_nothing_trackable_so_the_note_never_fires() {
        // The overwhelming majority of sessions on a machine. They must never
        // hear from this hook at all.
        let items = vec![item("vko", "VKO", 0, "*"), item("a", "User leaf", 1, "*")];
        assert!(!has_trackable_work(&items));
    }

    #[test]
    fn the_untracked_note_states_the_situation_without_ordering_anything() {
        let note = untracked_note();
        assert!(note.contains(AGENT_ROOT));
        assert!(note.contains("once per session"));
        for imperative in ["You must", "Create a", "Remember to"] {
            assert!(!note.contains(imperative), "imperative in: {note}");
        }
    }

    #[test]
    fn prompted_only_state_carries_no_claim() {
        let s = SessionState::prompted_only();
        assert!(s.prompted);
        assert!(!s.has_claim());
    }

    #[test]
    fn candidate_roots_finds_only_trees_containing_active_work() {
        let items = vec![
            item("r1", "Idle tree", 0, " "),
            item("a", "Leaf", 1, " "),
            item("r2", "Working tree", 0, "*"),
            item("b", "Leaf", 1, "*"),
        ];
        assert_eq!(candidate_roots(&items), vec!["r2".to_string()]);
    }

    #[test]
    fn candidate_roots_is_empty_when_nothing_is_active() {
        // The common case machine-wide: totui not in use, so the hook stays quiet.
        let items = vec![item("r1", "Idle", 0, " "), item("a", "Leaf", 1, " ")];
        assert!(candidate_roots(&items).is_empty());
    }

    #[test]
    fn messages_are_factual_not_imperative() {
        let root = "Ship the thing";
        for finding in [
            Finding::NoActiveLeaf { remaining: 2 },
            Finding::MultipleActive { count: 3 },
            Finding::TreeUnclosed,
            Finding::Stalled {
                content: "x".into(),
                turns: 20,
            },
        ] {
            let msg = finding.message(root);
            assert!(msg.contains(root), "{msg}");
            for imperative in ["You must", "you should", "Update the", "Remember to"] {
                assert!(!msg.contains(imperative), "imperative in: {msg}");
            }
        }
    }

    #[test]
    fn resume_keeps_the_claim_every_other_reason_releases_it() {
        // 'resume' means the same session is coming back; dropping its claim
        // there would orphan a tree that is still being worked on.
        assert!(!should_release_claim("resume"));
        for reason in [
            "clear",
            "logout",
            "prompt_input_exit",
            "bypass_permissions_disabled",
            "other",
            "",
        ] {
            assert!(should_release_claim(reason), "reason {reason:?}");
        }
    }

    #[test]
    fn only_context_losing_sources_get_reinjection() {
        // startup has no history to have forgotten.
        assert!(!needs_context_reinjection("startup"));
        for source in ["resume", "compact", "fork"] {
            assert!(needs_context_reinjection(source), "source {source:?}");
        }
        assert!(!needs_context_reinjection("clear"));
    }

    #[test]
    fn summary_states_progress_and_where_the_work_is() {
        let s = summarize(&tree(["x", "*", " "], "*"), "root").expect("summary");
        assert!(s.contains("Do the thing"), "{s}");
        assert!(s.contains("1 of 3"), "{s}");
        assert!(s.contains("\"Second\" is marked in progress"), "{s}");
    }

    #[test]
    fn summary_reports_when_nothing_is_active() {
        let s = summarize(&tree(["x", " ", " "], "*"), "root").expect("summary");
        assert!(s.contains("no item is currently marked in progress"), "{s}");
    }

    #[test]
    fn summary_counts_cancelled_as_done() {
        let s = summarize(&tree(["x", "-", "*"], "*"), "root").expect("summary");
        assert!(s.contains("2 of 3"), "{s}");
    }

    #[test]
    fn summary_is_absent_for_an_unknown_or_childless_root() {
        assert!(summarize(&tree(["x", "x", "x"], "x"), "gone").is_none());
        assert!(summarize(&[item("root", "Lone", 0, "*")], "root").is_none());
    }

    #[test]
    fn session_id_cannot_escape_the_sessions_directory() {
        let path = session_path("../../etc/passwd").expect("sanitizes rather than fails");
        assert!(path.ends_with("etcpasswd.json"), "got {path:?}");
        assert!(session_path("").is_err());
    }
}
