import { test, expect } from "@playwright/test";
import { spawn, execFileSync } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync, realpathSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { createServer } from "node:net";
import { createInterface } from "node:readline";
let directory, server, base, env;
const binary = resolve("../target/debug/totui");
const cli = (...args) =>
  JSON.parse(
    execFileSync(binary, ["todo", ...args], { env, encoding: "utf8" }),
  );
test.beforeEach(async () => {
  directory = mkdtempSync(resolve(tmpdir(), "totui-browser-"));
  env = { ...process.env, TOTUI_DATA_DIR: directory };
  const socket = createServer();
  await new Promise((done) => socket.listen(0, "127.0.0.1", done));
  const port = socket.address().port;
  await new Promise((done) => socket.close(done));
  base = `http://127.0.0.1:${port}`;
  server = spawn(binary, ["web", "--port", String(port)], {
    env,
    stdio: "ignore",
  });
  await expect
    .poll(async () => {
      try {
        return (await fetch(`${base}/api/health`)).status;
      } catch {
        return 0;
      }
    })
    .toBe(200);
});
test.afterEach(async () => {
  if (server.exitCode === null) {
    server.kill();
    await new Promise((done) => server.once("exit", done));
  }
  rmSync(directory, { recursive: true, force: true });
});

async function createInBrowser(page, content) {
  await page.getByRole("button", { name: "+ New task", exact: true }).click();
  await page.getByRole("textbox", { name: "Task", exact: true }).fill(content);
  await page.getByRole("button", { name: "Save task", exact: true }).click();
  await expect(
    page.locator(".task-title").filter({ hasText: content }),
  ).toBeVisible();
}

test("drafts, conflict recovery, external writers, tabs, and measured rendering", async ({
  page,
  context,
}) => {
  const task = cli("create", "--content", "Plan release", "--state", "*");
  await page.goto(base);
  await page.getByRole("button", { name: "Plan release in progress" }).click();
  await page
    .getByRole("textbox", { name: "Description", exact: true })
    .fill("An active browser draft");
  cli("update", task.id, "--content", "External CLI edit");
  await expect(page.locator(".task-title")).toHaveText("External CLI edit");
  await expect(
    page.getByRole("textbox", { name: "Description", exact: true }),
  ).toHaveValue("An active browser draft");
  await expect(
    page.getByRole("textbox", { name: "Description", exact: true }),
  ).toBeFocused();
  await page.getByRole("button", { name: "Save task", exact: true }).click();
  await expect(page.locator("#conflict")).toBeVisible();
  await page
    .getByRole("button", { name: "Keep my draft and retry against latest" })
    .click();
  await page.getByRole("button", { name: "Save task", exact: true }).click();
  await expect
    .poll(() => cli("get", task.id).description)
    .toBe("An active browser draft");
  const tab = await context.newPage();
  await tab.goto(base);
  await createInBrowser(tab, "From another browser tab");
  await expect(
    page.locator(".task-title").filter({ hasText: "From another browser tab" }),
  ).toBeVisible();
  const mcp = spawn(resolve("../target/debug/totui-mcp"), [], {
    env,
    stdio: ["pipe", "pipe", "ignore"],
  });
  const pending = new Map();
  const reader = createInterface({ input: mcp.stdout });
  reader.on("line", (line) => {
    const value = JSON.parse(line);
    pending.get(value.id)?.(value);
  });
  const send = (value) => {
    mcp.stdin.write(JSON.stringify(value) + "\n");
  };
  const call = (value) =>
    new Promise((done) => {
      pending.set(value.id, done);
      send(value);
    });
  try {
    await call({
      jsonrpc: "2.0",
      id: 1,
      method: "initialize",
      params: {
        protocolVersion: "2024-11-05",
        capabilities: {},
        clientInfo: { name: "web-tests", version: "1" },
      },
    });
    send({ jsonrpc: "2.0", method: "notifications/initialized" });
    const result = await call({
      jsonrpc: "2.0",
      id: 2,
      method: "tools/call",
      params: {
        name: "create_todo",
        arguments: { content: "From the MCP process" },
      },
    });
    expect(result.result.isError).not.toBe(true);
    await expect(
      page.locator(".task-title").filter({ hasText: "From the MCP process" }),
    ).toBeVisible();
  } finally {
    mcp.kill();
    reader.close();
  }
  await page.bringToFront();
  const latencies = [];
  for (let i = 0; i < 8; i++) {
    const rendered = page.evaluate(
      () =>
        new Promise((done) => {
          const observer = new MutationObserver(() => {
            const node = [...document.querySelectorAll(".task-title")].find(
              (n) => /^Latency \d+$/.test(n.textContent),
            );
            if (node) {
              observer.disconnect();
              requestAnimationFrame(() =>
                done({
                  stamp: Number(node.textContent.split(" ")[1]),
                  render: Date.now(),
                }),
              );
            }
          });
          observer.observe(document.getElementById("list"), {
            subtree: true,
            childList: true,
            characterData: true,
          });
        }),
    );
    await page.waitForTimeout(20);
    const commit = Number(
      execFileSync(
        "python3",
        [
          "-c",
          "import sqlite3,time,sys;c=sqlite3.connect(sys.argv[1]);c.execute('BEGIN IMMEDIATE');stamp=int(time.time()*1000);c.execute('UPDATE todos SET content=? WHERE id=?',(f'Latency {stamp}',sys.argv[2]));c.commit();print(int(time.time()*1000))",
          resolve(directory, "todos.db"),
          task.id,
        ],
        { encoding: "utf8" },
      ),
    );
    const result = await rendered;
    latencies.push(result.render - commit);
    cli("update", task.id, "--content", `Between measurements ${i}`);
    await expect(
      page
        .locator(".task-title")
        .filter({ hasText: `Between measurements ${i}` }),
    ).toBeVisible();
  }
  console.log(
    "Commit-to-animation-frame latency (ms):",
    JSON.stringify(latencies),
  );
  expect(Math.max(...latencies)).toBeLessThanOrEqual(500);
  await context.setOffline(true);
  await expect(page.locator("#connection")).toContainText("Reconnecting");
  cli("create", "--content", "Created while browser disconnected");
  await context.setOffline(false);
  await expect(
    page
      .locator(".task-title")
      .filter({ hasText: "Created while browser disconnected" }),
  ).toBeVisible({ timeout: 15000 });
});

test("phone hierarchy controls, all states, clearing fields, deletion, and themes", async ({
  browser,
}) => {
  const context = await browser.newContext({
    viewport: { width: 390, height: 844 },
    isMobile: true,
    hasTouch: true,
  });
  const page = await context.newPage();
  await page.goto(base);
  await createInBrowser(page, "Parent task");
  await page.getByRole("button", { name: "Parent task pending" }).tap();
  await expect(
    page.getByRole("dialog", { name: "Task details" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "+ Add child task" }).tap();
  await page
    .getByRole("textbox", { name: "Task", exact: true })
    .fill("Child task");
  await page.getByRole("button", { name: "Save task", exact: true }).tap();
  await expect(
    page.locator(".task-title").filter({ hasText: "Child task" }),
  ).toBeVisible();
  await page
    .getByRole("button", { name: "Collapse Parent task", exact: true })
    .tap();
  await expect(
    page.locator(".task-title").filter({ hasText: "Child task" }),
  ).toHaveCount(0);
  await page
    .getByRole("button", { name: "Expand Parent task", exact: true })
    .tap();
  await page.getByRole("button", { name: "Child task pending" }).tap();
  for (const state of ["*", "x", "?", "!", "-", " "]) {
    await page
      .getByRole("combobox", { name: "State", exact: true })
      .selectOption(state);
    await page.getByRole("button", { name: "Save task", exact: true }).tap();
    await expect
      .poll(() => cli("list").find((i) => i.content === "Child task").state)
      .toBe(state);
  }
  await page
    .getByRole("combobox", { name: "Priority", exact: true })
    .selectOption("P0");
  await page.getByLabel("Due date", { exact: true }).fill("2030-01-01");
  await page.getByRole("button", { name: "Save task", exact: true }).tap();
  await expect
    .poll(() => cli("list").find((i) => i.content === "Child task").due_date)
    .toBe("2030-01-01");
  await page
    .getByRole("combobox", { name: "Priority", exact: true })
    .selectOption("");
  await page.getByRole("button", { name: "Clear due date" }).tap();
  await page.getByRole("button", { name: "Save task", exact: true }).tap();
  await expect
    .poll(() => cli("list").find((i) => i.content === "Child task").priority)
    .toBeUndefined();
  await page
    .getByRole("combobox", { name: "Move under", exact: true })
    .selectOption("");
  await page
    .getByRole("button", { name: "Move task & descendants", exact: true })
    .tap();
  await expect
    .poll(
      () => cli("list").find((i) => i.content === "Child task").indent_level,
    )
    .toBe(0);
  await page
    .getByRole("button", { name: "Delete task & descendants", exact: true })
    .tap();
  await page.getByRole("button", { name: "Delete branch", exact: true }).tap();
  await expect(
    page.locator(".task-title").filter({ hasText: "Child task" }),
  ).toHaveCount(0);
  await expect
    .poll(() =>
      page.evaluate(() => document.documentElement.scrollWidth <= innerWidth),
    )
    .toBe(true);
  await page.getByRole("button", { name: "Switch color theme" }).tap();
  await page.screenshot({ path: "test-results/phone.png", fullPage: true });
  await page.setViewportSize({ width: 320, height: 700 });
  await expect
    .poll(() =>
      page.evaluate(() => document.documentElement.scrollWidth <= innerWidth),
    )
    .toBe(true);
  await page.evaluate(() => {
    document.documentElement.style.fontSize = "32px";
  });
  await expect
    .poll(() =>
      page.evaluate(() => document.documentElement.scrollWidth <= innerWidth),
    )
    .toBe(true);
  await context.close();
});

test("running TUI and browser exchange committed edits", async ({ page }) => {
  test.skip(process.platform === "win32", "PTY driver requires POSIX");
  const driver = spawn("python3", [resolve("tests/tui_driver.py"), binary], {
    env,
    stdio: ["ignore", "pipe", "pipe"],
  });
  let output = "",
    errors = "";
  driver.stdout.on("data", (chunk) => (output += chunk));
  driver.stderr.on("data", (chunk) => (errors += chunk));
  try {
    await expect
      .poll(() => output, { message: "TUI startup" })
      .toContain("READY");
    await page.goto(base);
    await expect(
      page.locator(".task-title").filter({ hasText: "TUI process task" }),
    ).toBeVisible();
    await createInBrowser(page, "Browser to TUI");
    await expect
      .poll(() => output + errors, {
        message: "TUI must render the browser edit",
        timeout: 16000,
      })
      .toContain("RENDERED");
    expect(errors).toBe("");
  } finally {
    if (driver.exitCode === null) driver.kill();
  }
});

test("project and historical selection resist late responses; nested filtering preserves context", async ({
  page,
}) => {
  const parent = cli("create", "--content", "Completed parent", "--state", "x");
  cli("create", "--content", "Open child", "--parent-id", parent.id);
  cli("create", "--content", "Historical task", "--date", "2025-01-01");
  execFileSync("python3", [
    "-c",
    "import sqlite3,sys,uuid;c=sqlite3.connect(sys.argv[1]);c.execute('INSERT INTO projects VALUES(?,?,?)',(str(uuid.uuid4()),'other','2026-01-01T00:00:00Z'));c.commit()",
    resolve(directory, "todos.db"),
  ]);
  cli("create", "--project", "other", "--content", "Other project task");
  await page.goto(base);
  await page.getByRole("checkbox", { name: "Hide completed" }).check();
  await expect(
    page.locator(".task-title").filter({ hasText: "Completed parent" }),
  ).toBeVisible();
  await expect(
    page.locator(".task-title").filter({ hasText: "Open child" }),
  ).toBeVisible();
  let captured;
  const capturedPromise = new Promise((done) => (captured = done));
  let delay = true;
  await page.route("**/api/snapshot?*", async (route) => {
    const response = await route.fetch();
    if (delay) {
      delay = false;
      captured();
      await new Promise((done) => setTimeout(done, 300));
    }
    await route.fulfill({ response });
  });
  await page.getByRole("button", { name: "Refresh tasks" }).click();
  await capturedPromise;
  await page.getByRole("button", { name: "other", exact: true }).click();
  await expect(page.locator(".task-title")).toHaveText(["Other project task"]);
  await page.waitForTimeout(350);
  await expect(page.locator(".task-title")).toHaveText(["Other project task"]);
  await page.getByRole("button", { name: "default", exact: true }).click();
  await page.getByLabel("Browse historical date").fill("2025-01-01");
  await expect(page.locator(".task-title")).toHaveText(["Historical task"]);
  await expect(
    page.getByRole("button", { name: "+ New task", exact: true }),
  ).toBeDisabled();
  cli("create", "--content", "New Today task");
  await page.waitForTimeout(250);
  await expect(page.locator(".task-title")).toHaveText(["Historical task"]);
});

test("reordering above the viewport preserves the visible task and active draft", async ({
  page,
}) => {
  let last;
  for (let i = 0; i < 30; i++)
    last = cli(
      "create",
      "--content",
      `Ordered task ${String(i).padStart(2, "0")}`,
    );
  await page.goto(base);
  await expect(page.locator(".task-row")).toHaveCount(30);
  await page.evaluate(() => {
    document.querySelector("main").scrollTop = 650;
  });
  const anchor = await page.evaluate(() => {
    const top = document.querySelector("main").getBoundingClientRect().top;
    const row = [...document.querySelectorAll(".task-row")].find(
      (row) => row.getBoundingClientRect().top >= top,
    );
    return { id: row.dataset.id, y: row.getBoundingClientRect().top };
  });
  await page.locator(`[data-id="${anchor.id}"] .task-open`).click();
  await page
    .getByRole("textbox", { name: "Description", exact: true })
    .fill("Draft survives movement above the viewport");
  execFileSync("python3", [
    "-c",
    "import sqlite3,sys;c=sqlite3.connect(sys.argv[1]);c.execute('BEGIN IMMEDIATE');c.execute('UPDATE todos SET position=position+1');c.execute('UPDATE todos SET position=0 WHERE id=?',(sys.argv[2],));c.commit()",
    resolve(directory, "todos.db"),
    last.id,
  ]);
  await expect(page.locator(".task-title").first()).toHaveText(
    "Ordered task 29",
  );
  const y = await page
    .locator(`[data-id="${anchor.id}"]`)
    .evaluate((row) => row.getBoundingClientRect().top);
  expect(Math.abs(y - anchor.y)).toBeLessThan(2);
  await expect(
    page.getByRole("textbox", { name: "Description", exact: true }),
  ).toHaveValue("Draft survives movement above the viewport");
  await expect(
    page.getByRole("textbox", { name: "Description", exact: true }),
  ).toBeFocused();
  await page.screenshot({ path: "test-results/desktop-light.png" });
  await page.getByRole("button", { name: "Switch color theme" }).click();
  await page.screenshot({ path: "test-results/desktop-dark.png" });
});

for (const phone of [false, true]) {
  test(`hierarchy round trip with descendants on ${phone ? "phone" : "desktop"}`, async ({
    browser,
  }) => {
    const context = await browser.newContext({
      viewport: phone
        ? { width: 390, height: 844 }
        : { width: 1280, height: 900 },
      isMobile: phone,
      hasTouch: phone,
    });
    const page = await context.newPage();
    const destination = cli("create", "--content", "Destination");
    cli("create", "--content", "Existing child", "--parent-id", destination.id);
    const branch = cli("create", "--content", "Moving branch");
    const descendant = cli(
      "create",
      "--content",
      "Moving descendant",
      "--parent-id",
      branch.id,
    );
    await page.goto(base);
    const activate = async (locator) =>
      phone ? locator.tap() : locator.click();
    await activate(
      page.getByRole("button", { name: "Collapse all", exact: true }),
    );
    await expect(page.locator(".task-row")).toHaveCount(2);
    await activate(
      page.getByRole("button", { name: "Expand all", exact: true }),
    );
    await expect(page.locator(".task-row")).toHaveCount(4);
    await activate(
      page.getByRole("button", { name: "Collapse Destination", exact: true }),
    );
    await activate(
      page.getByRole("button", { name: "Moving branch pending", exact: true }),
    );
    await expect(page.locator("#move-root")).toBeDisabled();
    await page
      .getByRole("combobox", { name: "Move under", exact: true })
      .selectOption(destination.id);
    await activate(page.locator("#move"));
    await expect
      .poll(() => cli("get", branch.id).parent_id)
      .toBe(destination.id);
    await expect.poll(() => cli("get", descendant.id).indent_level).toBe(2);
    await expect(page.locator("#move-root")).toBeEnabled();
    if (phone) await activate(page.locator("#close-editor"));
    await expect(
      page.locator(`.task-row[data-id="${descendant.id}"]`),
    ).toBeVisible();
    if (phone)
      await activate(
        page.getByRole("button", {
          name: "Moving branch pending",
          exact: true,
        }),
      );
    await activate(page.locator("#move-root"));
    await expect.poll(() => cli("get", branch.id).indent_level).toBe(0);
    await expect
      .poll(() => cli("get", descendant.id).parent_id)
      .toBe(branch.id);
    await expect.poll(() => cli("get", descendant.id).indent_level).toBe(1);
    await page
      .getByRole("combobox", { name: "Position", exact: true })
      .selectOption(destination.id);
    await activate(page.locator("#move"));
    await expect.poll(() => cli("list")[0].id).toBe(branch.id);
    await expect(page.locator("#move-root")).toBeDisabled();
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= innerWidth,
      ),
    ).toBe(true);
    await page.screenshot({
      path: `test-results/hierarchy-${phone ? "phone" : "desktop"}.png`,
    });
    await context.close();
  });
}

for (const mapped of [false, true]) {
  test(`startup selects ${mapped ? "folder mapping" : "last used project"} and honors URL selection`, async ({
    page,
  }) => {
    server.kill();
    await new Promise((done) => server.once("exit", done));
    execFileSync("python3", [
      "-c",
      "import sqlite3,sys,uuid;c=sqlite3.connect(sys.argv[1]);c.execute('INSERT INTO projects VALUES(?,?,?)',(str(uuid.uuid4()),'web-notes','2026-01-01T00:00:00Z'));c.commit()",
      resolve(directory, "todos.db"),
    ]);
    cli(
      "create",
      "--project",
      "web-notes",
      "--content",
      "Selected project task",
    );
    writeFileSync(
      resolve(directory, "config.toml"),
      mapped
        ? `last_used_project = "default"\n[folder_projects]\n${JSON.stringify(realpathSync(directory))} = "web-notes"\n`
        : 'last_used_project = "web-notes"\n',
    );
    server = spawn(binary, ["web", "--port", new URL(base).port], {
      env,
      cwd: directory,
      stdio: "pipe",
    });
    let output = "";
    server.stdout.on("data", (chunk) => (output += chunk));
    server.stderr.on("data", (chunk) => (output += chunk));
    await expect.poll(() => output).toContain("?project=web-notes");
    await page.goto(base);
    await expect(page.locator("#project-label")).toHaveText("WEB-NOTES");
    await expect(page.locator(".task-title")).toHaveText(
      "Selected project task",
    );
    await page.goto(`${base}/?project=default`);
    await expect(page.locator("#project-label")).toHaveText("DEFAULT");
    await page.getByRole("button", { name: "web-notes", exact: true }).click();
    await expect(page).toHaveURL(`${base}/?project=web-notes`);
    await page.reload();
    await expect(page.locator(".task-title")).toHaveText(
      "Selected project task",
    );
  });
}

test("Save task commits parent, position, and text together", async ({
  page,
}) => {
  const parent = cli("create", "--content", "Destination");
  const task = cli("create", "--content", "Move me");
  await page.goto(base);
  await page
    .getByRole("button", { name: "Move me pending", exact: true })
    .click();
  await page
    .getByRole("combobox", { name: "Move under", exact: true })
    .selectOption(parent.id);
  await page.locator("#content").fill("Moved and edited");
  await page.locator("#save").click();
  await expect.poll(() => cli("get", task.id).parent_id).toBe(parent.id);
  await expect.poll(() => cli("get", task.id).content).toBe("Moved and edited");
  await page
    .getByRole("combobox", { name: "Move under", exact: true })
    .selectOption("");
  await page
    .getByRole("combobox", { name: "Position", exact: true })
    .selectOption(parent.id);
  await page.locator("#save").click();
  await expect.poll(() => cli("list")[0].id).toBe(task.id);
  await expect.poll(() => cli("get", task.id).indent_level).toBe(0);
});

test("verbose payload logging and invalid combined save leaves all fields unchanged", async ({
  page,
}) => {
  server.kill();
  await new Promise((done) => server.once("exit", done));
  server = spawn(binary, ["web", "--verbose", "--port", new URL(base).port], {
    env,
    stdio: "pipe",
  });
  let output = "";
  server.stdout.on("data", (chunk) => (output += chunk));
  server.stderr.on("data", (chunk) => (output += chunk));
  await expect.poll(() => output).toContain("Workspace available");
  const task = cli("create", "--content", "Original content");
  const response = await page.request.patch(`${base}/api/todos/${task.id}`, {
    data: { content: "Payload marker", placement: { parent_id: task.id } },
  });
  expect(response.status()).toBe(400);
  expect(cli("get", task.id).content).toBe("Original content");
  expect(cli("get", task.id).indent_level).toBe(0);
  await expect.poll(() => output).toContain("Payload marker");
  await expect.poll(() => output).toContain("Mutation rejected");
});

for (const phone of [false, true]) {
  test(`drag reorders, nests, and unnests branches on ${phone ? "touch" : "desktop"}`, async ({
    browser,
  }) => {
    const context = await browser.newContext({
      viewport: phone
        ? { width: 390, height: 844 }
        : { width: 1280, height: 900 },
      isMobile: phone,
      hasTouch: phone,
    });
    const page = await context.newPage();
    const a = cli("create", "--content", "Branch A");
    const child = cli("create", "--content", "Child A", "--parent-id", a.id);
    const b = cli("create", "--content", "Branch B");
    const c = cli("create", "--content", "Branch C");
    await page.goto(base);
    await page
      .getByRole("button", { name: "Branch A pending", exact: true })
      .click();
    await page.locator("#description").fill("Draft stays while dragging");
    if (phone) await page.locator("#close-editor").click();
    const client = phone ? await context.newCDPSession(page) : null;
    async function pointer(type, x, y) {
      if (phone)
        await client.send("Input.dispatchTouchEvent", {
          type: {
            down: "touchStart",
            move: "touchMove",
            up: "touchEnd",
            cancel: "touchCancel",
          }[type],
          touchPoints:
            type === "up" || type === "cancel" ? [] : [{ x, y, id: 1 }],
        });
      else if (type === "down") {
        await page.mouse.move(x, y);
        await page.mouse.down();
      } else if (type === "up") await page.mouse.up();
      else await page.mouse.move(x, y, { steps: 6 });
    }
    async function drag(id, targetId, fraction, root = false) {
      const handle = page.locator(`[data-id="${id}"] .drag-handle`);
      await expect(handle).toBeEnabled();
      await handle.scrollIntoViewIfNeeded();
      const from = await handle.boundingBox();
      await pointer("down", from.x + 22, from.y + 22);
      await pointer("move", from.x + 12, from.y + 22);
      const to = await page
        .locator(root ? "#root-drop" : `[data-id="${targetId}"]`)
        .boundingBox();
      await pointer("move", to.x + to.width / 2, to.y + to.height * fraction);
      await expect(page.locator("#drag-feedback")).toBeVisible();
      await page.screenshot({
        path: `test-results/drag-${phone ? "phone" : "desktop"}.png`,
      });
      await pointer("up");
      await expect(page.locator("#drag-feedback")).toBeHidden();
    }
    await drag(a.id, b.id, 0.85);
    await expect
      .poll(() => cli("list").map((i) => i.id))
      .toEqual([b.id, a.id, child.id, c.id]);
    await drag(c.id, b.id, 0.5);
    await expect.poll(() => cli("get", c.id).parent_id).toBe(b.id);
    await drag(child.id, c.id, 0.15);
    await expect
      .poll(() => cli("list").map((i) => i.id))
      .toEqual([b.id, child.id, c.id, a.id]);
    await drag(c.id, null, 0.5, true);
    await expect.poll(() => cli("get", c.id).indent_level).toBe(0);
    if (phone)
      await page
        .getByRole("button", { name: "Branch A pending", exact: true })
        .click();
    await expect(page.locator("#description")).toHaveValue(
      "Draft stays while dragging",
    );
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= innerWidth,
      ),
    ).toBe(true);
    await context.close();
  });
}

test("drag rejects stale placement while keeping an active draft", async ({
  page,
}) => {
  const a = cli("create", "--content", "Drag A");
  const b = cli("create", "--content", "Drag B");
  await page.goto(base);
  await page
    .getByRole("button", { name: "Drag A pending", exact: true })
    .click();
  await page.locator("#description").fill("Do not lose this draft");
  const from = await page
    .locator(`[data-id="${a.id}"] .drag-handle`)
    .boundingBox();
  const to = await page.locator(`[data-id="${b.id}"]`).boundingBox();
  await page.mouse.move(from.x + 22, from.y + 22);
  await page.mouse.down();
  await page.mouse.move(to.x + to.width / 2, to.y + to.height / 2, {
    steps: 5,
  });
  cli("create", "--content", "External commit during drag");
  await page.mouse.up();
  await expect(page.locator("#error")).toContainText("Move was not saved");
  expect(cli("get", a.id).indent_level).toBe(0);
  await expect(page.locator("#description")).toHaveValue(
    "Do not lose this draft",
  );
  await expect(
    page
      .locator(".task-title")
      .filter({ hasText: "External commit during drag" }),
  ).toBeVisible();
});

test("touch drag autoscrolls and cancels without moving", async ({
  browser,
}) => {
  const context = await browser.newContext({
    viewport: { width: 390, height: 844 },
    isMobile: true,
    hasTouch: true,
  });
  const page = await context.newPage();
  const first = cli("create", "--content", "Scroll source");
  for (let i = 0; i < 22; i++) cli("create", "--content", `Scroll task ${i}`);
  await page.goto(base);
  const handle = page.locator(`[data-id="${first.id}"] .drag-handle`);
  await expect(handle).toBeVisible();
  const from = await handle.boundingBox();
  const client = await context.newCDPSession(page);
  await client.send("Input.dispatchTouchEvent", {
    type: "touchStart",
    touchPoints: [{ x: from.x + 22, y: from.y + 22, id: 1 }],
  });
  await client.send("Input.dispatchTouchEvent", {
    type: "touchMove",
    touchPoints: [{ x: from.x + 22, y: 820, id: 1 }],
  });
  await expect.poll(() => page.evaluate(() => scrollY)).toBeGreaterThan(100);
  await client.send("Input.dispatchTouchEvent", {
    type: "touchCancel",
    touchPoints: [],
  });
  await expect(page.locator("#drag-feedback")).toBeHidden();
  expect(cli("list")[0].id).toBe(first.id);
  await context.close();
});

test("all six task states have matching emoji labels", async ({ page }) => {
  const icons = {
    " ": "⬜",
    "*": "🔄",
    x: "✅",
    "?": "❔",
    "!": "❗",
    "-": "🚫",
  };
  for (const [state, icon] of Object.entries(icons))
    cli("create", "--content", `State ${icon}`, "--state", state);
  await page.goto(base);
  await expect(page.locator(".symbol")).toHaveText(Object.values(icons));
  for (const [state, icon] of Object.entries(icons)) {
    await expect(page.locator(`#state option[value="${state}"]`)).toContainText(
      icon,
    );
  }
});

for (const phone of [false, true]) {
  test(`completion checkbox saves and reopens tasks on ${phone ? "touch" : "keyboard"}`, async ({
    browser,
  }) => {
    const context = await browser.newContext({
      viewport: phone
        ? { width: 390, height: 844 }
        : { width: 1280, height: 900 },
      isMobile: phone,
      hasTouch: phone,
    });
    const page = await context.newPage();
    const task = cli("create", "--content", "Clickable task", "--state", "*");
    await page.goto(base);
    await page
      .getByRole("button", { name: "Clickable task in progress", exact: true })
      .click();
    await page.locator("#description").fill("Keep this draft");
    if (phone) await page.locator("#close-editor").click();
    const checkbox = page.getByRole("checkbox", {
      name: "Complete Clickable task",
      exact: true,
    });
    if (phone) await checkbox.tap();
    else {
      await checkbox.focus();
      await page.keyboard.press("Space");
    }
    await expect(checkbox).toBeChecked();
    await expect.poll(() => cli("get", task.id).state).toBe("x");
    const completedAt = () =>
      execFileSync(
        "python3",
        [
          "-c",
          "import sqlite3,sys;print(sqlite3.connect(sys.argv[1]).execute('SELECT completed_at FROM todos WHERE id=?',(sys.argv[2],)).fetchone()[0])",
          resolve(directory, "todos.db"),
          task.id,
        ],
        { encoding: "utf8" },
      ).trim();
    expect(completedAt()).not.toBe("None");
    await expect(checkbox).toBeEnabled();
    if (phone) await checkbox.tap();
    else await page.keyboard.press("Space");
    await expect(checkbox).not.toBeChecked();
    await expect.poll(() => cli("get", task.id).state).toBe(" ");
    expect(completedAt()).toBe("None");
    if (phone)
      await page
        .getByRole("button", { name: "Clickable task pending", exact: true })
        .click();
    await expect(page.locator("#description")).toHaveValue("Keep this draft");
    if (phone) await page.locator("#close-editor").click();
    await page.getByRole("checkbox", { name: "Hide completed" }).check();
    await checkbox.click();
    await expect(checkbox).toHaveCount(0);
    if (!phone) {
      cli("create", "--content", "Historical checkbox", "--date", "2025-01-01");
      await page.getByLabel("Browse historical date").fill("2025-01-01");
      await expect(
        page.getByRole("checkbox", { name: "Complete Historical checkbox" }),
      ).toBeDisabled();
    }
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= innerWidth,
      ),
    ).toBe(true);
    await page.screenshot({
      path: `test-results/checkbox-${phone ? "phone" : "desktop"}.png`,
    });
    await context.close();
  });
}

test("right-click state menu saves all states, preserves drafts, and supports keyboard dismissal", async ({
  page,
}) => {
  const task = cli("create", "--content", "Context task");
  await page.goto(base);
  await page
    .getByRole("button", { name: "Context task pending", exact: true })
    .click();
  await page.locator("#description").fill("Context menu draft");
  const row = page.locator(`[data-id="${task.id}"]`);
  const checkbox = row.locator(".symbol");
  const states = [
    ["*", "🔄 In progress"],
    ["?", "❔ Question"],
    ["!", "❗ Important"],
    ["-", "🚫 Cancelled"],
    ["x", "✅ Done"],
    [" ", "⬜ Pending"],
  ];
  for (const [state, label] of states) {
    await expect(checkbox).toBeEnabled();
    await (state === "*" ? checkbox : row.locator(".task-open")).click({
      button: "right",
    });
    await expect(
      page.getByRole("menu", { name: "State of Context task" }),
    ).toBeVisible();
    await expect(page.getByRole("menuitemradio")).toHaveCount(6);
    await page.getByRole("menuitemradio", { name: label, exact: true }).click();
    await expect(page.locator("#state-menu")).toBeHidden();
    await expect.poll(() => cli("get", task.id).state).toBe(state);
    await expect(page.locator("#description")).toHaveValue(
      "Context menu draft",
    );
  }
  await expect(checkbox).toBeEnabled();
  await checkbox.focus();
  await page.keyboard.press("Shift+F10");
  await expect(
    page.getByRole("menuitemradio", { name: "⬜ Pending" }),
  ).toBeFocused();
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("Enter");
  await expect.poll(() => cli("get", task.id).state).toBe("*");
  await expect(checkbox).toBeEnabled();
  await checkbox.click({ button: "right" });
  await page.keyboard.press("Escape");
  await expect(page.locator("#state-menu")).toBeHidden();
  await expect(checkbox).toBeFocused();
  await checkbox.click({ button: "right" });
  await page.locator("#heading").click();
  await expect(page.locator("#state-menu")).toBeHidden();
  await page.setViewportSize({ width: 390, height: 844 });
  await page.locator("#close-editor").click();
  await checkbox.click({ button: "right" });
  const box = await page.locator("#state-menu").boundingBox();
  expect(box.x + box.width).toBeLessThanOrEqual(390);
  expect(box.y + box.height).toBeLessThanOrEqual(844);
  await page.screenshot({ path: "test-results/state-menu-phone.png" });
  await page.keyboard.press("Escape");
  cli("create", "--content", "History menu", "--date", "2025-01-01");
  await page.getByLabel("Browse historical date").fill("2025-01-01");
  await page
    .getByRole("button", { name: "History menu pending", exact: true })
    .click({ button: "right" });
  await expect(page.locator("#state-menu")).toBeHidden();
});

test("state menu refuses a stale choice after an external edit", async ({
  page,
}) => {
  const task = cli("create", "--content", "Menu conflict");
  await page.goto(base);
  await page
    .getByRole("checkbox", { name: "Complete Menu conflict" })
    .click({ button: "right" });
  cli("update", task.id, "--state", "!");
  await expect(page.locator(".symbol")).toHaveText("❗");
  await page.getByRole("menuitemradio", { name: "✅ Done" }).click();
  await expect(page.locator("#error")).toContainText("Conflict");
  expect(cli("get", task.id).state).toBe("!");
});

test("compact rows show prominent priority badges without redundant state text", async ({
  page,
}) => {
  for (const priority of ["P0", "P1", "P2"])
    cli("create", "--content", `${priority} task`, "--priority", priority);
  cli("create", "--content", "No priority");
  await page.goto(base);
  await expect(page.locator(".priority-badge:visible")).toHaveText([
    "P0",
    "P1",
    "P2",
  ]);
  await expect(page.locator(".meta:visible")).toHaveCount(0);
  const row = page.locator(".task-row").first();
  expect((await row.boundingBox()).height).toBeLessThanOrEqual(48);
  expect(
    (await row.locator(".symbol").boundingBox()).height,
  ).toBeGreaterThanOrEqual(44);
  await page.screenshot({ path: "test-results/compact-light.png" });
  await page.locator("#theme").click();
  await page.screenshot({ path: "test-results/compact-dark.png" });
  await page.setViewportSize({ width: 390, height: 844 });
  await expect(page.locator(".priority-badge:visible")).toHaveText([
    "P0",
    "P1",
    "P2",
  ]);
  await page.screenshot({ path: "test-results/compact-phone.png" });
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
});

test("details panel takes space only while editing and keeps closed drafts", async ({
  page,
}) => {
  cli("create", "--content", "Panel task");
  await page.goto(base);
  await expect(page.locator("#details")).toBeHidden();
  const width = (await page.locator("main").boundingBox()).width;
  await page
    .getByRole("button", { name: "Panel task pending", exact: true })
    .click();
  await expect(page.locator("#details")).toBeVisible();
  expect((await page.locator("main").boundingBox()).width).toBeLessThan(width);
  await page.locator("#description").fill("Draft survives closing");
  await page.locator("#close-editor").click();
  await expect(page.locator("#details")).toBeHidden();
  expect((await page.locator("main").boundingBox()).width).toBe(width);
  await page
    .getByRole("button", { name: "Panel task pending", exact: true })
    .click();
  await expect(page.locator("#description")).toHaveValue(
    "Draft survives closing",
  );
  await page.locator("#close-editor").click();
  await page.screenshot({ path: "test-results/expanded-workspace.png" });
  await page.setViewportSize({ width: 390, height: 844 });
  await expect(page.locator("#details")).toBeHidden();
  await page
    .getByRole("button", { name: "Panel task pending", exact: true })
    .click();
  await expect(
    page.getByRole("dialog", { name: "Task details" }),
  ).toBeVisible();
  await expect(page.locator("#description")).toHaveValue(
    "Draft survives closing",
  );
  await page.locator("#close-editor").click();
  await expect(page.locator("#details")).toBeHidden();
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
});
