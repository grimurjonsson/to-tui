import assert from "node:assert/strict";
import { test } from "node:test";
import { TotuiCliClient, type CliRunner } from "./client.js";
import { TotuiDataSource } from "./data-source.js";

const cwd = "/workspace/MergeQuest";
const destination = { backend: "remote", remote: "home", server_url: "https://totui.example", project: "MergeQuest", directory: cwd, folder: cwd };
const item = { id: "remote-id", content: "Remote task", state: " ", indent_level: 0 };
function harness(respond?: (args: string[]) => unknown) {
  const calls: { command: string; args: string[]; cwd: string; signal?: AbortSignal }[] = [];
  const runner: CliRunner = async (command, args, options) => {
    calls.push({ command, args, cwd: options.cwd, signal: options.signal });
    if (respond) return respond(args);
    if (args.includes("context")) return destination;
    if (args.includes("list")) return [item];
    return item;
  };
  const client = new TotuiCliClient({ cwd }, runner);
  return { client, calls, data: new TotuiDataSource(client) };
}

test("panel and tools resolve cwd and read the same pinned remote, not local/default", async () => {
  const { client, data, calls } = harness();
  const list = await data.listTodos("2026-09-14");
  const tool = await client.callTool("list_todos", { date: "2026-09-14" });
  assert.deepEqual(list.items, [item]);
  assert.deepEqual(tool.result, [item]);
  assert.equal(list.destination?.project, "MergeQuest");
  assert.equal(list.source, "cli");
  assert.deepEqual(calls.map(c => c.args), [
    ["todo", "context"],
    ["--remote", "home", "todo", "list", "--project", "MergeQuest", "--date", "2026-09-14"],
    ["--remote", "home", "todo", "list", "--project", "MergeQuest", "--date", "2026-09-14"],
  ]);
  assert.ok(calls.every(c => c.cwd === cwd && c.command === "totui"));
});

test("all mutation payloads use CLI JSON with scope outside payload", async () => {
  const { client, calls } = harness();
  const content = "quotes ' \" $(touch nope)\nnew line";
  let announced = false;
  await client.callTool("create_todo", { content, parent_id: "parent", date: "2026-09-14" }, undefined, d => {
    assert.equal(d.project, "MergeQuest");
    assert.equal(calls.length, 1);
    announced = true;
  });
  assert.ok(announced);
  assert.deepEqual(calls[1].args.slice(0, 5), ["--remote", "home", "todo", "create", "--json"]);
  assert.deepEqual(JSON.parse(calls[1].args[5]), { content, parent_id: "parent" });
  await client.callTool("update_todo", { id: item.id, state: "*", description: "", due_date: "2026-10-01" });
  assert.deepEqual(calls[2].args.slice(0, 6), ["--remote", "home", "todo", "update", item.id, "--json"]);
  assert.deepEqual(JSON.parse(calls[2].args[6]), { state: "*", description: "", due_date: "2026-10-01" });
  await client.callTool("delete_todo", { id: item.id });
  assert.deepEqual(calls[3].args.slice(0, 5), ["--remote", "home", "todo", "delete", item.id]);
});

test("toggle reads current CLI state and pins the same date for its update", async () => {
  const { client, calls } = harness(args => args.includes("context") ? destination : { ...item, state: "x" });
  await client.callTool("mark_complete", { id: item.id });
  assert.ok(calls[1].args.includes("get"));
  assert.equal(JSON.parse(calls[2].args[6]).state, " ");
  assert.deepEqual(calls[1].args.slice(-4), calls[2].args.slice(-4));
});

test("explicit project is validated on the pinned backend without changing panel project", async () => {
  const { client, calls } = harness(args => args.includes("context")
    ? { ...destination, project: args.includes("Other") ? "Other" : "MergeQuest" } : [item]);
  const result = await client.callTool("list_todos", { project: "Other" });
  assert.equal(result.destination.project, "Other");
  assert.deepEqual(calls[1].args, ["--remote", "home", "todo", "context", "--project", "Other"]);
  assert.equal((await client.context()).project, "MergeQuest");
});

test("remote auth failure is an error, never a local or MCP fallback", async () => {
  const { client, data, calls } = harness(args => {
    if (args.includes("context")) return destination;
    throw new Error("authentication expired");
  });
  await assert.rejects(client.callTool("list_todos", {}), /authentication expired/);
  const list = await data.listTodos();
  assert.equal(list.source, "error");
  assert.match(list.error!, /authentication expired/);
  assert.equal(calls.length, 3);
  assert.ok(calls.slice(1).every(c => c.args[0] === "--remote"));
});

test("local selection is explicit and pinned, missing CLI context cannot fall back", async () => {
  const calls: string[][] = [];
  const client = new TotuiCliClient({ cwd, local: true }, async (_cmd, args) => {
    calls.push(args);
    return args.includes("context") ? { ...destination, backend: "local", remote: null, server_url: null } : [item];
  });
  await client.callTool("list_todos", {});
  assert.equal(calls[0][0], "--local");
  assert.equal(calls[1][0], "--local");
  const broken = harness(() => { throw new Error("unrecognized subcommand 'context'"); });
  await assert.rejects(broken.client.callTool("create_todo", { content: "no" }), /context/);
  assert.equal(broken.calls.length, 1);
});

test("panel focus and completion mutations share the remote client", async () => {
  const { data, calls } = harness();
  await data.setFocus(item.id, [item, { ...item, id: "focused", state: "*" }], "2026-09-14");
  const writes = calls.filter(c => c.args.includes("update"));
  assert.equal(writes.length, 2);
  assert.equal(JSON.parse(writes[0].args[6]).state, " ");
  assert.equal(JSON.parse(writes[1].args[6]).state, "*");
  await data.toggleTodo(item.id, [item], "2026-09-14");
  assert.ok(calls.some(c => c.args.includes("get")));
  assert.ok(calls.slice(1).every(c => c.args[0] === "--remote"));
});

test("malformed context and list JSON fail closed", async () => {
  const bad = harness(() => ({ backend: "remote", project: "MergeQuest" }));
  await assert.rejects(bad.client.callTool("create_todo", { content: "no" }), /context/i);
  assert.equal(bad.calls.length, 1);
  const wrongList = harness(args => args.includes("context") ? destination : { items: [item] });
  assert.equal((await wrongList.data.listTodos()).source, "error");
});

test("projects use the selected backend and cancellation propagates", async () => {
  const { client, calls } = harness(args => args.includes("context") ? destination : ["MergeQuest"]);
  const controller = new AbortController();
  assert.deepEqual((await client.callTool("list_projects", {}, controller.signal)).result, ["MergeQuest"]);
  assert.deepEqual(calls[1].args, ["--remote", "home", "todo", "projects"]);
  assert.ok(calls[1].signal);
  controller.abort();
  assert.equal(calls[1].signal!.aborted, true);
  await assert.rejects(client.callTool("delete_todo", { id: item.id }, controller.signal), /abort/i);
  assert.equal(calls.length, 2);
});
