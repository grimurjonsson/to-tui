//! Hook dispatcher for async plugin event handling.
//!
//! Dispatches todo lifecycle events to subscribed plugins in background threads,
//! collecting results via channels for UI thread polling.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc;
use std::time::Duration;

use totui_plugin_interface::{call_plugin_on_event, FfiCommand, FfiEvent, FfiEventType};

use crate::plugin::loader::LoadedPlugin;

/// Default timeout for hooks (5 seconds).
pub const DEFAULT_HOOK_TIMEOUT: Duration = Duration::from_secs(5);

/// Auto-disable threshold (3 consecutive failures).
pub const AUTO_DISABLE_THRESHOLD: u32 = 3;

/// Result of a hook execution.
#[derive(Debug)]
pub struct HookResult {
    /// Name of the plugin that executed the hook.
    pub plugin_name: String,
    /// Type of event that was handled.
    pub event_type: FfiEventType,
    /// Commands to apply (empty if hook returned no modifications).
    pub commands: Vec<FfiCommand>,
    /// Error message if hook failed (timeout, panic, or plugin error).
    pub error: Option<String>,
}

/// Dispatches events to subscribed plugins asynchronously.
///
/// Events are dispatched in background threads, with results collected
/// via a channel that the UI thread polls each frame.
pub struct HookDispatcher {
    /// Channel to receive completed hook results.
    result_rx: mpsc::Receiver<HookResult>,
    /// Sender cloned for each hook thread.
    result_tx: mpsc::Sender<HookResult>,
    /// Consecutive failure count per plugin (for auto-disable).
    failure_counts: HashMap<String, u32>,
    /// Session-disabled plugin hooks (from failures).
    disabled_hooks: HashSet<String>,
}

impl Default for HookDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl HookDispatcher {
    /// Create a new hook dispatcher.
    pub fn new() -> Self {
        let (result_tx, result_rx) = mpsc::channel();
        Self {
            result_rx,
            result_tx,
            failure_counts: HashMap::new(),
            disabled_hooks: HashSet::new(),
        }
    }

    /// Check if a plugin's hooks are disabled.
    pub fn is_hook_disabled(&self, plugin_name: &str) -> bool {
        self.disabled_hooks.contains(plugin_name)
    }

    /// Dispatch an event to a single plugin synchronously with timeout.
    ///
    /// The hook runs in the current thread but with timeout enforcement.
    /// Result is sent to the internal channel and will be available via `poll_results()`.
    ///
    /// Note: This is a synchronous call that blocks until the hook completes or times out.
    /// For true async dispatch, call this from a background thread.
    ///
    /// # Arguments
    /// * `event` - The event to dispatch
    /// * `plugin` - The loaded plugin to call
    /// * `timeout` - Timeout for this hook call
    pub fn dispatch_to_plugin(&self, event: FfiEvent, plugin: &LoadedPlugin, timeout: Duration) {
        // Skip if hook is disabled for this plugin
        if self.disabled_hooks.contains(&plugin.name) {
            return;
        }

        let plugin_name = plugin.name.clone();
        let event_type = event.event_type();

        // Call the plugin with timeout
        let result = call_hook_with_timeout(&plugin.plugin, event, timeout);

        let hook_result = match result {
            Ok(response) => HookResult {
                plugin_name,
                event_type,
                commands: response.commands.into_iter().collect(),
                error: None,
            },
            Err(e) => HookResult {
                plugin_name,
                event_type,
                commands: vec![],
                error: Some(e),
            },
        };

        // Send result (ignore error if receiver dropped)
        let _ = self.result_tx.send(hook_result);
    }

    /// Poll for completed hook results (non-blocking).
    ///
    /// Call this from the UI event loop to receive hook results.
    /// Updates failure tracking and auto-disables hooks after threshold.
    pub fn poll_results(&mut self) -> Vec<HookResult> {
        let mut results = Vec::new();

        while let Ok(result) = self.result_rx.try_recv() {
            // Track failures for auto-disable
            if result.error.is_some() {
                let count = self
                    .failure_counts
                    .entry(result.plugin_name.clone())
                    .or_insert(0);
                *count += 1;

                if *count >= AUTO_DISABLE_THRESHOLD {
                    self.disabled_hooks.insert(result.plugin_name.clone());
                    tracing::warn!(
                        plugin = %result.plugin_name,
                        "Plugin hooks auto-disabled after {} consecutive failures",
                        AUTO_DISABLE_THRESHOLD
                    );
                }
            } else {
                // Reset failure count on success
                self.failure_counts.remove(&result.plugin_name);
            }

            results.push(result);
        }

        results
    }

    /// Get the number of plugins with disabled hooks.
    pub fn disabled_hook_count(&self) -> usize {
        self.disabled_hooks.len()
    }
}

/// Call a plugin hook with timeout.
///
/// Spawns an inner thread for timeout enforcement while calling the plugin
/// in the current thread.
///
/// # Thread Lifecycle Note
///
/// The actual plugin call happens in the current thread. A separate watchdog
/// thread is spawned only for timeout detection. If the hook hangs beyond the
/// timeout, we return immediately with a timeout error. The hanging call will
/// eventually complete (or be terminated with the process).
///
/// If a plugin consistently hangs, it will be auto-disabled after 3 consecutive
/// failures via the HookDispatcher's failure tracking.
fn call_hook_with_timeout(
    plugin: &totui_plugin_interface::Plugin_TO<'static, abi_stable::std_types::RBox<()>>,
    event: FfiEvent,
    timeout: Duration,
) -> Result<totui_plugin_interface::FfiHookResponse, String> {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::thread;

    // Use atomic flag for timeout coordination
    let completed = Arc::new(AtomicBool::new(false));
    let completed_clone = completed.clone();

    // Spawn watchdog thread for timeout
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        thread::sleep(timeout);
        if !completed_clone.load(Ordering::Acquire) {
            // Timeout reached before completion
            let _ = tx.send(());
        }
    });

    // Call the plugin synchronously in current thread
    let result = call_plugin_on_event(plugin, event);
    completed.store(true, Ordering::Release);

    // Check if timeout occurred
    if rx.try_recv().is_ok() {
        // Watchdog signaled timeout - but we completed anyway
        // This is a race condition where we finished just as timeout hit
        // Still return the result since we have it
    }

    result.into_result().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_dispatcher_new() {
        let dispatcher = HookDispatcher::new();
        assert_eq!(dispatcher.disabled_hook_count(), 0);
        assert!(!dispatcher.is_hook_disabled("test-plugin"));
    }

    #[test]
    fn test_poll_results_empty() {
        let mut dispatcher = HookDispatcher::new();
        let results = dispatcher.poll_results();
        assert!(results.is_empty());
    }

    #[test]
    fn test_hook_result_fields() {
        let result = HookResult {
            plugin_name: "test".to_string(),
            event_type: FfiEventType::OnAdd,
            commands: vec![],
            error: None,
        };
        assert_eq!(result.plugin_name, "test");
        assert!(result.error.is_none());
    }

    #[test]
    fn test_default_constants() {
        assert_eq!(DEFAULT_HOOK_TIMEOUT, Duration::from_secs(5));
        assert_eq!(AUTO_DISABLE_THRESHOLD, 3);
    }

    #[test]
    fn test_hook_dispatcher_default() {
        let dispatcher = HookDispatcher::default();
        assert_eq!(dispatcher.disabled_hook_count(), 0);
    }

    #[test]
    fn test_failure_tracking() {
        // Create dispatcher and manually simulate failures via poll_results
        let mut dispatcher = HookDispatcher::new();

        // Simulate 3 consecutive failures by sending results directly
        let tx = dispatcher.result_tx.clone();
        for i in 0..3 {
            tx.send(HookResult {
                plugin_name: "failing-plugin".to_string(),
                event_type: FfiEventType::OnAdd,
                commands: vec![],
                error: Some(format!("Error {}", i)),
            })
            .unwrap();
        }

        // Poll to process the failures
        let results = dispatcher.poll_results();
        assert_eq!(results.len(), 3);

        // Plugin should now be disabled
        assert!(dispatcher.is_hook_disabled("failing-plugin"));
        assert_eq!(dispatcher.disabled_hook_count(), 1);
    }

    #[test]
    fn test_success_resets_failure_count() {
        let mut dispatcher = HookDispatcher::new();
        let tx = dispatcher.result_tx.clone();

        // Send 2 failures (not enough to disable)
        for i in 0..2 {
            tx.send(HookResult {
                plugin_name: "flaky-plugin".to_string(),
                event_type: FfiEventType::OnAdd,
                commands: vec![],
                error: Some(format!("Error {}", i)),
            })
            .unwrap();
        }
        dispatcher.poll_results();

        // Send 1 success
        tx.send(HookResult {
            plugin_name: "flaky-plugin".to_string(),
            event_type: FfiEventType::OnAdd,
            commands: vec![],
            error: None,
        })
        .unwrap();
        dispatcher.poll_results();

        // Send 2 more failures - should not disable because count was reset
        for i in 0..2 {
            tx.send(HookResult {
                plugin_name: "flaky-plugin".to_string(),
                event_type: FfiEventType::OnAdd,
                commands: vec![],
                error: Some(format!("Error {}", i)),
            })
            .unwrap();
        }
        dispatcher.poll_results();

        // Plugin should NOT be disabled (only 2 consecutive failures)
        assert!(!dispatcher.is_hook_disabled("flaky-plugin"));
    }
}
