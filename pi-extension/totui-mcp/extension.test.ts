import assert from "node:assert/strict";
import { test } from "node:test";
import { chmod, mkdtemp, mkdir, readFile, realpath, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import extension from "./index.js";
import { visibleWidth } from "@earendil-works/pi-tui";
import { TotuiCliClient, runCli } from "./client.js";
import { TotuiDataSource } from "./data-source.js";
import { TotuiPanelComponent } from "./panel.js";
import { startWidgetPoller } from "./widget.js";
import type { TotuiTodoList } from "./types.js";

async function fixture(t: { after: (fn: () => Promise<void>) => void }) {
  const root = await mkdtemp(join(tmpdir(), "totui-extension-test-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  const command = join(root, "fake-totui");
  const log = join(root, "calls.jsonl");
  await mkdir(join(root, "MergeQuest"));
  const cwd = await realpath(join(root, "MergeQuest"));
  await writeFile(command, `#!/usr/bin/env node
const fs = require('node:fs');
const args = process.argv.slice(2);
const log = ${JSON.stringify(log)};
fs.appendFileSync(log, JSON.stringify({args, cwd:process.cwd()})+'\\n');
const opt = name => args.includes(name) ? args[args.indexOf(name)+1] : undefined;
const op = args[args.indexOf('todo')+1];
const project = opt('--project') || require('node:path').basename(process.cwd());
const item = {id:'remote-id', content:'Remote task', state:' ', indent_level:0};
let result;
if(op==='context') result = {backend:'remote',remote:opt('--remote') || 'home',server_url:'https://totui.example',project,directory:process.cwd()};
else if(op==='list') result = [item];
else if(op==='get') result = {...item,state:'x'};
else if(op==='projects') result = ['MergeQuest','Other'];
else if(op==='create'||op==='update') { result = {...item,...JSON.parse(opt('--json'))}; process.stderr.write('Destination: remote home\\n'); }
else if(op==='delete') result = {deleted:['remote-id']};
else {process.stderr.write('Unexpected command'); process.exit(1);}
process.stdout.write(JSON.stringify(result));
`);
  await chmod(command, 0o755);
  return { root, command, cwd, calls: async () => (await readFile(log, "utf8")).trim().split("\n").map(line => JSON.parse(line)) };
}

function host(command: string, cwd: string) {
  const tools = new Map<string, any>();
  const events = new Map<string, any>();
  const commands = new Map<string, any>();
  const flags = new Map<string, unknown>([["totui-command", command]]);
  const widgets: any[] = [];
  const notifications: string[] = [];
  const theme = { fg: (_color: string, text: string) => text };
  const ctx: any = { cwd, hasUI: false, mode: "print", ui: {
    theme, notify: (text: string) => notifications.push(text),
    setWidget: (_name: string, value: unknown) => widgets.push(value), setStatus: () => {},
    onTerminalInput: () => () => {},
  } };
  extension({
    registerFlag: () => {}, getFlag: (name: string) => flags.get(name),
    registerTool: (tool: any) => tools.set(tool.name, tool),
    registerCommand: (name: string, command: any) => commands.set(name, command),
    registerShortcut: () => {}, on: (name: string, handler: any) => events.set(name, handler),
  } as any);
  const call = async (name: string, params: object = {}, context = ctx) => {
    const response = await tools.get(`totui_${name}`).execute("call", params, undefined, undefined, context);
    return JSON.parse(response.content[0].text);
  };
  return { tools, events, commands, flags, widgets, notifications, ctx, call, theme };
}

test("registered tools invoke actual CLI subprocesses with correct cwd, scope and payloads", async t => {
  const f = await fixture(t);
  const h = host(f.command, f.cwd);
  assert.equal(h.tools.size, 7);
  assert.equal((await h.call("context")).destination.project, "MergeQuest");
  assert.equal((await h.call("list_todos")).result[0].id, "remote-id");
  const content = "literal $(touch never) ' \"\nsecond line";
  assert.equal((await h.call("create_todo", { content, parent_id: "parent" })).result.content, content);
  assert.equal((await h.call("update_todo", { id: "remote-id", state: "*" })).result.state, "*");
  assert.equal((await h.call("mark_complete", { id: "remote-id" })).result.state, " ");
  assert.deepEqual((await h.call("delete_todo", { id: "remote-id" })).result.deleted, ["remote-id"]);
  assert.deepEqual((await h.call("list_projects")).result, ["MergeQuest", "Other"]);
  const calls = await f.calls();
  assert.ok(calls.every(c => c.cwd === f.cwd && c.args.includes("todo")));
  assert.ok(calls.slice(1).every(c => c.args[0] === "--remote" && c.args[1] === "home"));
  assert.ok(calls.every(c => !c.args.includes("serve")));
});

test("widget startup and panel mutations use CLI, shutdown clears UI, tools respect another cwd", async t => {
  const f = await fixture(t);
  const h = host(f.command, f.cwd);
  h.ctx.hasUI = true;
  h.ctx.mode = "tui";
  await h.events.get("session_start")({}, h.ctx);
  assert.match(h.widgets.at(-1)[0], /MergeQuest on home/);
  assert.match(h.widgets.at(-1)[0], /0\/1/);
  const other = join(f.root, "Other");
  await mkdir(other);
  assert.equal((await h.call("context", {}, { ...h.ctx, cwd: other })).destination.project, "Other");
  await h.events.get("session_shutdown")({}, h.ctx);
  assert.equal(h.widgets.at(-1), undefined);
});

test("runtime flags and reconnect re-resolve rather than retaining a local default", async t => {
  const f = await fixture(t);
  const h = host(f.command, f.cwd);
  h.flags.set("totui-project", "Other");
  h.flags.set("totui-remote", "work");
  assert.equal((await h.call("context")).destination.project, "Other");
  assert.equal((await h.call("context")).destination.remote, "work");
  h.flags.set("totui-project", "MergeQuest");
  await h.commands.get("totui-reconnect").handler("", h.ctx);
  assert.equal((await h.call("context")).destination.project, "MergeQuest");
  h.flags.set("totui-api-url", "https://old-custom-backend");
  await h.commands.get("totui-reconnect").handler("", h.ctx).then(
    () => assert.fail("legacy URL must not silently select a different backend"),
    (error: Error) => assert.match(error.message, /Remove --totui-api-url/),
  );
});

test("legacy API binary flag and retained MCP default work after reload", async t => {
  const f = await fixture(t);
  const h = host(f.command, f.cwd);
  h.flags.delete("totui-command");
  h.flags.set("totui-api-command", f.command);
  h.flags.set("totui-mcp-command", "totui-mcp");
  assert.equal((await h.call("context")).destination.remote, "home");
  assert.equal((await h.call("list_todos")).result[0].id, "remote-id");
  assert.ok((await f.calls()).every(c => c.args.includes("todo") && !c.args.includes("serve")));
});

test("modern binary setting takes precedence over the legacy API command", async t => {
  const f = await fixture(t);
  const h = host(f.command, f.cwd);
  h.flags.set("totui-api-command", "/nonexistent/legacy/totui");
  assert.equal((await h.call("context")).destination.remote, "home");
});

test("legacy API command environment variable remains a CLI alias", async t => {
  const f = await fixture(t);
  const previous = process.env.TOTUI_API_COMMAND;
  process.env.TOTUI_API_COMMAND = f.command;
  t.after(async () => {
    if (previous === undefined) delete process.env.TOTUI_API_COMMAND;
    else process.env.TOTUI_API_COMMAND = previous;
  });
  const h = host(f.command, f.cwd);
  h.flags.delete("totui-command");
  assert.equal((await h.call("context")).destination.remote, "home");
});

test("reconnect accepts a distinct command context and updates the widget destination", async t => {
  const f = await fixture(t);
  const h = host(f.command, f.cwd);
  h.ctx.hasUI = true;
  h.ctx.mode = "tui";
  await h.events.get("session_start")({}, h.ctx);
  t.after(() => h.events.get("session_shutdown")({}, h.ctx));
  h.flags.set("totui-project", "Other");
  await h.commands.get("totui-reconnect").handler("", { ...h.ctx });
  assert.match(h.widgets.at(-1)[0], /Other on home/);
});

test("shutdown invalidates a pending panel open before its read finishes", async t => {
  const f = await fixture(t);
  const h = host(f.command, f.cwd);
  h.ctx.hasUI = true;
  h.ctx.mode = "tui";
  await h.events.get("session_start")({}, h.ctx);
  let finish!: (list: TotuiTodoList) => void;
  t.mock.method(TotuiDataSource.prototype, "listTodos", () => new Promise<TotuiTodoList>(resolve => { finish = resolve; }));
  let opened = 0;
  h.ctx.ui.custom = async () => { opened++; };
  const opening = h.commands.get("totui").handler("", { ...h.ctx });
  await h.events.get("session_shutdown")({}, h.ctx);
  finish({ source: "error", date: "2026-09-14", items: [], error: "aborted" });
  await opening;
  assert.equal(opened, 0);
});

test("real process runner rejects bad JSON, nonzero exits, missing binary and cancellation", async t => {
  const f = await fixture(t);
  await assert.rejects(runCli(join(f.root, "missing"), [], { cwd: f.cwd }), /CLI failed/);
  await writeFile(f.command, "#!/usr/bin/env node\nprocess.stdout.write('not JSON');\n");
  await assert.rejects(runCli(f.command, [], { cwd: f.cwd }), /invalid JSON/);
  await writeFile(f.command, "#!/usr/bin/env node\nprocess.stderr.write('login required');process.exit(1);\n");
  await assert.rejects(runCli(f.command, [], { cwd: f.cwd }), /login required/);
  const controller = new AbortController();
  controller.abort();
  await assert.rejects(runCli(f.command, [], { cwd: f.cwd, signal: controller.signal }), /abort/i);
});

test("poller coalesces overlapping reads and does not repaint after stop", async () => {
  let resolve!: (list: TotuiTodoList) => void;
  let count = 0;
  const updates: TotuiTodoList[] = [];
  const data = { listTodos: () => { count++; return new Promise<TotuiTodoList>(r => { resolve = r; }); } };
  const poller = startWidgetPoller(data as TotuiDataSource, 60_000, list => updates.push(list));
  const a = poller.refresh();
  const b = poller.refresh();
  assert.equal(count, 1);
  poller.stop();
  resolve({ source: "cli", date: "2026-09-14", items: [] });
  await Promise.all([a, b]);
  assert.equal(updates.length, 0);
});

test("empty error panel reports failure, clips long errors and requests redraw", async () => {
  const client = new TotuiCliClient({ cwd: "/tmp" }, async () => { throw new Error("offline"); });
  const list = await new TotuiDataSource(client).listTodos();
  let renders = 0;
  const panel = new TotuiPanelComponent({ ...list, error: "offline " + "x".repeat(200) },
    { fg: (_color: string, text: string) => text } as any,
    new TotuiDataSource(client), () => {}, () => {}, () => renders++);
  assert.match(panel.render(50).join("\n"), /offline/);
  assert.doesNotMatch(panel.render(50).join("\n"), /No todos/);
  assert.ok(panel.render(50).every(line => visibleWidth(line) <= 50));
  assert.doesNotThrow(() => panel.render(1));
  panel.invalidate();
  assert.ok(renders > 0);
  client.close();
  await assert.rejects(client.context(), /abort/i);
});
