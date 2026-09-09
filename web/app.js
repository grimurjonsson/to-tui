"use strict";
const $ = (id) => document.getElementById(id);
const state = {
  userId: null,
  project:
    new URLSearchParams(location.search).get("project") ||
    document.querySelector('meta[name="totui-project"]').content,
  date: "",
  snapshot: null,
  projects: [],
  selected: null,
  drafts: new Map(),
  collapsed: new Set(),
  rows: new Map(),
  generation: 0,
  timer: null,
  saving: false,
  editor: null,
};
const stateIcons = {
  " ": "⬜",
  "*": "🔄",
  x: "✅",
  "?": "❔",
  "!": "❗",
  "-": "🚫",
};
const complete = (item) => ["x", "-"].includes(item.state);
const scope = () => `${state.project}/${state.snapshot?.date || state.date}`;
const key = (id) => `${scope()}/${id}`;
const query = () =>
  new URLSearchParams({
    project: state.project,
    ...(state.date ? { date: state.date } : {}),
  });
function text(node, value) {
  if (node.textContent !== value) node.textContent = value;
}
function message(id, value) {
  text($(id), value);
  $(id).hidden = !value;
}
async function api(path, options = {}) {
  const response = await fetch(path, {
    cache: "no-store",
    ...options,
    headers: {
      "Content-Type": "application/json",
      ...(state.userId ? { "X-Totui-Expected-User": state.userId } : {}),
      ...options.headers,
    },
  });
  const userId = response.headers.get("X-Totui-User");
  if (
    response.status === 401 ||
    (userId && state.userId && userId !== state.userId)
  ) {
    state.generation++;
    document.body.replaceChildren();
    location.reload();
    return new Promise(() => {});
  }
  if (userId) state.userId = userId;
  if (!response.ok) {
    const data = await response.json().catch(() => ({}));
    const error = new Error(
      data.error || `Request failed (${response.status})`,
    );
    error.status = response.status;
    throw error;
  }
  return response.status === 204 ? null : response.json();
}
function schedule() {
  clearTimeout(state.timer);
  state.timer = setTimeout(refresh, 25);
}
async function refresh(force = false) {
  if ((state.saving || state.drag) && force !== true) {
    state.refreshPending = true;
    return false;
  }
  const generation = ++state.generation;
  const currentScope = query().toString();
  try {
    const [snapshotResult, projectsResult] = await Promise.allSettled([
      api(`api/snapshot?${currentScope}`),
      api("api/projects"),
    ]);
    if (generation !== state.generation || currentScope !== query().toString())
      return;
    if (projectsResult.status === "fulfilled") {
      const previousProjects = state.projects;
      state.projects = projectsResult.value.projects;
      for (const old of previousProjects) {
        const renamed = state.projects.find((project) => project.id === old.id);
        if (renamed && renamed.name !== old.name)
          renameProjectDrafts(old.name, renamed.name);
      }
      if (!state.projects.some((project) => project.name === state.project)) {
        const old = previousProjects.find(
          (project) => project.name === state.project,
        );
        const renamed = state.projects.find(
          (project) => project.id === old?.id,
        );
        switchProject(renamed?.name || "default");
        return;
      }
      renderProjects();
    }
    if (snapshotResult.status === "rejected") throw snapshotResult.reason;
    const snapshot = snapshotResult.value;
    const previous = state.snapshot;
    state.snapshot = snapshot;
    renderList(previous);
    if (
      state.editor &&
      !state.editor.dirty &&
      !state.editor.moveDirty &&
      !state.saving &&
      state.editor.scope === scope()
    )
      loadEditor(state.selected);
    if (
      state.editor &&
      state.editor.id !== "new" &&
      !snapshot.items.some((i) => i.id === state.editor.id)
    ) {
      $("task-actions").hidden = true;
      message(
        "draft-status",
        "This task was deleted elsewhere. Your draft is preserved.",
      );
    }
    if (state.editor && state.editor.scope !== scope())
      message(
        "draft-status",
        "This draft belongs to an earlier day. It is preserved, but cannot be saved to Today.",
      );
    message(
      "error",
      projectsResult.status === "rejected"
        ? "Projects could not be refreshed. Use Refresh to try again."
        : state.mutationError || "",
    );
    return true;
  } catch (error) {
    if (generation === state.generation)
      message(
        "error",
        `${error.message}. Use Refresh to try again. Your draft is preserved.`,
      );
  }
}
function renderProjects() {
  $("kanban-link").href = `/kanban?project=${encodeURIComponent(state.project)}`;
  const focusedProject = document.activeElement?.dataset.project;
  const signature = JSON.stringify(state.projects);
  if ($("projects").dataset.signature !== signature) {
    $("projects").replaceChildren();
    $("project-picker").replaceChildren();
    for (const project of state.projects) {
      const button = document.createElement("button");
      button.textContent = project.name;
      button.dataset.project = project.name;
      button.onclick = () => switchProject(project.name);
      $("projects").append(button);
      $("project-picker").add(new Option(project.name, project.name));
    }
    $("projects").dataset.signature = signature;
    if (focusedProject)
      [...$("projects").children]
        .find((button) => button.dataset.project === focusedProject)
        ?.focus({ preventScroll: true });
  }
  for (const button of $("projects").children) {
    button.classList.toggle(
      "selected",
      button.dataset.project === state.project,
    );
    button.setAttribute(
      "aria-current",
      button.dataset.project === state.project ? "page" : "false",
    );
  }
  $("project-picker").value = state.project;
}
function renameProjectDrafts(oldName, newName) {
  for (const [draftKey, draft] of [...state.drafts]) {
    if (draft.project !== oldName) continue;
    state.drafts.delete(draftKey);
    draft.project = newName;
    draft.scope = `${newName}/${draft.date}`;
    state.drafts.set(`${newName}${draftKey.slice(oldName.length)}`, draft);
  }
}
let projectEdit = null;
function projectControls() {
  const project = state.projects.find(
    (project) => project.id === $("manage-project-picker").value,
  );
  const protectedProject = !project || project.name === "default";
  $("rename-project").disabled = protectedProject;
  $("delete-project").disabled = protectedProject;
  $("project-protection").hidden = !protectedProject;
}
function showProjectManagement() {
  projectEdit = null;
  $("project-management").hidden = false;
  $("project-form").hidden = true;
  text($("project-dialog-title"), "Manage projects");
  projectControls();
}
$("manage-projects").onclick = () => {
  $("account-menu").hidePopover();
  $("manage-project-picker").replaceChildren();
  for (const project of state.projects) {
    $("manage-project-picker").add(
      new Option(
        project.name,
        project.id,
        false,
        project.name === state.project,
      ),
    );
  }
  showProjectManagement();
  $("project-dialog").showModal();
};
$("close-projects").onclick = () => $("project-dialog").close();
$("manage-project-picker").onchange = () => {
  $("project-form").hidden = true;
  projectControls();
};
function editProject(mode) {
  const project = state.projects.find(
    (project) => project.id === $("manage-project-picker").value,
  );
  projectEdit = { mode, project: mode === "create" ? null : project };
  $("project-management").hidden = true;
  text(
    $("project-dialog-title"),
    mode === "create"
      ? "New project"
      : mode === "rename"
        ? `Rename “${project.name}”`
        : `Delete “${project.name}”?`,
  );
  $("project-form").hidden = false;
  $("project-name-label").hidden = mode === "delete";
  $("project-name").disabled = mode === "delete";
  $("project-name").value = mode === "rename" ? project.name : "";
  message("project-error", "");
  message(
    "project-delete-message",
    mode === "delete"
      ? `Permanently delete “${project.name}” and all its tasks, history, and saved files? This cannot be undone.`
      : "",
  );
  text(
    $("save-project"),
    mode === "delete"
      ? "Delete project"
      : mode === "rename"
        ? "Save name"
        : "Create project",
  );
  $("save-project").className = mode === "delete" ? "danger" : "primary";
  if (mode === "delete") $("cancel-project-edit").focus();
  else $("project-name").focus();
}
$("project-name").oninput = () => message("project-error", "");
$("new-project").onclick = () => editProject("create");
$("rename-project").onclick = () => editProject("rename");
$("delete-project").onclick = () => editProject("delete");
$("cancel-project-edit").onclick = () => {
  showProjectManagement();
  $("new-project").focus();
};
$("project-dialog").addEventListener("cancel", (event) => {
  if (state.saving) event.preventDefault();
});
$("project-form").onsubmit = async (event) => {
  event.preventDefault();
  if (state.saving) return;
  const { mode, project } = projectEdit;
  state.saving = true;
  const controls = [
    ...$("project-dialog").querySelectorAll("button,input,select"),
  ];
  const disabled = controls.map((control) => control.disabled);
  controls.forEach((control) => (control.disabled = true));
  message("project-error", "");
  try {
    const result = await api(
      mode === "create" ? "api/projects" : `api/projects/${project.id}`,
      {
        method:
          mode === "create" ? "POST" : mode === "rename" ? "PATCH" : "DELETE",
        ...(mode !== "delete"
          ? { body: JSON.stringify({ name: $("project-name").value }) }
          : {}),
      },
    );
    if (mode === "rename") renameProjectDrafts(project.name, result.name);
    if (mode === "delete") {
      for (const [draftKey, draft] of state.drafts) {
        if (draft.project === project.name) state.drafts.delete(draftKey);
      }
    }
    $("project-dialog").close();
    state.saving = false;
    if (
      mode === "create" ||
      (mode === "rename" && state.project === project.name)
    )
      switchProject(result.name);
    else if (mode === "delete" && state.project === project.name)
      switchProject("default");
    else await refresh();
  } catch (error) {
    message("project-error", error.message);
  } finally {
    state.saving = false;
    controls.forEach((control, index) => (control.disabled = disabled[index]));
    schedule();
  }
};
function switchProject(project) {
  state.project = project;
  const url = new URL(location.href);
  url.searchParams.set("project", project);
  history.replaceState(null, "", url);
  changeScope();
}
function changeScope() {
  state.mutationError = "";
  closeStateMenu(false);
  cancelDrag();
  state.generation++;
  state.selected = null;
  state.editor = null;
  state.snapshot = null;
  $("add").disabled = true;
  $("empty-add").disabled = true;
  state.rows.clear();
  $("list").replaceChildren();
  closeEditor();
  message("notice", "Loading tasks…");
  refresh();
}
function visibleItems(items) {
  const included = new Set();
  const byId = new Map(items.map((i) => [i.id, i]));
  for (const item of items)
    if (!$("hide-completed").checked || !complete(item)) {
      let current = item;
      while (current && !included.has(current.id)) {
        included.add(current.id);
        current = byId.get(current.parent_id);
      }
    }
  return items.filter((item) => {
    if (!included.has(item.id)) return false;
    let parent = byId.get(item.parent_id);
    const visited = new Set();
    while (parent && !visited.has(parent.id)) {
      if (state.collapsed.has(key(parent.id))) return false;
      visited.add(parent.id);
      parent = byId.get(parent.parent_id);
    }
    return true;
  });
}
function renderList(previous) {
  const snapshot = state.snapshot;
  if (!snapshot) return;
  text($("heading"), state.date ? snapshot.date : "Today");
  text($("project-label"), state.project.toUpperCase());
  const done = snapshot.items.filter(complete).length;
  text(
    $("summary"),
    `${snapshot.items.length - done} open · ${done} completed · ${snapshot.date}`,
  );
  $("date").max = snapshot.today;
  $("date").value = state.date;
  $("today").classList.toggle("selected", !state.date);
  $("add").disabled = snapshot.read_only;
  $("empty-add").hidden = snapshot.read_only;
  $("empty-add").disabled = snapshot.read_only;
  message("notice", snapshot.read_only ? "History · read-only snapshot" : "");
  const visible = visibleItems(snapshot.items);
  const ids = new Set(visible.map((i) => i.id));
  const old = new Map(
    (previous?.items || []).map((i) => [i.id, JSON.stringify(i)]),
  );
  const hasChildren = new Set(
    snapshot.items.map((i) => i.parent_id).filter(Boolean),
  );
  $("expand-all").disabled = !hasChildren.size;
  $("collapse-all").disabled = !hasChildren.size;
  const active = document.activeElement;
  const scroll = document.querySelector("main").scrollTop;
  const pageScroll = window.scrollY;
  const mobile = matchMedia("(max-width:850px)").matches;
  const boundary = mobile
    ? 0
    : document.querySelector("main").getBoundingClientRect().top;
  const anchor =
    (mobile ? pageScroll : scroll) > 0
      ? [...$("list").children].find(
          (row) => row.getBoundingClientRect().bottom > boundary,
        )
      : null;
  const anchorTop = anchor?.getBoundingClientRect().top;
  for (const [id, row] of state.rows)
    if (!ids.has(id)) {
      row.remove();
      state.rows.delete(id);
    }
  let cursor = $("list").firstElementChild;
  for (const item of visible) {
    let row = state.rows.get(item.id);
    if (!row) {
      row = document.createElement("div");
      row.className = "task-row";
      row.dataset.id = item.id;
      row.oncontextmenu = (event) => openStateMenu(event, item.id);
      row.onkeydown = (event) => {
        if (
          event.key === "ContextMenu" ||
          (event.shiftKey && event.key === "F10")
        )
          openStateMenu(event, item.id);
      };
      const branch = document.createElement("button");
      branch.className = "branch";
      branch.innerHTML =
        '<svg viewBox="0 0 32 32" width="24" height="24" aria-hidden="true" focusable="false"><rect x="1" y="1" width="30" height="30" rx="7" fill="currentColor" opacity="0.1"/><path d="M13 9 L20 16 L13 23" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"/></svg>';
      branch.onclick = () => {
        const k = key(item.id);
        state.collapsed.has(k)
          ? state.collapsed.delete(k)
          : state.collapsed.add(k);
        renderList(state.snapshot);
      };
      const symbol = document.createElement("button");
      symbol.className = "symbol";
      symbol.setAttribute("role", "checkbox");
      symbol.setAttribute("aria-haspopup", "menu");
      symbol.onclick = () => toggleCompletion(item.id);
      const open = document.createElement("button");
      open.className = "task-open";
      open.innerHTML =
        '<span class="priority-badge" hidden></span><span class="task-title"></span><span class="meta" hidden></span>';
      open.onclick = () => select(item.id);
      const handle = document.createElement("button");
      handle.className = "drag-handle";
      handle.innerHTML =
        '<svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true"><path d="M9 5h0M15 5h0M9 12h0M15 12h0M9 19h0M15 19h0" stroke="currentColor" stroke-width="3" stroke-linecap="round"/></svg>';
      handle.onpointerdown = (event) => startDrag(event, item.id);
      handle.onpointermove = trackDrag;
      handle.onpointerup = finishDrag;
      handle.onpointercancel = cancelDrag;
      handle.onlostpointercapture = cancelDrag;
      handle.onclick = (event) => {
        if (event.detail === 0) select(item.id);
      };
      row.append(branch, symbol, open, handle);
      state.rows.set(item.id, row);
    }
    const handle = row.querySelector(".drag-handle");
    handle.hidden = snapshot.read_only;
    handle.setAttribute("aria-label", `Move ${item.content}`);
    handle.title = "Drag to move; use Enter for move controls";
    const branch = row.children[0];
    branch.disabled = !hasChildren.has(item.id);
    branch.setAttribute(
      "aria-label",
      `${state.collapsed.has(key(item.id)) ? "Expand" : "Collapse"} ${item.content}`,
    );
    if (hasChildren.has(item.id))
      branch.setAttribute(
        "aria-expanded",
        String(!state.collapsed.has(key(item.id))),
      );
    else branch.removeAttribute("aria-expanded");
    row.style.setProperty("--depth", item.indent_level);
    row.classList.toggle("active", state.selected === item.id);
    row.classList.toggle("complete", complete(item));
    row.classList.toggle(
      "context",
      $("hide-completed").checked && complete(item),
    );
    const checkbox = row.children[1];
    text(checkbox, stateIcons[item.state]);
    checkbox.disabled = snapshot.read_only || state.saving;
    checkbox.setAttribute("aria-checked", String(item.state === "x"));
    checkbox.setAttribute("aria-label", `Complete ${item.content}`);
    checkbox.title = `${item.state_description.replace("_", " ")} · ${item.state === "x" ? "Mark pending" : "Mark done"}`;
    text(row.querySelector(".task-title"), item.content);
    row
      .querySelector(".task-open")
      .setAttribute(
        "aria-label",
        `${item.content} ${item.state_description.replace("_", " ")}`,
      );
    row
      .querySelector(".task-open")
      .setAttribute(
        "aria-description",
        [
          item.priority && `Priority ${item.priority}`,
          item.due_date && `Due ${item.due_date}`,
        ]
          .filter(Boolean)
          .join(" · "),
      );
    const priority = row.querySelector(".priority-badge");
    priority.hidden = !item.priority;
    priority.dataset.priority = item.priority || "";
    text(priority, item.priority || "");
    text(
      row.querySelector(".meta"),
      [
        item.due_date && `Due ${item.due_date}`,
        item.indent_level > 5 && `Depth ${item.indent_level}`,
      ]
        .filter(Boolean)
        .join(" · "),
    );
    row.querySelector(".meta").hidden = !row.querySelector(".meta").textContent;
    if (
      previous?.date === snapshot.date &&
      old.get(item.id) !== JSON.stringify(item)
    ) {
      row.classList.remove("changed");
      requestAnimationFrame(() => row.classList.add("changed"));
    }
    if (row !== cursor) $("list").insertBefore(row, cursor);
    cursor = row.nextElementSibling;
  }
  $("empty").hidden = visible.length > 0;
  text(
    $("empty").querySelector("h2"),
    snapshot.items.length
      ? "Nothing left in this view."
      : "A little room to think.",
  );
  text(
    $("empty").querySelector("p"),
    snapshot.items.length
      ? "All tasks are completed. Turn off Hide completed to see them."
      : snapshot.read_only
        ? "There are no tasks recorded for this date."
        : "No tasks here yet. Add something you want to move forward.",
  );
  if (active?.isConnected && active !== document.activeElement)
    active.focus({ preventScroll: true });
  document.querySelector("main").scrollTop = scroll;
  window.scrollTo({ top: pageScroll, behavior: "instant" });
  if (anchor?.isConnected) {
    const delta = anchor.getBoundingClientRect().top - anchorTop;
    if (mobile)
      window.scrollTo({ top: pageScroll + delta, behavior: "instant" });
    else document.querySelector("main").scrollTop = scroll + delta;
  }
}
function select(id) {
  state.selected = id;
  loadEditor(id);
  renderList(state.snapshot);
  $("details").classList.add("open");
  syncModal();
  if (matchMedia("(max-width:850px)").matches) $("close-editor").focus();
}
function loadEditor(id) {
  if (!id || !state.snapshot) return;
  let draft = state.drafts.get(key(id));
  const item = state.snapshot.items.find((i) => i.id === id);
  if (!draft) {
    if (!item && id !== "new") return;
    draft = {
      id,
      scope: scope(),
      project: state.project,
      date: state.snapshot.date,
      revision: state.snapshot.revision,
      dirty: false,
      values: { ...item },
      parent: null,
    };
    state.drafts.set(key(id), draft);
  } else if (!draft.dirty && !draft.moveDirty && item) {
    draft.values = { ...item };
    draft.revision = state.snapshot.revision;
  }
  state.editor = draft;
  $("details").hidden = false;
  $("editor").hidden = false;
  for (const name of [
    "content",
    "description",
    "state",
    "priority",
    "due_date",
  ])
    $("task-form").elements[name].value =
      draft.values[name] ?? (name === "state" ? " " : "");
  const readonly = state.snapshot.read_only || (!item && id !== "new");
  for (const element of $("task-form").elements) element.disabled = readonly;
  $("task-actions").hidden = id === "new" || readonly;
  $("delete-actions").hidden = id === "new" || readonly;
  message(
    "draft-status",
    readonly
      ? "Read-only · this task is historical or has been deleted."
      : draft.dirty || draft.moveDirty
        ? "Unsaved draft · kept while you browse"
        : "",
  );
  $("conflict").hidden = true;
  text($("save-status"), "");
  if (item) renderMove(item);
}
function renderMove(item) {
  const items = state.snapshot.items;
  const index = items.findIndex((i) => i.id === item.id);
  const excluded = new Set([item.id]);
  for (
    let i = index + 1;
    i < items.length && items[i].indent_level > item.indent_level;
    i++
  )
    excluded.add(items[i].id);
  $("parent").replaceChildren(new Option("Root level", ""));
  for (const candidate of items)
    if (!excluded.has(candidate.id))
      $("parent").add(new Option(candidate.content, candidate.id));
  $("parent").value = state.editor.moveParent ?? item.parent_id ?? "";
  $("move-root").disabled = !item.parent_id;
  state.moveExcluded = excluded;
  renderPositions();
  if (state.editor.moveBefore) $("before").value = state.editor.moveBefore;
}
function renderPositions() {
  $("before").replaceChildren(new Option("At the end", ""));
  for (const item of state.snapshot.items)
    if (
      !state.moveExcluded.has(item.id) &&
      (item.parent_id || "") === $("parent").value
    )
      $("before").add(new Option(`Before: ${item.content}`, item.id));
}
function newTask(parent = null) {
  if (!state.snapshot || state.snapshot.read_only) return;
  const existing = state.drafts.get(key("new"));
  if (!existing)
    state.drafts.set(key("new"), {
      id: "new",
      scope: scope(),
      project: state.project,
      date: state.snapshot.date,
      revision: state.snapshot.revision,
      dirty: true,
      values: {},
      parent,
    });
  select("new");
  $("content").focus();
}
function closeEditor() {
  $("details").classList.remove("open");
  syncModal();
  $("editor").hidden = true;
  $("details").hidden = true;
  state.editor = null;
}
function captureDraft() {
  if (!state.editor) return;
  state.editor.dirty = true;
  state.editor.values = Object.fromEntries(new FormData($("task-form")));
  message("draft-status", "Unsaved draft · kept while you browse");
}
async function mutate(path, method, body, draft = state.editor) {
  if (state.saving) return false;
  state.saving = true;
  state.mutationError = "";
  const focused = document.activeElement;
  const controls = [
    ...document.querySelectorAll("button,input,select,textarea"),
  ];
  const disabled = controls.map((control) => control.disabled);
  controls.forEach((control) => {
    control.disabled = true;
  });
  text($("save-status"), "Saving…");
  try {
    await api(path, {
      method,
      headers: { "X-Totui-Today": draft?.date || state.snapshot.date },
      ...(body ? { body: JSON.stringify(body) } : {}),
    });
    const refreshed = await refresh(true);
    text($("save-status"), refreshed ? "Saved" : "Saved; refresh needed");
    message("editor-error", "");
    return true;
  } catch (error) {
    if ([404, 409].includes(error.status) && draft) {
      await refresh(true);
      $("conflict").hidden = false;
      const latest = state.snapshot?.items.find((i) => i.id === draft.id);
      $("recreate").hidden = !!latest;
      $("rebase").hidden = !latest;
      text(
        $("latest"),
        latest
          ? `Latest task: ${latest.content}\n${latest.description || "No description"}\n${latest.state_description} · ${latest.priority || "No priority"} · ${latest.due_date || "No due date"}`
          : "The task may have been deleted or moved to another day. Your draft is still here.",
      );
    }
    state.mutationError = error.message;
    text($("save-status"), "Not saved");
    message("error", error.message);
    message("editor-error", error.message);
    return false;
  } finally {
    state.saving = false;
    if (state.refreshPending) {
      state.refreshPending = false;
      schedule();
    }
    controls.forEach((control, index) => {
      control.disabled = disabled[index];
    });
    if (
      document.activeElement === document.body &&
      focused?.isConnected &&
      !focused.disabled
    )
      focused.focus({ preventScroll: true });
  }
}
$("task-form").addEventListener("input", captureDraft);
$("task-form").onsubmit = async (event) => {
  event.preventDefault();
  const draft = state.editor;
  if (!draft || draft.scope !== scope() || state.snapshot.read_only) return;
  captureDraft();
  const values = draft.values;
  const body = {
    content: values.content,
    description: values.description,
    state: values.state,
    ...(values.priority ? { priority: values.priority } : {}),
    ...(values.due_date ? { due_date: values.due_date } : {}),
  };
  const creating = draft.id === "new";
  if (creating) body.parent_id = draft.parent;
  else
    Object.assign(body, {
      expected_revision: draft.revision,
      clear_due_date: !values.due_date,
      clear_priority: !values.priority,
      ...(draft.moveDirty
        ? {
            placement: {
              parent_id: $("parent").value || null,
              before_id: $("before").value || null,
            },
          }
        : {}),
    });
  const target = new URLSearchParams({
    project: draft.project,
    date: draft.date,
  });
  if (
    await mutate(
      `api/todos${creating ? "" : "/" + draft.id}?${target}`,
      creating ? "POST" : "PATCH",
      body,
      draft,
    )
  ) {
    state.drafts.delete(`${draft.scope}/${draft.id}`);
    state.editor = null;
    if (creating) {
      state.selected = null;
      closeEditor();
    }
    if (!creating) {
      if (body.placement) revealParent(body.placement.parent_id);
      loadEditor(draft.id);
    }
  }
};
$("rebase").onclick = () => {
  if (state.editor && state.editor.scope === scope()) {
    state.editor.revision = state.snapshot.revision;
    $("conflict").hidden = true;
    text($("save-status"), "Review your draft, then Save");
  }
};
$("discard").onclick = () => {
  if (state.editor) {
    state.drafts.delete(key(state.editor.id));
    loadEditor(state.editor.id);
  }
};
$("clear-date").onclick = () => {
  $("due-date").value = "";
  captureDraft();
};
$("add").onclick = () => newTask();
$("empty-add").onclick = () => newTask();
$("add-child").onclick = () => newTask(state.selected);
$("parent").onchange = () => {
  state.editor.moveDirty = true;
  state.editor.moveParent = $("parent").value;
  state.editor.moveBefore = "";
  message("draft-status", "Unsaved placement · Save task to apply all changes");
  renderPositions();
};
$("before").onchange = () => {
  state.editor.moveDirty = true;
  state.editor.moveBefore = $("before").value;
  message("draft-status", "Unsaved placement · Save task to apply all changes");
};
$("move-root").onclick = () => moveTask(null, null);
$("move").onclick = () =>
  moveTask($("parent").value || null, $("before").value || null);
async function moveTask(parent, before) {
  const draft = state.editor;
  if (!draft || draft.dirty) {
    message("draft-status", "Save your draft before moving this branch.");
    return;
  }
  if (
    await mutate(`api/todos/${draft.id}/move?${query()}`, "POST", {
      parent_id: parent,
      before_id: before,
      expected_revision: draft.revision,
    })
  ) {
    state.drafts.delete(key(draft.id));
    revealParent(parent);
    loadEditor(draft.id);
  }
}
function revealParent(parent) {
  let ancestor = parent;
  while (ancestor) {
    state.collapsed.delete(key(ancestor));
    ancestor = state.snapshot.items.find(
      (item) => item.id === ancestor,
    )?.parent_id;
  }
  renderList(state.snapshot);
}
$("delete").onclick = () => {
  const index = state.snapshot.items.findIndex((i) => i.id === state.selected);
  let count = 1;
  while (
    index + count < state.snapshot.items.length &&
    state.snapshot.items[index + count].indent_level >
      state.snapshot.items[index].indent_level
  )
    count++;
  text(
    $("delete-message"),
    `This deletes “${state.snapshot.items[index].content}” and ${count - 1} descendant task(s).`,
  );
  $("delete-dialog").showModal();
};
$("cancel-delete").onclick = () => $("delete-dialog").close();
$("confirm-delete").onclick = async () => {
  $("delete-dialog").close();
  const draft = state.editor;
  if (
    await mutate(
      `api/todos/${draft.id}?${query()}&revision=${draft.revision}`,
      "DELETE",
    )
  ) {
    state.drafts.delete(key(draft.id));
    state.selected = null;
    closeEditor();
    await refresh();
  }
};
$("close-editor").onclick = () => {
  closeEditor();
  state.rows
    .get(state.selected)
    ?.querySelector(".task-open")
    .focus({ preventScroll: true });
};
$("project-picker").onchange = (event) => switchProject(event.target.value);
$("date").onchange = (event) => {
  state.date = event.target.value;
  changeScope();
};
$("today").onclick = () => {
  state.date = "";
  changeScope();
};
$("expand-all").onclick = () => {
  for (const item of state.snapshot.items) state.collapsed.delete(key(item.id));
  renderList(state.snapshot);
};
$("collapse-all").onclick = () => {
  for (const item of state.snapshot.items) state.collapsed.add(key(item.id));
  renderList(state.snapshot);
};
$("hide-completed").onchange = () => renderList(state.snapshot);
$("refresh").onclick = refresh;
const savedTheme = localStorage.getItem("totui-theme");
document.documentElement.dataset.theme =
  savedTheme ||
  (matchMedia("(prefers-color-scheme:dark)").matches ? "dark" : "light");
$("theme").onclick = () => {
  const theme =
    document.documentElement.dataset.theme === "dark" ? "light" : "dark";
  document.documentElement.dataset.theme = theme;
  localStorage.setItem("totui-theme", theme);
};
const events = new EventSource("api/events");
events.onopen = () => {
  text($("connection"), "● Live");
  $("connection").className = "connected";
  schedule();
};
events.addEventListener("change", schedule);
events.onerror = () => {
  text($("connection"), "Reconnecting…");
  $("connection").className = "";
};
document.addEventListener("visibilitychange", () => {
  if (!document.hidden) refresh();
});
window.addEventListener("pageshow", refresh);
window.addEventListener("online", refresh);
window.addEventListener("focus", refresh);
setInterval(() => {
  if (!document.hidden) refresh();
}, 15000);
window.addEventListener("beforeunload", (event) => {
  if ([...state.drafts.values()].some((d) => d.dirty || d.moveDirty)) {
    event.preventDefault();
    event.returnValue = "";
  }
});
refresh();

function syncModal() {
  const modal =
    matchMedia("(max-width:850px)").matches &&
    $("details").classList.contains("open");
  document.querySelector("main").inert = modal;
  document.querySelector(".topbar").inert = modal;
  document.querySelector(".sidebar").inert = modal;
  $("details").setAttribute("role", modal ? "dialog" : "complementary");
  if (modal) $("details").setAttribute("aria-modal", "true");
  else $("details").removeAttribute("aria-modal");
}
matchMedia("(max-width:850px)").addEventListener("change", syncModal);
$("details").addEventListener("keydown", (event) => {
  if (event.key === "Escape") {
    $("close-editor").click();
    return;
  }
  if (event.key !== "Tab" || $("details").getAttribute("aria-modal") !== "true")
    return;
  const controls = [
    ...$("details").querySelectorAll("button,input,select,textarea"),
  ].filter((el) => !el.disabled && el.getClientRects().length);
  const first = controls[0],
    last = controls.at(-1);
  if (event.shiftKey && document.activeElement === first) {
    event.preventDefault();
    last.focus();
  }
  if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault();
    first.focus();
  }
});

$("recreate").onclick = () => {
  const old = state.editor;
  if (!old) return;
  state.drafts.delete(`${old.scope}/${old.id}`);
  const draft = {
    ...old,
    id: "new",
    scope: scope(),
    date: state.snapshot.date,
    parent: null,
    dirty: true,
  };
  state.drafts.set(key("new"), draft);
  select("new");
};

window.addEventListener("offline", () => {
  text($("connection"), "Reconnecting…");
  $("connection").className = "";
});

function startDrag(event, id) {
  if (event.button !== 0 || state.saving || state.snapshot.read_only) return;
  state.generation++;
  state.refreshPending = true;
  const items = state.snapshot.items;
  const index = items.findIndex((item) => item.id === id);
  const excluded = new Set([id]);
  for (
    let i = index + 1;
    i < items.length && items[i].indent_level > items[index].indent_level;
    i++
  )
    excluded.add(items[i].id);
  state.drag = {
    id,
    excluded,
    revision: state.snapshot.revision,
    query: query().toString(),
    handle: event.currentTarget,
    pointer: event.pointerId,
    startX: event.clientX,
    startY: event.clientY,
    x: event.clientX,
    y: event.clientY,
    active: false,
  };
  event.currentTarget.setPointerCapture(event.pointerId);
}
function trackDrag(event) {
  const drag = state.drag;
  if (!drag || drag.pointer !== event.pointerId) return;
  drag.x = event.clientX;
  drag.y = event.clientY;
  if (
    !drag.active &&
    Math.hypot(drag.x - drag.startX, drag.y - drag.startY) >= 6
  ) {
    drag.active = true;
    document.body.classList.add("dragging");
    state.rows.get(drag.id)?.classList.add("drag-source");
    $("drag-feedback").hidden = false;
    $("root-drop").hidden = false;
    drag.frame = requestAnimationFrame(dragFrame);
  }
  if (drag.active) {
    event.preventDefault();
    updateDrop();
  }
}
function updateDrop() {
  const drag = state.drag;
  if (!drag?.active) return;
  document
    .querySelectorAll(".drop-before,.drop-after,.drop-inside")
    .forEach((row) =>
      row.classList.remove("drop-before", "drop-after", "drop-inside"),
    );
  $("root-drop").classList.remove("drop-inside");
  drag.target = null;
  const under = document.elementFromPoint(drag.x, drag.y);
  if (under?.closest("#root-drop")) {
    drag.target = { parent_id: null, before_id: null };
    $("root-drop").classList.add("drop-inside");
    text($("drag-feedback"), "Move to root · at the end");
  } else {
    const row = under?.closest(".task-row");
    const target = state.snapshot.items.find(
      (item) => item.id === row?.dataset.id,
    );
    if (target && !drag.excluded.has(target.id)) {
      const rect = row.getBoundingClientRect();
      const fraction = (drag.y - rect.top) / rect.height;
      const placement =
        fraction < 0.3 ? "before" : fraction > 0.7 ? "after" : "inside";
      row.classList.add(`drop-${placement}`);
      let before = null;
      if (placement === "before") before = target.id;
      if (placement === "after") {
        const items = state.snapshot.items;
        const index = items.findIndex((item) => item.id === target.id);
        before =
          items
            .slice(index + 1)
            .find(
              (item) =>
                item.parent_id === target.parent_id &&
                !drag.excluded.has(item.id),
            )?.id || null;
      }
      drag.target = {
        parent_id:
          placement === "inside" ? target.id : target.parent_id || null,
        before_id: before,
      };
      text(
        $("drag-feedback"),
        `${placement === "inside" ? "Nest under" : placement === "before" ? "Place before" : "Place after"}: ${target.content}`,
      );
    } else
      text(
        $("drag-feedback"),
        "Drag to a task edge to reorder, or its middle to nest",
      );
  }
  $("drag-feedback").style.left =
    `${Math.max(8, Math.min(drag.x - 100, innerWidth - 288))}px`;
  $("drag-feedback").style.top =
    `${Math.max(8, Math.min(drag.y - 64, innerHeight - 64))}px`;
}
function dragFrame() {
  const drag = state.drag;
  if (!drag?.active) return;
  const mobile = matchMedia("(max-width:850px)").matches;
  const main = document.querySelector("main");
  const rect = mobile
    ? { top: 0, bottom: innerHeight }
    : main.getBoundingClientRect();
  const speed =
    drag.y < rect.top + 70 ? -10 : drag.y > rect.bottom - 90 ? 10 : 0;
  if (speed) {
    if (mobile) window.scrollBy(0, speed);
    else main.scrollTop += speed;
    updateDrop();
  }
  drag.frame = requestAnimationFrame(dragFrame);
}
function clearDrag() {
  const drag = state.drag;
  if (!drag) return;
  state.drag = null;
  cancelAnimationFrame(drag.frame);
  if (drag.handle.hasPointerCapture(drag.pointer))
    drag.handle.releasePointerCapture(drag.pointer);
  document.body.classList.remove("dragging");
  document
    .querySelectorAll(".drag-source,.drop-before,.drop-after,.drop-inside")
    .forEach((row) =>
      row.classList.remove(
        "drag-source",
        "drop-before",
        "drop-after",
        "drop-inside",
      ),
    );
  $("drag-feedback").hidden = true;
  $("root-drop").hidden = true;
  return drag;
}
function cancelDrag() {
  if (clearDrag() && state.refreshPending) {
    state.refreshPending = false;
    schedule();
  }
}
async function finishDrag(event) {
  if (state.drag?.pointer !== event.pointerId) return;
  updateDrop();
  const drag = clearDrag();
  if (!drag.active || !drag.target) {
    if (!drag.active) select(drag.id);
    if (state.refreshPending) schedule();
    return;
  }
  const saved = await mutate(
    `api/todos/${drag.id}/move?${drag.query}`,
    "POST",
    { ...drag.target, expected_revision: drag.revision },
    null,
  );
  if (saved) {
    revealParent(drag.target.parent_id);
    if (state.editor && !state.editor.dirty && !state.editor.moveDirty)
      loadEditor(state.selected);
    message("drag-result", "Task and descendants moved");
    clearTimeout(state.dragResultTimer);
    state.dragResultTimer = setTimeout(() => message("drag-result", ""), 2500);
  } else {
    state.mutationError =
      "Move was not saved. The list may have changed; review it and drag again. Your editor draft is preserved.";
    await refresh();
    message("error", state.mutationError);
  }
}
document.addEventListener("keydown", (event) => {
  if (event.key === "Escape" && state.drag) {
    event.preventDefault();
    cancelDrag();
  }
});
window.addEventListener("blur", cancelDrag);

document.addEventListener("visibilitychange", () => {
  if (document.hidden) cancelDrag();
});

async function toggleCompletion(id) {
  if (state.saving || state.drag || state.snapshot.read_only) return;
  const item = state.snapshot.items.find((task) => task.id === id);
  if (!item) return;
  await setTaskState(
    id,
    item.state === "x" ? " " : "x",
    state.snapshot.revision,
    query().toString(),
  );
}
async function setTaskState(id, nextState, revision, targetQuery) {
  const saved = await mutate(
    `api/todos/${id}?${targetQuery}`,
    "PATCH",
    { state: nextState, expected_revision: revision },
    null,
  );
  if (saved) {
    if (state.editor && !state.editor.dirty && !state.editor.moveDirty)
      loadEditor(state.selected);
  } else await refresh();
}

function openStateMenu(event, id) {
  if (state.snapshot.read_only || state.saving || state.drag) return;
  const item = state.snapshot.items.find((task) => task.id === id);
  if (!item) return;
  event.preventDefault();
  closeStateMenu(false);
  state.stateMenu = {
    id,
    revision: state.snapshot.revision,
    query: query().toString(),
    focus:
      event.target.closest("button") ||
      state.rows.get(id)?.querySelector(".task-open"),
  };
  const menu = $("state-menu");
  menu.setAttribute("aria-label", `State of ${item.content}`);
  menu.replaceChildren();
  for (const option of $("state").options) {
    const button = document.createElement("button");
    button.type = "button";
    button.setAttribute("role", "menuitemradio");
    button.setAttribute("aria-checked", String(option.value === item.state));
    button.textContent = option.text;
    button.onclick = async () => {
      const selection = state.stateMenu;
      closeStateMenu();
      if (selection)
        await setTaskState(
          selection.id,
          option.value,
          selection.revision,
          selection.query,
        );
    };
    menu.append(button);
  }
  menu.hidden = false;
  const anchor = state.rows.get(id).getBoundingClientRect();
  const x = event.clientX || anchor.left;
  const y = event.clientY || anchor.bottom;
  const rect = menu.getBoundingClientRect();
  menu.style.left = `${Math.max(8, Math.min(x, innerWidth - rect.width - 8))}px`;
  menu.style.top = `${Math.max(8, Math.min(y, innerHeight - rect.height - 8))}px`;
  menu.querySelector('[aria-checked="true"]').focus({ preventScroll: true });
}
function closeStateMenu(restoreFocus = true) {
  const selection = state.stateMenu;
  state.stateMenu = null;
  $("state-menu").hidden = true;
  if (restoreFocus && selection?.focus?.isConnected)
    selection.focus.focus({ preventScroll: true });
}
$("state-menu").onkeydown = (event) => {
  const buttons = [...$("state-menu").children];
  const index = buttons.indexOf(document.activeElement);
  if (["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) {
    event.preventDefault();
    const next =
      event.key === "Home"
        ? 0
        : event.key === "End"
          ? buttons.length - 1
          : (index + (event.key === "ArrowDown" ? 1 : -1) + buttons.length) %
            buttons.length;
    buttons[next].focus();
  } else if (event.key === "Escape" || event.key === "Tab") {
    if (event.key === "Escape") event.preventDefault();
    event.stopPropagation();
    closeStateMenu();
  }
};
document.addEventListener("pointerdown", (event) => {
  if (state.stateMenu && !$("state-menu").contains(event.target))
    closeStateMenu(false);
});
window.addEventListener("resize", () => closeStateMenu());
window.addEventListener("blur", () => closeStateMenu(false));

function renderAvatar(user, url) {
  const initials = (user.email?.split("@")[0] || "Local")
    .split(/[.\s_+-]+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((part) => [...part][0])
    .join("")
    .toUpperCase();
  text($("avatar-initials"), initials || "U");
  const avatar = $("user-avatar");
  if (avatar.dataset.url === (url || "")) return;
  avatar.dataset.url = url || "";
  avatar.hidden = true;
  $("avatar-initials").hidden = false;
  if (!url) {
    avatar.removeAttribute("src");
    return;
  }
  avatar.onload = () => {
    avatar.hidden = false;
    $("avatar-initials").hidden = true;
  };
  avatar.onerror = () => {
    avatar.hidden = true;
    $("avatar-initials").hidden = false;
  };
  avatar.src = url;
}

async function refreshServer() {
  try {
    const info = await api("api/server");
    state.server = info;
    text($("server-version"), `Server version: ${info.version}`);
    $("server-current").hidden =
      !info.latest_version ||
      !!info.check_error ||
      info.update_available ||
      info.upgrade_pending;
    $("server-version").title =
      info.check_error || `Server version ${info.version}`;
    text(
      $("current-user"),
      info.user.email ||
        (info.user.id === "local" ? "Local workspace" : info.user.id),
    );
    renderAvatar(info.user, info.avatar_url);
    $("logout").hidden = !info.logout_url;
    $("server-upgrade").hidden = !info.update_available;
    $("server-upgrade").disabled = !info.can_upgrade || info.upgrade_pending;
    text($("server-upgrade"), `⬆️ v${info.latest_version}`);
    $("server-upgrade").title = info.can_upgrade
      ? "Back up and upgrade server"
      : "Update available — ask the server owner to upgrade";
    const status = info.upgrade_status;
    message(
      "server-message",
      info.upgrade_pending
        ? "Server upgrade in progress. The server will briefly restart; your drafts stay in this tab."
        : status?.state === "failed"
          ? status.message
          : "",
    );
  } catch (error) {
    $("server-current").hidden = true;
    message(
      "server-message",
      `Server information unavailable: ${error.message}`,
    );
  }
}
$("account-menu").addEventListener("beforetoggle", (event) => {
  if (event.newState !== "open") return;
  const anchor = $("account-menu-button").getBoundingClientRect();
  const menu = $("account-menu");
  menu.style.left = `${Math.max(8, Math.min(anchor.left, innerWidth - 168))}px`;
  menu.style.top = anchor.top < 120 ? `${anchor.bottom + 8}px` : "auto";
  menu.style.bottom =
    anchor.top < 120 ? "auto" : `${innerHeight - anchor.top + 8}px`;
});
function accountMenuItems() {
  return [...$("account-menu").querySelectorAll('[role="menuitem"]')].filter(
    (item) => !item.hidden,
  );
}
$("account-menu").addEventListener("toggle", (event) => {
  if (event.newState === "open")
    accountMenuItems()[0]?.focus({ preventScroll: true });
});
$("account-menu").onkeydown = (event) => {
  event.stopPropagation();
  if (["ArrowUp", "ArrowDown", "Home", "End"].includes(event.key)) {
    event.preventDefault();
    const items = accountMenuItems();
    const current = items.indexOf(document.activeElement);
    const index =
      event.key === "Home"
        ? 0
        : event.key === "End"
          ? items.length - 1
          : (current + (event.key === "ArrowDown" ? 1 : -1) + items.length) %
            items.length;
    items[index]?.focus({ preventScroll: true });
  } else if (event.key === "Tab") $("account-menu").hidePopover();
};
window.addEventListener("resize", () => $("account-menu").hidePopover());
document.addEventListener(
  "scroll",
  () => $("account-menu").hidePopover(),
  true,
);
$("logout").onclick = () => {
  $("account-menu").hidePopover();
  if (
    [...state.drafts.values()].some(
      (draft) => draft.dirty || draft.moveDirty,
    ) &&
    !confirm("Log out and discard unsaved drafts?")
  )
    return;
  state.drafts.clear();
  location.assign(state.server.logout_url);
};
$("server-upgrade").onclick = async () => {
  const info = state.server;
  if (
    !info?.can_upgrade ||
    !confirm(
      `Back up all server data and upgrade from v${info.version} to v${info.latest_version}? The server will briefly be unavailable.`,
    )
  )
    return;
  $("server-upgrade").disabled = true;
  try {
    await api("api/server/upgrade", {
      method: "POST",
      body: JSON.stringify({ version: info.latest_version }),
    });
    message(
      "server-message",
      "Upgrade queued. Backing up server data before installation…",
    );
  } catch (error) {
    message("server-message", error.message);
    $("server-upgrade").disabled = false;
  }
};
refreshServer();
setInterval(refreshServer, 15000);
