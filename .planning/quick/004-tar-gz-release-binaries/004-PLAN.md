---
phase: quick
plan: 004
type: execute
wave: 1
depends_on: []
files_modified:
  - .github/workflows/release.yml
  - scripts/install.sh
  - scripts/install-binary.sh
  - src/utils/upgrade.rs
autonomous: true

must_haves:
  truths:
    - "Release workflow creates .tar.gz archives with binary + README"
    - "install.sh downloads and extracts .tar.gz archives"
    - "install-binary.sh downloads and extracts .tar.gz archives"
    - "TUI self-upgrade downloads and extracts .tar.gz archives"
  artifacts:
    - path: ".github/workflows/release.yml"
      provides: "tar.gz archive creation in CI"
      contains: "tar -czvf"
    - path: "scripts/install.sh"
      provides: "tar.gz download and extraction"
      contains: "tar -xzf"
    - path: "scripts/install-binary.sh"
      provides: "tar.gz download and extraction for MCP"
      contains: "tar -xzf"
    - path: "src/utils/upgrade.rs"
      provides: "tar.gz download and extraction for self-upgrade"
      contains: "flate2"
  key_links:
    - from: ".github/workflows/release.yml"
      to: "GitHub Releases"
      via: "softprops/action-gh-release"
      pattern: "totui.*\\.tar\\.gz"
---

<objective>
Change release binaries from raw executables to .tar.gz archives.

Purpose: Standardize release format to match common Rust project conventions, allow including README/LICENSE in archives, and provide consistent extraction workflow across all installation methods.

Output: Updated release workflow and all installation scripts to produce/consume .tar.gz archives.
</objective>

<execution_context>
@~/.claude/get-shit-done/workflows/execute-plan.md
@~/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.github/workflows/release.yml
@scripts/install.sh
@scripts/install-binary.sh
@src/utils/upgrade.rs
</context>

<tasks>

<task type="auto">
  <name>Task 1: Update release workflow to create .tar.gz archives</name>
  <files>.github/workflows/release.yml</files>
  <action>
    Modify the release workflow to create .tar.gz archives instead of uploading raw binaries.

    For Unix builds:
    - Create archive: `totui-{target}.tar.gz` containing:
      - `totui` (the binary)
    - Create archive: `totui-mcp-{target}.tar.gz` containing:
      - `totui-mcp` (the binary)
    - Use: `tar -czvf archive.tar.gz -C directory binary_name`

    For Windows builds:
    - Create archive: `totui-{target}.zip` containing:
      - `totui.exe`
    - Create archive: `totui-mcp-{target}.zip` containing:
      - `totui-mcp.exe`
    - Use: `zip archive.zip binary.exe`

    Keep archive naming consistent: `{binary_name}-{target}.tar.gz` (or .zip for Windows)
  </action>
  <verify>Review the workflow changes ensure tar/zip commands are correct</verify>
  <done>Release workflow creates .tar.gz archives (Unix) and .zip archives (Windows)</done>
</task>

<task type="auto">
  <name>Task 2: Update install.sh to extract .tar.gz archives</name>
  <files>scripts/install.sh</files>
  <action>
    Update the install script to download and extract .tar.gz archives.

    Changes needed:
    1. Update DOWNLOAD_URL to use `.tar.gz` extension (Unix) or `.zip` (Windows)
    2. After download, extract the archive to a temp directory:
       - Unix: `tar -xzf archive.tar.gz -C temp_dir`
       - Windows: `unzip archive.zip -d temp_dir`
    3. Move the extracted binary to INSTALL_DIR
    4. Clean up temp files and archive

    The binary inside the archive is named `totui` or `totui-mcp` (no target suffix).
  </action>
  <verify>`shellcheck scripts/install.sh` passes (if available) and manual review</verify>
  <done>install.sh downloads .tar.gz, extracts binary, and installs it correctly</done>
</task>

<task type="auto">
  <name>Task 3: Update install-binary.sh to extract .tar.gz archives</name>
  <files>scripts/install-binary.sh</files>
  <action>
    Update the MCP plugin install script to download and extract .tar.gz archives.

    Changes needed:
    1. Update DOWNLOAD_URL to use `.tar.gz` extension (Unix) or `.zip` (Windows)
    2. Download to a temp file with proper extension
    3. Extract the archive: `tar -xzf archive.tar.gz -C temp_dir`
    4. Move the extracted `totui-mcp` binary to INSTALL_DIR
    5. Clean up temp files

    Keep the same output messages and error handling.
  </action>
  <verify>Manual review of script logic</verify>
  <done>install-binary.sh downloads .tar.gz, extracts, and installs totui-mcp</done>
</task>

<task type="auto">
  <name>Task 4: Update Rust upgrade module to extract .tar.gz archives</name>
  <files>src/utils/upgrade.rs</files>
  <action>
    Update the TUI self-upgrade module to handle .tar.gz archives.

    Changes needed:
    1. Update `get_asset_download_url()` to append `.tar.gz` to the URL
    2. Update `prepare_binary()` to extract the archive:
       - Use `flate2` and `tar` crates for extraction
       - Extract to a temp directory
       - Find the `totui` binary in the extracted contents
       - Return path to the extracted binary
    3. Update `download_file_blocking()` if needed to handle different file names
    4. Update tests to expect `.tar.gz` URL pattern

    Add dependencies to Cargo.toml if not already present:
    - `flate2` for gzip decompression
    - `tar` for tar archive extraction

    The extraction logic:
    ```rust
    use flate2::read::GzDecoder;
    use tar::Archive;

    let tar_gz = std::fs::File::open(archive_path)?;
    let tar = GzDecoder::new(tar_gz);
    let mut archive = Archive::new(tar);
    archive.unpack(extract_dir)?;
    ```
  </action>
  <verify>`cargo test` passes, especially the URL format test</verify>
  <done>TUI self-upgrade downloads .tar.gz and extracts binary before replacement</done>
</task>

</tasks>

<verification>
1. Review all changed files for consistency in archive naming
2. Verify tar/extraction commands are correct for each platform
3. `cargo test` passes
4. `cargo clippy` passes
</verification>

<success_criteria>
- Release workflow creates `totui-{target}.tar.gz` and `totui-mcp-{target}.tar.gz` archives
- All install scripts download and extract .tar.gz archives correctly
- TUI self-upgrade module uses flate2/tar to extract downloaded archives
- All tests pass
- No breaking changes to existing functionality
</success_criteria>

<output>
After completion, create `.planning/quick/004-tar-gz-release-binaries/004-SUMMARY.md`
</output>
