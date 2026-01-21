---
phase: quick
plan: 001
type: execute
wave: 1
depends_on: []
files_modified: [README.md]
autonomous: true

must_haves:
  truths:
    - "Installation section shows only the curl command"
    - "MCP section is concise with only essential configuration"
    - "No update instructions exist (auto-upgrade handles this)"
    - "No plugin marketplace instructions exist (obsolete)"
  artifacts:
    - path: "README.md"
      provides: "Simplified project documentation"
      contains: "curl -fsSL"
  key_links: []
---

<objective>
Simplify README.md by removing obsolete installation alternatives and verbose MCP setup instructions.

Purpose: The program now auto-upgrades, making manual update instructions unnecessary. The plugin marketplace approach is obsolete. Users just need the curl command to install.

Output: A cleaner README.md focused on getting started quickly.
</objective>

<execution_context>
@~/.claude/get-shit-done/workflows/execute-plan.md
@~/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@README.md
</context>

<tasks>

<task type="auto">
  <name>Task 1: Simplify Installation Section</name>
  <files>README.md</files>
  <action>
Replace the entire Installation section (lines 22-51) with a minimal version:

1. Remove "### Quick Install (Recommended)" subheading - just use "## Installation"
2. Keep ONLY the curl command block
3. Keep the brief explanation of what the installer prompts for (totui, totui-mcp, or both)
4. Remove entirely:
   - "### From Source" subsection (lines 37-45)
   - "### Using Cargo" subsection (lines 47-51)

The new Installation section should be approximately 10-12 lines total.
  </action>
  <verify>grep -A 15 "## Installation" README.md | wc -l shows ~15 lines or less</verify>
  <done>Installation section contains only curl command and brief installer explanation</done>
</task>

<task type="auto">
  <name>Task 2: Simplify MCP Server Section</name>
  <files>README.md</files>
  <action>
Drastically simplify the MCP Server section (lines 119-217). The new section should contain:

1. Keep the intro line: "The MCP server allows AI assistants like Claude to manage your todos."

2. Show ONLY the direct MCP configuration approach (the `claude mcp add` commands):
```bash
# User-scoped (available in all projects)
claude mcp add --transport stdio --scope user totui-mcp -- /usr/local/bin/totui-mcp

# Project-scoped (creates .mcp.json in project)
claude mcp add --transport stdio --scope project totui-mcp -- /usr/local/bin/totui-mcp
```
Note: Change the path from `/path/to/totui-mcp` to `/usr/local/bin/totui-mcp` since that's where the installer puts it.

3. Keep the verification commands (claude mcp list, /mcp)

4. Keep a simple JSON example for other LLM tools:
```json
{
  "totui-mcp": {
    "command": "/usr/local/bin/totui-mcp",
    "args": []
  }
}
```

Remove entirely:
- "Option 1: Plugin Marketplace" section (lines 125-143)
- The "Direct MCP Configuration" heading (just make it the default)
- "Pre-built Binaries" section (lines 167-180) - installer handles this
- "Updating the Plugin" section (lines 182-187) - auto-upgrade handles this
- "Local Development Setup" section (lines 189-197) - belongs in CLAUDE.md/CONTRIBUTING
- "Manual MCP Server Setup" heading (lines 199-206) - just keep the JSON config example

Target: ~25-30 lines total for the MCP section.
  </action>
  <verify>grep -n "Plugin Marketplace\|Updating the Plugin\|Local Development" README.md returns no matches</verify>
  <done>MCP section is concise with only essential setup information, no obsolete instructions</done>
</task>

</tasks>

<verification>
- [ ] `grep -c "From Source\|Using Cargo" README.md` returns 0
- [ ] `grep -c "Plugin Marketplace\|Updating the Plugin" README.md` returns 0
- [ ] `grep "curl -fsSL" README.md` still returns the install command
- [ ] `grep "/usr/local/bin/totui-mcp" README.md` shows correct path in MCP config
- [ ] README.md is well-formed markdown (no broken sections)
</verification>

<success_criteria>
- Installation section: ~10-12 lines, only curl command
- MCP section: ~25-30 lines, direct configuration only
- No obsolete update/plugin marketplace instructions
- README reads cleanly top to bottom
</success_criteria>

<output>
After completion, create `.planning/quick/001-cleanup-readme-simplify-install/001-SUMMARY.md`
</output>
