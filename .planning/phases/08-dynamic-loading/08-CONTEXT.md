# Phase 8: Dynamic Loading - Context

**Gathered:** 2026-01-25
**Status:** Ready for planning

<domain>
## Phase Boundary

Load native plugins (.so/.dylib/.dll) at runtime with safety guarantees. Proxy pattern keeps libraries alive, plugin panics are caught at FFI boundary, and plugins never unload during app lifetime.

</domain>

<decisions>
## Implementation Decisions

### Loading failure feedback
- Popup notification when plugins fail to load (not silent, not status bar only)
- Collect all failures during startup, show combined popup once all plugins attempted
- Brief message with hint: "Run `totui plugin status` for details"
- Failed plugins NOT auto-disabled — error shows each launch until user fixes or manually disables

### Panic behavior
- Error notification popup when plugin panics during operation
- Plugin auto-disabled for rest of session after panic (prevents repeated crashes)
- Notification includes plugin name AND panic message (may be technical)
- Always log panics to file with backtrace when available (not just when RUST_LOG set)

### Startup experience
- TUI renders first, then status bar shows per-plugin loading progress
- Blocked briefly — wait for plugin loading before accepting input
- No summary message after successful load — status bar returns to normal

### Dependency reporting
- Version mismatch: Clear message "Plugin X requires to-tui 2.1+, you have 2.0"
- Corrupted/missing symbols: Generic message "Plugin X failed to load — may be corrupted or incompatible"
- `totui plugin status` shows full diagnostics: version requirements, file paths, actual error
- Multiple failures listed individually, not grouped by reason

### Claude's Discretion
- Exact popup UI implementation (modal, dialog widget)
- Loading progress format in status bar
- Log file location and rotation
- Error message wording beyond specified patterns

</decisions>

<specifics>
## Specific Ideas

- TUI should feel responsive — show the interface immediately, load plugins while visible
- Status bar progress lets user know the app isn't frozen if plugins take time
- Technical panic messages are acceptable — this is a dev-focused tool

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 08-dynamic-loading*
*Context gathered: 2026-01-25*
