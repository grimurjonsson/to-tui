"use strict";
const $ = id => document.getElementById(id);
const columns = ["backlog", "ready", "in_progress", "review", "done", "blocked"];
const label = value => value.replaceAll("_", " ").replace(/^./, c => c.toUpperCase());
let project = new URLSearchParams(location.search).get("project") || "default";
let board = null, projects = [], userId = null, editorTicket = null, mode = null;
let generation = 0, saving = false, refreshing = false, pending = false;
let rendered = null;
let draggedTicket = null;
const statusRank = status => ({ backlog: 0, ready: 1, in_progress: 2, blocked: 2, review: 3, done: 4 })[status];
function clearDrag() {
  draggedTicket = null;
  document.querySelectorAll(".dragging,.drop-target").forEach(node => node.classList.remove("dragging", "drop-target"));
}
async function dropTicket(status) {
  const ticket = draggedTicket;
  clearDrag();
  if (!ticket || saving || ticket.status === status) { refresh(); return; }
  if (statusRank(status) < statusRank(ticket.status)) {
    openEditor("ticket", ticket);
    $("status").value = status;
    $("reason").focus();
    render();
    refresh();
    return;
  }
  saving = true; generation++; setDisabled();
  try {
    board = await api("/api/kanban", { method: "POST", body: JSON.stringify({ project, actor: "user", action: "move_ticket", id: ticket.id, expected_revision: ticket.revision, status, reason: null }) });
    render();
  } catch (error) {
    openEditor("ticket", ticket);
    $("status").value = status;
    message("form-error", error.message);
  } finally { saving = false; setDisabled(); refresh(); }
}
function message(id, value) { $(id).textContent = value; $(id).hidden = !value; }
function element(tag, text, className) { const node = document.createElement(tag); node.textContent = text; if (className) node.className = className; return node; }
async function api(path, options = {}) {
  const response = await fetch(path, { cache: "no-store", ...options, headers: { "Content-Type": "application/json", ...(userId ? { "X-Totui-Expected-User": userId } : {}) } });
  const nextUser = response.headers.get("X-Totui-User");
  if (response.status === 401 || (userId && nextUser && userId !== nextUser)) {
    document.body.replaceChildren(element("p", "Your session changed. Reload to continue."));
    events.close(); throw new Error("Session changed");
  }
  if (nextUser) userId = nextUser;
  const data = await response.json().catch(() => ({}));
  if (!response.ok) throw new Error(data.error || `Request failed (${response.status})`);
  return data;
}
function render() {
  $("heading").textContent = board?.name || "Your project board";
  $("summary").textContent = board ? `${board.tickets.filter(t => !t.trashed && !t.archived).length} ${board.tickets.filter(t => !t.trashed && !t.archived).length === 1 ? "ticket" : "tickets"} · Shared with your agents` : "Create a board to organize work with your agents.";
  $("create").textContent = board ? "+ New ticket" : "Create board";
  $("create").disabled = saving;
  $("tasks-link").href = `/?project=${encodeURIComponent(project)}`;
  const signature = JSON.stringify([project, board]);
  if (signature !== rendered) {
  rendered = signature;
  for (const id of ["columns", "backlog", "trash-tickets", "completed-tickets"]) $(id).replaceChildren();
  $("completed").hidden = !board;
  $("completed-summary").textContent = `Completed (${board?.tickets.filter(t => t.archived).length || 0})`;
  const trashCount = board?.tickets.filter(t => t.trashed).length || 0;
  $("trash").hidden = !trashCount;
  $("trash-summary").textContent = `Trash (${trashCount}) · Restore tickets`;
  if (board) for (const status of [...columns.filter(s => s !== "backlog"), "backlog", "completed", "trash"]) {
    const tickets = board.tickets.filter(t => status === "trash" ? t.trashed : status === "completed" ? t.archived : !t.trashed && !t.archived && t.status === status);
    const column = element("section", "", "kanban-column");
    column.dataset.status = status;
    column.ondragover = event => {
      if (!draggedTicket || saving || ["trash", "completed"].includes(status) || draggedTicket.status === status) return;
      event.preventDefault(); event.dataTransfer.dropEffect = "move";
      column.classList.add("drop-target");
    };
    column.ondragleave = event => { if (!column.contains(event.relatedTarget)) column.classList.remove("drop-target"); };
    column.ondrop = event => { event.preventDefault(); if (!["trash", "completed"].includes(status)) dropTicket(status); };
    const heading = element("h2", status === "completed" ? "Archived from Done" : label(status)); heading.append(element("span", tickets.length)); column.append(heading);
    const list = element("div", "", "ticket-list"); column.append(list);
    for (const ticket of tickets) {
      const isRow = ["backlog", "completed", "trash"].includes(status);
      const card = element("article", "", isRow ? "kanban-row" : "kanban-card");
      const open = element("button", "", "kanban-card-open"); open.type = "button";
      card.draggable = !ticket.trashed && !ticket.archived;
      card.ondragstart = event => {
        if (saving || $("editor").open) { event.preventDefault(); return; }
        draggedTicket = structuredClone(ticket); generation++;
        event.dataTransfer.effectAllowed = "move";
        event.dataTransfer.setData("text/plain", ticket.id);
        card.classList.add("dragging");
      };
      card.ondragend = () => { clearDrag(); refresh(); };
      open.append(element("strong", ticket.title), element("span", ticket.assignee || "Unassigned"));
      if (ticket.feedback) open.append(element("span", "Feedback needs attention", "needs-feedback"));
      open.append(element("span", `Updated ${new Date(ticket.updated_at).toLocaleString()}`));
      open.onclick = () => openEditor("ticket", ticket);
      card.append(open);
      const actions = element("div", "", "card-actions");
      function shortcut(text, handler, className) {
        const button = element("button", text, className); button.type = "button";
        button.onclick = handler; button.disabled = saving; actions.append(button);
      }
      if (ticket.trashed) shortcut("Restore", () => quickAction(ticket, "restore_ticket"));
      else if (ticket.archived) shortcut("Restore to Done", () => quickAction(ticket, "unarchive_ticket"));
      else if (ticket.status === "backlog") {
        shortcut("To board", () => quickAction(ticket, "move_ticket", { status: "ready", reason: null }));
        shortcut("Trash", () => quickAction(ticket, "trash_ticket"), "danger");
      } else if (ticket.status === "done") shortcut("Archive", () => quickAction(ticket, "archive_ticket"));
      if (actions.childElementCount) card.append(actions); list.append(card);
    }
    if (!tickets.length) column.append(element("p", "No tickets", "empty-column"));
    $(status === "backlog" ? "backlog" : status === "trash" ? "trash-tickets" : status === "completed" ? "completed-tickets" : "columns").append(column);
  }
  }
  if (editorTicket && $("editor").open) {
    const current = board?.tickets.find(t => t.id === editorTicket.id);
    $("stale").hidden = current?.revision === editorTicket.revision;
    setDisabled();
  }
}
async function refresh() {
  if (saving || refreshing || draggedTicket) { pending = true; return; }
  refreshing = true;
  const requestGeneration = generation;
  try {
    const result = await api("/api/projects");
    if (requestGeneration !== generation) return;
    const previous = projects.find(p => p.name === project);
    projects = result.projects;
    if (previous) project = projects.find(p => p.id === previous.id)?.name || project;
    if (!projects.some(p => p.name === project)) {
      project = projects[0]?.name || "default";
      if ($("editor").open) $("editor").close();
    }
    $("project").replaceChildren(...projects.map(p => { const option = element("option", p.name); option.value = p.name; return option; }));
    $("project").value = project;
    const next = await api(`/api/kanban?project=${encodeURIComponent(project)}`);
    if (requestGeneration !== generation) return;
    board = next; render(); message("error", "");
    history.replaceState(null, "", `?project=${encodeURIComponent(project)}`);
  } catch (error) { if (requestGeneration === generation) message("error", error.message); }
  finally { refreshing = false; if (pending) { pending = false; refresh(); } }
}
function setDisabled() {
  const stale = editorTicket && !$("stale").hidden;
  $("project").disabled = saving;
  $("create").disabled = saving;
  $("reload-ticket").disabled = saving;
  document.querySelectorAll(".card-actions button").forEach(button => { button.disabled = saving; });
  for (const id of ["save", "move", "add-comment", "resolve", "to-board", "archive-ticket", "trash-ticket"]) $(id).disabled = saving || stale || ((editorTicket?.trashed || editorTicket?.archived) && id !== "trash-ticket" && id !== "archive-ticket");
}
function openEditor(nextMode, ticket = null) {
  mode = nextMode; editorTicket = ticket ? structuredClone(ticket) : null;
  message("form-error", ""); $("stale").hidden = true;
  $("editor-heading").textContent = mode === "board" ? "Create board" : ticket ? "Ticket details" : "New ticket";
  $("board-fields").hidden = mode !== "board"; $("ticket-fields").hidden = mode === "board";
  $("ticket-actions").hidden = !ticket;
  $("board-name").value = "Delivery"; $("title").value = ticket?.title || "";
  $("description").value = ticket?.description || ""; $("assignee").value = ticket?.assignee || "";
  for (const id of ["reason", "comment", "resolution"]) $(id).value = "";
  if (ticket) {
    $("status").value = ticket.status;
    $("to-board").hidden = ticket.status !== "backlog" || ticket.trashed || ticket.archived;
    $("trash-ticket").hidden = !ticket.trashed && (ticket.status !== "backlog" || ticket.archived);
    $("archive-ticket").hidden = ticket.status !== "done" || ticket.trashed;
    $("archive-ticket").textContent = ticket.archived ? "Restore to Done" : "Archive";
    $("trash-ticket").textContent = ticket.trashed ? "Restore" : "Trash";
    $("feedback").classList.toggle("needs-feedback", Boolean(ticket.feedback));
    $("feedback").textContent = ticket.feedback ? `Outstanding feedback\n${ticket.feedback}` : "No outstanding move feedback.";
    $("resolution-fields").hidden = !ticket.feedback;
    $("activity").replaceChildren(...ticket.activity.slice().reverse().map(event => {
      const item = element("li", `${label(event.action)}${event.from ? `: ${label(event.from)} → ${label(event.to)}` : ""}${event.body ? `\n${event.body}` : ""}`);
      item.append(element("small", `${event.actor} · ${new Date(event.at).toLocaleString()}`)); return item;
    }));
  }
  setDisabled(); if (!$("editor").open) $("editor").showModal();
}
async function mutate(action, fields) {
  if (saving) return;
  const draft = Object.fromEntries(["title", "description", "assignee", "reason", "comment", "resolution"].map(id => [id, $(id).value]));
  saving = true; generation++; setDisabled();
  try {
    const result = await api("/api/kanban", { method: "POST", body: JSON.stringify({ project, actor: "user", action, ...fields }) });
    board = result; message("form-error", "");
    const id = editorTicket?.id;
    if (id && !["trash_ticket", "restore_ticket", "archive_ticket", "unarchive_ticket"].includes(action)) {
      openEditor("ticket", board.tickets.find(t => t.id === id));
      const submitted = { edit_ticket: ["title", "description", "assignee"], move_ticket: ["reason"], comment: ["comment"], address_feedback: ["resolution"] }[action] || [];
      for (const [field, value] of Object.entries(draft)) if (!submitted.includes(field)) $(field).value = value;
    } else $("editor").close();
    render();
  } catch (error) { message("form-error", error.message); $("form-error").scrollIntoView({ block: "center" }); }
  finally { saving = false; setDisabled(); refresh(); }
}
async function quickAction(ticket, action, fields = {}) {
  if (saving) return;
  saving = true; generation++; setDisabled();
  try {
    board = await api("/api/kanban", { method: "POST", body: JSON.stringify({ project, actor: "user", action, id: ticket.id, expected_revision: ticket.revision, ...fields }) });
    render(); message("error", "");
  } catch (error) { openEditor("ticket", ticket); message("form-error", error.message); }
  finally { saving = false; setDisabled(); refresh(); }
}
$("to-board").onclick = () => mutate("move_ticket", { ...ticketFields(), status: "ready", reason: null });
$("archive-ticket").onclick = () => mutate(editorTicket.archived ? "unarchive_ticket" : "archive_ticket", ticketFields());
$("trash-ticket").onclick = () => mutate(editorTicket.trashed ? "restore_ticket" : "trash_ticket", ticketFields());
const ticketFields = () => ({ id: editorTicket.id, expected_revision: editorTicket.revision });
$("create").onclick = () => openEditor(board ? "new" : "board");
$("close").onclick = () => { if (!saving) $("editor").close(); };
$("editor").addEventListener("cancel", event => { if (saving) event.preventDefault(); });
$("reload-ticket").onclick = () => { const ticket = board?.tickets.find(t => t.id === editorTicket?.id); if (ticket) openEditor("ticket", ticket); else $("editor").close(); };
$("ticket-form").onsubmit = event => {
  event.preventDefault();
  if (mode === "board") return mutate("create_board", { name: $("board-name").value });
  mutate(editorTicket ? "edit_ticket" : "create_ticket", { ...(editorTicket ? ticketFields() : {}), title: $("title").value, description: $("description").value, assignee: $("assignee").value.trim() || null });
};
$("move").onclick = () => mutate("move_ticket", { ...ticketFields(), status: $("status").value, reason: $("reason").value.trim() || null });
$("add-comment").onclick = () => mutate("comment", { ...ticketFields(), body: $("comment").value });
$("resolve").onclick = () => mutate("address_feedback", { ...ticketFields(), resolution: $("resolution").value });
$("project").onchange = () => { clearDrag(); generation++; project = $("project").value; board = null; $("editor").close(); refresh(); };
$("refresh").onclick = refresh;
$("status").replaceChildren(...columns.map(status => { const option = element("option", label(status)); option.value = status; return option; }));
$("theme").onclick = () => { document.documentElement.dataset.theme = document.documentElement.dataset.theme === "dark" ? "light" : "dark"; };
if (matchMedia("(prefers-color-scheme: dark)").matches) document.documentElement.dataset.theme = "dark";
const events = new EventSource("/api/events");
events.onopen = () => { $("connection").textContent = "● Live"; refresh(); };
events.addEventListener("change", refresh);
events.onerror = () => { $("connection").textContent = "Reconnecting…"; };
window.addEventListener("focus", refresh); window.addEventListener("online", refresh);
setInterval(() => { if (!document.hidden) refresh(); }, 15000);
refresh();
