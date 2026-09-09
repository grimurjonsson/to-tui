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

const openMove = (page) =>
  page.locator("#task-actions").evaluate((details) => {
    details.open = true;
  });
async function createInBrowser(page, content) {
  await page.getByRole("button", { name: "New task", exact: true }).click();
  await page.getByRole("textbox", { name: "Task", exact: true }).fill(content);
  await page.getByRole("button", { name: "Save task", exact: true }).click();
  await expect(
    page.locator(".task-title").filter({ hasText: content }),
  ).toBeVisible();
}

test("j and k keep moving after closing the editor", async ({ page }) => {
  const tasks = ["First", "Second", "Third"].map((content) =>
    cli("create", "--content", content),
  );
  await page.goto(base);
  await page.locator(`[data-id="${tasks[0].id}"] .task-edit`).click();
  await page.locator("#close-editor").click();
  await page.keyboard.press("j");
  await page.keyboard.press("j");
  await expect(page.locator(`[data-id="${tasks[2].id}"]`)).toHaveClass(
    /active|cursor/,
  );
  await expect(page.locator("#details")).not.toBeVisible();
  await page.keyboard.press("k");
  await page.keyboard.press("k");
  await expect(
    page.locator(`[data-id="${tasks[0].id}"] .task-open`),
  ).toBeFocused();
  await expect(page.locator(".task-row.active,.task-row.cursor")).toHaveCount(
    1,
  );
});

test("j and k repeatedly move one highlighted task and follow the editor without losing drafts", async ({
  page,
}) => {
  const tasks = ["First", "Second", "Third", "Fourth"].map((content) =>
    cli("create", "--content", content),
  );
  await page.goto(base);
  await page.locator(`[data-id="${tasks[0].id}"] .task-edit`).click();
  await page.locator("#description").fill("Keep my draft");
  await page.locator(`[data-id="${tasks[0].id}"] .task-open`).focus();
  await page.locator(`[data-id="${tasks[0].id}"] .task-open`).hover();
  for (const [key, index] of [
    ["j", 1],
    ["j", 2],
    ["j", 3],
    ["k", 2],
    ["k", 1],
    ["k", 0],
  ]) {
    await page.keyboard.press(key);
    await expect(page.locator(".task-row.active,.task-row.cursor")).toHaveCount(
      1,
    );
    await expect(
      page.locator(".task-row.active,.task-row.cursor"),
    ).toHaveAttribute("data-id", tasks[index].id);
    await expect(
      page.locator(`[data-id="${tasks[index].id}"] .task-open`),
    ).toBeFocused();
    await expect(page.locator("#content")).toHaveValue(tasks[index].content);
    if (index !== 0)
      await expect(page.locator(`[data-id="${tasks[0].id}"]`)).toHaveCSS(
        "background-color",
        "rgba(0, 0, 0, 0)",
      );
  }
  await expect(page.locator("#description")).toHaveValue("Keep my draft");
  expect(cli("get", tasks[0].id).description).toBeUndefined();
});

test("copy task uses the requested format from keyboard and editor without saving", async ({
  page,
}) => {
  const task = cli(
    "create",
    "--content",
    "Plan café ☕",
    "--description",
    "First step\nSecond step",
  );
  await page.addInitScript(() => {
    window.copiedTasks = [];
    Object.defineProperty(navigator, "clipboard", {
      value: {
        writeText: async (value) => {
          window.copiedTasks.push(value);
        },
      },
    });
  });
  await page.goto(base);
  const row = page.locator(`[data-id="${task.id}"] .task-open`);
  await row.focus();
  await page.keyboard.press("y");
  await expect(page.locator("#list-copy-status")).toHaveText(
    "Copied to clipboard",
  );
  expect(await page.evaluate(() => window.copiedTasks)).toEqual([
    "Plan café ☕ - First step\nSecond step",
  ]);

  await page.locator(`[data-id="${task.id}"] .task-edit`).click();
  await page.locator("#content").fill("Edited title");
  await page.locator("#description").fill("Unsaved details");
  await page.getByRole("button", { name: "Copy task", exact: true }).click();
  await expect(page.locator("#copy-status")).toHaveText("Copied to clipboard");
  expect(await page.evaluate(() => window.copiedTasks.at(-1))).toBe(
    "Edited title - Unsaved details",
  );
  expect(cli("get", task.id).content).toBe("Plan café ☕");
  expect(cli("get", task.id).description).toBe("First step\nSecond step");

  for (const description of ["", "   \n  "]) {
    await page.locator("#description").fill(description);
    await page.locator("#close-editor").focus();
    await page.keyboard.press("y");
    expect(await page.evaluate(() => window.copiedTasks.at(-1))).toBe(
      "Edited title",
    );
  }
  const count = await page.evaluate(() => window.copiedTasks.length);
  await page.locator("#description").fill("");
  await page.keyboard.press("y");
  await expect(page.locator("#description")).toHaveValue("y");
  await page.locator("#close-editor").focus();
  await page.keyboard.press("Control+y");
  expect(await page.evaluate(() => window.copiedTasks.length)).toBe(count);
});

test("copy task reports denied and unavailable clipboard access without changing the task", async ({
  page,
}) => {
  const task = cli("create", "--content", "Keep this task");
  const before = cli("get", task.id);
  await page.addInitScript(() => {
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: {
        writeText: async () => {
          throw new DOMException("Denied", "NotAllowedError");
        },
      },
    });
  });
  await page.goto(base);
  await page.locator(`[data-id="${task.id}"] .task-edit`).click();
  await page.locator("#copy-task").click();
  await expect(page.locator("#copy-status")).toContainText("Could not copy");
  await page.evaluate(() =>
    Object.defineProperty(navigator, "clipboard", { value: undefined }),
  );
  await page.locator("#copy-task").click();
  await expect(page.locator("#copy-status")).toContainText(
    "Clipboard unavailable",
  );
  expect(cli("get", task.id)).toEqual(before);
});

test("drafts, conflict recovery, external writers, tabs, and measured rendering", async ({
  page,
  context,
}) => {
  const task = cli("create", "--content", "Plan release", "--state", "*");
  await page.goto(base);
  await page.getByRole("button", { name: "Edit Plan release" }).click();
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
  await page.getByRole("button", { name: "Edit Parent task" }).tap();
  await expect(
    page.getByRole("dialog", { name: "Task details" }),
  ).toBeVisible();
  await openMove(page);
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
  await page.getByRole("button", { name: "Edit Child task" }).tap();
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
  await openMove(page);
  await page
    .getByRole("combobox", { name: "Move under", exact: true })
    .selectOption("");
  await openMove(page);
  await page.getByRole("button", { name: "Move here", exact: true }).tap();
  await expect
    .poll(
      () => cli("list").find((i) => i.content === "Child task").indent_level,
    )
    .toBe(0);
  await page
    .getByRole("button", { name: "Delete task and its children", exact: true })
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
    page.getByRole("button", { name: "New task", exact: true }),
  ).toBeHidden();
  cli("create", "--content", "New Today task");
  await page.waitForTimeout(250);
  await expect(page.locator(".task-title")).toHaveText(["Historical task"]);
});

test("reordering above the viewport preserves the visible task and active draft", async ({
  page,
}) => {
  let last;
  for (let i = 0; i < 45; i++)
    last = cli(
      "create",
      "--content",
      `Ordered task ${String(i).padStart(2, "0")}`,
    );
  await page.goto(base);
  await expect(page.locator(".task-row")).toHaveCount(45);
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
  await page.locator(`[data-id="${anchor.id}"] .task-edit`).click();
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
    "Ordered task 44",
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
    await activate(page.getByRole("button", { name: "Fold all", exact: true }));
    await expect(page.locator(".task-row")).toHaveCount(2);
    await activate(
      page.getByRole("button", { name: "Unfold all", exact: true }),
    );
    await expect(page.locator(".task-row")).toHaveCount(4);
    await activate(
      page.getByRole("button", { name: "Collapse Destination", exact: true }),
    );
    await activate(
      page.getByRole("button", { name: "Edit Moving branch", exact: true }),
    );
    await expect(page.locator("#move-root")).toBeDisabled();
    await openMove(page);
    await page
      .getByRole("combobox", { name: "Move under", exact: true })
      .selectOption(destination.id);
    await openMove(page);
    await activate(page.locator("#move"));
    await expect
      .poll(() => cli("get", branch.id).parent_id)
      .toBe(destination.id);
    await expect.poll(() => cli("get", descendant.id).indent_level).toBe(2);
    await openMove(page);
    await expect(page.locator("#move-root")).toBeEnabled();
    if (phone) await activate(page.locator("#close-editor"));
    await expect(
      page.locator(`.task-row[data-id="${descendant.id}"]`),
    ).toBeVisible();
    if (phone)
      await activate(
        page.getByRole("button", {
          name: "Edit Moving branch",
          exact: true,
        }),
      );
    await openMove(page);
    await activate(page.locator("#move-root"));
    await expect.poll(() => cli("get", branch.id).indent_level).toBe(0);
    await expect
      .poll(() => cli("get", descendant.id).parent_id)
      .toBe(branch.id);
    await expect.poll(() => cli("get", descendant.id).indent_level).toBe(1);
    await openMove(page);
    await page
      .getByRole("combobox", { name: "Position", exact: true })
      .selectOption(destination.id);
    await openMove(page);
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
    await expect(page.locator("#project-label")).toHaveText("web-notes");
    await expect(page.locator(".task-title")).toHaveText(
      "Selected project task",
    );
    await page.goto(`${base}/?project=default`);
    await expect(page.locator("#project-label")).toHaveText("default");
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
  await page.getByRole("button", { name: "Edit Move me", exact: true }).click();
  await openMove(page);
  await page
    .getByRole("combobox", { name: "Move under", exact: true })
    .selectOption(parent.id);
  await page.locator("#content").fill("Moved and edited");
  await page.locator("#save").click();
  await expect.poll(() => cli("get", task.id).parent_id).toBe(parent.id);
  await expect.poll(() => cli("get", task.id).content).toBe("Moved and edited");
  await openMove(page);
  await page
    .getByRole("combobox", { name: "Move under", exact: true })
    .selectOption("");
  await openMove(page);
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
      .getByRole("button", { name: "Edit Branch A", exact: true })
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
        .getByRole("button", { name: "Edit Branch A", exact: true })
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
  await page.getByRole("button", { name: "Edit Drag A", exact: true }).click();
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

test("all six task states have matching glyph labels", async ({ page }) => {
  const icons = {
    " ": "[ ]",
    "*": "[*]",
    x: "[x]",
    "?": "[?]",
    "!": "[!]",
    "-": "[-]",
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
      .getByRole("button", { name: "Edit Clickable task", exact: true })
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
        .getByRole("button", { name: "Edit Clickable task", exact: true })
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
    .getByRole("button", { name: "Edit Context task", exact: true })
    .click();
  await page.locator("#description").fill("Context menu draft");
  const row = page.locator(`[data-id="${task.id}"]`);
  const checkbox = row.locator(".symbol");
  const states = [
    ["*", "[*] In progress"],
    ["?", "[?] Question"],
    ["!", "[!] Important"],
    ["-", "[-] Cancelled"],
    ["x", "[x] Done"],
    [" ", "[ ] Pending"],
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
    page.getByRole("menuitemradio", { name: "[ ] Pending" }),
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
    .getByRole("button", { name: "Edit History menu", exact: true })
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
  await expect(page.locator(".symbol")).toHaveText("[!]");
  await page.getByRole("menuitemradio", { name: "[x] Done" }).click();
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
    .getByRole("button", { name: "Edit Panel task", exact: true })
    .click();
  await expect(page.locator("#details")).toBeVisible();
  expect((await page.locator("main").boundingBox()).width).toBeLessThan(width);
  await page.locator("#description").fill("Draft survives closing");
  await page.locator("#close-editor").click();
  await expect(page.locator("#details")).toBeHidden();
  expect((await page.locator("main").boundingBox()).width).toBe(width);
  await page
    .getByRole("button", { name: "Edit Panel task", exact: true })
    .click();
  await expect(page.locator("#description")).toHaveValue(
    "Draft survives closing",
  );
  await page.locator("#close-editor").click();
  await page.screenshot({ path: "test-results/expanded-workspace.png" });
  await page.setViewportSize({ width: 390, height: 844 });
  await expect(page.locator("#details")).toBeHidden();
  await page
    .getByRole("button", { name: "Edit Panel task", exact: true })
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

test("project CRUD preserves tasks on rename and removes them on delete", async ({
  page,
  context,
}) => {
  await page.goto(base);
  await expect(page.locator("#add")).toBeEnabled();
  await page.getByRole("button", { name: "Open account menu" }).click();
  await page.locator("#manage-projects").click();
  await expect(page.locator("#rename-project")).toBeDisabled();
  await expect(page.locator("#delete-project")).toBeDisabled();
  await page.locator("#new-project").click();
  await expect(
    page.getByRole("dialog", { name: "New project", exact: true }),
  ).toBeVisible();
  await expect(page.locator("#manage-project-picker")).toBeHidden();
  await expect(page.locator("#rename-project")).toBeHidden();
  await expect(page.locator("#project-name")).toBeFocused();
  await page.screenshot({ path: "test-results/new-project-desktop.png" });
  await page.locator("#cancel-project-edit").click();
  await expect(page.locator("#manage-project-picker")).toBeVisible();
  await page.locator("#new-project").click();
  await page.locator("#project-name").fill("Work");
  await page.locator("#save-project").click();
  await expect(page.locator("#project-label")).toHaveText("Work");
  await createInBrowser(page, "Keep through rename");
  const tab = await context.newPage();
  await tab.goto(`${base}/?project=Work`);
  await expect(tab.locator(".task-title")).toHaveText("Keep through rename");
  await page.getByRole("button", { name: "Open account menu" }).click();
  await page.locator("#manage-projects").click();
  await page.locator("#rename-project").click();
  await page.locator("#project-name").fill("default");
  await page.locator("#save-project").click();
  await expect(page.locator("#project-error")).toContainText("already exists");
  await expect(page.locator("#project-name")).toHaveValue("default");
  await page.locator("#project-name").fill("Work & plans");
  await page.screenshot({ path: "test-results/project-manager-desktop.png" });
  await page.locator("#save-project").click();
  await expect(page.locator("#project-label")).toHaveText("Work & plans");
  await expect(page.locator(".task-title")).toHaveText("Keep through rename");
  await expect(tab.locator("#project-label")).toHaveText("Work & plans");
  await expect(tab.locator(".task-title")).toHaveText("Keep through rename");
  await page.setViewportSize({ width: 390, height: 844 });
  await page.getByRole("button", { name: "Open account menu" }).click();
  await page.locator("#manage-projects").click();
  await page.locator("#delete-project").click();
  await expect(page.locator("#project-delete-message")).toContainText(
    "all its tasks, history",
  );
  await page.screenshot({ path: "test-results/project-manager-mobile.png" });
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
  await page.locator("#cancel-project-edit").click();
  await expect(page.locator("#project-form")).toBeHidden();
  await page.locator("#delete-project").click();
  await page.locator("#save-project").click();
  await expect(page.locator("#project-label")).toHaveText("default");
  await expect(tab.locator("#project-label")).toHaveText("default");
  await page.getByRole("button", { name: "Open account menu" }).click();
  await page.locator("#manage-projects").click();
  await page.locator("#new-project").click();
  await page.locator("#project-name").fill("Work & plans");
  await page.locator("#save-project").click();
  await expect(page.locator("#project-label")).toHaveText("Work & plans");
  await expect(page.locator(".task-title")).toHaveCount(0);
});

test("project API validates names and protects default", async () => {
  const request = (path, method, name) =>
    fetch(`${base}/api/projects${path}`, {
      method,
      headers: { "Content-Type": "application/json" },
      ...(name === undefined ? {} : { body: JSON.stringify({ name }) }),
    });
  for (const name of [
    "   ",
    "../outside",
    "a/b",
    "a\\b",
    ".",
    "..",
    "a\nname",
  ]) {
    expect((await request("", "POST", name)).status).toBe(400);
  }
  const projects = await (await fetch(`${base}/api/projects`)).json();
  const id = projects.projects.find((project) => project.name === "default").id;
  expect((await request(`/${id}`, "PATCH", "Other")).status).toBe(400);
  expect((await request(`/${id}`, "DELETE")).status).toBe(400);
  expect((await request("", "POST", "default")).status).toBe(409);
  expect(
    (await request("/00000000-0000-0000-0000-000000000000", "DELETE")).status,
  ).toBe(404);
});

test("project lifecycle moves history and bindings and cleans stored data", async () => {
  const request = (path, method, body) =>
    fetch(`${base}${path}`, {
      method,
      headers: { "Content-Type": "application/json" },
      ...(body ? { body: JSON.stringify(body) } : {}),
    });
  const created = await request("/api/projects", "POST", { name: "History" });
  expect(created.status).toBe(201);
  const project = await created.json();
  expect(
    (
      await request("/api/todos?project=History", "POST", {
        content: "Stored task",
      })
    ).status,
  ).toBe(201);
  writeFileSync(
    resolve(directory, "config.toml"),
    'last_used_project = "History"\n[folder_projects]\n"/test/repo" = "History"\n',
  );
  const db = (code) =>
    JSON.parse(
      execFileSync(
        "python3",
        [
          "-c",
          `
import sqlite3, json, os
from pathlib import Path
conn = sqlite3.connect(os.path.join(os.environ['TOTUI_DATA_DIR'], 'todos.db'))
${code}
`,
        ],
        { env, encoding: "utf8" },
      ),
    );
  db(`
conn.execute("INSERT INTO archived_todos (id, original_date, archived_at, content, state, indent_level, position, created_at, updated_at, project) SELECT id, date, created_at, content, state, indent_level, position, created_at, updated_at, project FROM todos WHERE project='History'")
conn.execute("INSERT INTO todo_metadata (id, todo_id, plugin_name, created_at, updated_at) SELECT 'metadata', id, 'test', created_at, updated_at FROM todos WHERE project='History'")
conn.execute("INSERT INTO project_metadata (id, project_name, plugin_name, created_at, updated_at) VALUES ('project-meta', 'History', 'test', '', '')")
conn.commit()
print('null')
`);
  expect(
    (await request(`/api/projects/${project.id}`, "PATCH", { name: "Renamed" }))
      .status,
  ).toBe(200);
  expect(
    db(`
print(json.dumps([
conn.execute("SELECT project FROM archived_todos").fetchone()[0],
conn.execute("SELECT project_name FROM project_metadata").fetchone()[0],
conn.execute("SELECT COUNT(*) FROM list_revisions WHERE project='History'").fetchone()[0],
Path(os.environ['TOTUI_DATA_DIR'], 'projects', 'Renamed').is_dir(),
'last_used_project = "Renamed"' in Path(os.environ['TOTUI_DATA_DIR'], 'config.toml').read_text(),
]))
`),
  ).toEqual(["Renamed", "Renamed", 0, true, true]);
  expect((await request(`/api/projects/${project.id}`, "DELETE")).status).toBe(
    204,
  );
  expect(
    db(`
print(json.dumps([
*[conn.execute('SELECT COUNT(*) FROM ' + table).fetchone()[0] for table in ['todos', 'archived_todos', 'todo_metadata', 'project_metadata']],
Path(os.environ['TOTUI_DATA_DIR'], 'projects', 'Renamed').exists(),
'Renamed' in Path(os.environ['TOTUI_DATA_DIR'], 'config.toml').read_text(),
]))
`),
  ).toEqual([0, 0, 0, 0, false, false]);
});

test("an account switch reloads before using another user's response", async ({
  page,
}) => {
  cli("create", "--content", "Account switch task");
  await page.goto(base);
  await expect(page.locator(".task-title")).toHaveText("Account switch task");
  await page.locator(".task-edit").click();
  await page
    .getByRole("textbox", { name: "Description", exact: true })
    .fill("Unsaved private draft");
  await page.route(
    "**/api/snapshot?*",
    async (route) => {
      await route.fulfill({
        status: 409,
        headers: { "X-Totui-User": "another-account" },
        body: "Account changed",
      });
    },
    { times: 1 },
  );
  await Promise.all([
    page.waitForEvent("domcontentloaded"),
    page.getByRole("button", { name: "Refresh tasks" }).click(),
  ]);
  await expect(page.locator(".task-title")).toHaveText("Account switch task");
  await expect(page.locator("#editor")).toBeHidden();
  await expect(page.getByText("Unsaved private draft")).toHaveCount(0);
});

test("server account, owner upgrade confirmation, and logout", async ({
  page,
}) => {
  await page.emulateMedia({ colorScheme: "dark" });
  let upgradeCalls = 0;
  await page.route("**/api/server", (route) =>
    route.fulfill({
      json: {
        version: "0.7.0",
        user: { id: "owner", email: "owner@example.test" },
        logout_url: "/oauth2/sign_out?rd=%2Fsigned-out",
        can_upgrade: true,
        latest_version: "0.8.0",
        update_available: true,
        upgrade_pending: false,
        upgrade_status: {
          state: "complete",
          message: "Server v0.7.0 is already up to date.",
        },
      },
    }),
  );
  await page.route("**/api/server/upgrade", async (route) => {
    expect(route.request().postDataJSON()).toEqual({ version: "0.8.0" });
    upgradeCalls++;
    await route.fulfill({ status: 202, json: { status: "queued" } });
  });
  await page.goto(base);
  await expect(page.locator("#server-version")).toHaveText("v0.7.0");
  await expect(page.locator("#current-user")).toHaveText("owner@example.test");
  await expect(page.locator("#server-message")).toBeHidden();
  await expect(page.locator(".sidebar #current-user")).toBeVisible();
  await page.screenshot({ path: "/tmp/totui-sidebar-desktop.png" });
  page.once("dialog", (dialog) => dialog.dismiss());
  await page.locator("#server-upgrade").click();
  expect(upgradeCalls).toBe(0);
  page.once("dialog", (dialog) => dialog.accept());
  await page.locator("#server-upgrade").click();
  await expect(page.locator("#server-message")).toContainText("Upgrade queued");
  expect(upgradeCalls).toBe(1);
  await page.route("**/oauth2/sign_out?*", (route) =>
    route.fulfill({ body: "Signed out" }),
  );
  await expect(page.getByRole("menuitem", { name: "Log out" })).toBeHidden();
  await page.getByRole("button", { name: "Open account menu" }).click();
  await expect(
    page.getByRole("menuitem", { name: "Manage projects" }),
  ).toBeFocused();
  await page.screenshot({ path: "/tmp/totui-compact-account-menu.png" });
  await page.keyboard.press("End");
  await expect(page.getByRole("menuitem", { name: "Log out" })).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("menuitem", { name: "Log out" })).toBeHidden();
  await expect(
    page.getByRole("button", { name: "Open account menu" }),
  ).toBeFocused();
  await page.keyboard.press("Enter");
  await page.getByRole("menuitem", { name: "Log out", exact: true }).click();
  await expect(page).toHaveURL(/oauth2\/sign_out/);
});

test("nonowners see the update indicator without upgrade access on mobile", async ({
  page,
}) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.route("**/api/server", (route) =>
    route.fulfill({
      json: {
        version: "0.7.0",
        user: { id: "member", email: "member@example.test" },
        logout_url: "/oauth2/sign_out",
        can_upgrade: false,
        latest_version: "0.8.0",
        update_available: true,
      },
    }),
  );
  await page.goto(base);
  await expect(page.locator("#server-upgrade")).toHaveText("Upgrade to v0.8.0");
  await expect(page.locator("#server-upgrade")).toBeDisabled();
  await expect(page.locator("#current-user")).toBeVisible();
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
  await page.locator("#current-user").scrollIntoViewIfNeeded();
  await page.getByRole("button", { name: "Open account menu" }).click();
  await expect(
    page.getByRole("menu", { name: "Account", exact: true }),
  ).toBeVisible();
  const menu = await page.locator("#account-menu").boundingBox();
  expect(menu.y).toBeGreaterThanOrEqual(0);
  expect(menu.y + menu.height).toBeLessThanOrEqual(844);
  await page.screenshot({ path: "/tmp/totui-account-menu-mobile.png" });
  await page.locator("#server-version").click();
  await expect(page.locator("#account-menu")).toBeHidden();
});

test("task scrolling keeps the sidebar fixed and overflowing projects scroll separately", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1280, height: 720 });
  for (let index = 0; index < 35; index++) {
    const task = await fetch(`${base}/api/todos`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ content: `Scrolling task ${index}` }),
    });
    expect(task.ok).toBe(true);
    const project = await fetch(`${base}/api/projects`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ name: `project-${index}` }),
    });
    expect(project.ok).toBe(true);
  }
  await page.route("**/api/server", (route) =>
    route.fulfill({
      json: {
        version: "0.7.0",
        user: { id: "member", email: "member@example.test" },
        logout_url: "/oauth2/sign_out",
        can_upgrade: false,
        update_available: false,
      },
    }),
  );
  await page.goto(base);
  await expect(page.locator("#current-user")).toHaveText("member@example.test");
  await expect(page.locator(".task-row")).toHaveCount(35);
  const sidebar = await page.locator(".sidebar").boundingBox();
  const account = await page.locator(".server-account").boundingBox();
  expect(account.y + account.height).toBeLessThanOrEqual(720);
  await page.locator("main").hover();
  await page.mouse.wheel(0, 650);
  await expect
    .poll(() => page.locator("main").evaluate((element) => element.scrollTop))
    .toBeGreaterThan(0);
  expect(await page.evaluate(() => window.scrollY)).toBe(0);
  expect(await page.locator(".sidebar").boundingBox()).toEqual(sidebar);
  expect(await page.locator(".server-account").boundingBox()).toEqual(account);
  const mainScroll = await page
    .locator("main")
    .evaluate((element) => element.scrollTop);
  await page.locator("#projects").hover();
  await page.mouse.wheel(0, 1500);
  await expect
    .poll(() =>
      page.locator("#projects").evaluate((element) => element.scrollTop),
    )
    .toBeGreaterThan(0);
  expect(
    await page.locator("main").evaluate((element) => element.scrollTop),
  ).toBe(mainScroll);
  expect(await page.locator(".server-account").boundingBox()).toEqual(account);
  expect(await page.evaluate(() => document.documentElement.scrollHeight)).toBe(
    720,
  );
});

for (const found of [true, false]) {
  test(`account avatar ${found ? "shows Gravatar" : "falls back to initials when unavailable"}`, async ({
    page,
  }) => {
    const avatarUrl = "https://gravatar.com/avatar/test-user?s=80&d=404&r=g";
    await page.route("**/api/server", (route) =>
      route.fulfill({
        json: {
          version: "0.7.0",
          user: { id: "member", email: "test.user@example.test" },
          avatar_url: avatarUrl,
          logout_url: "/oauth2/sign_out",
          can_upgrade: false,
          update_available: false,
        },
      }),
    );
    await page.route("https://gravatar.com/avatar/**", (route) => {
      expect(route.request().headers().referer).toBeUndefined();
      return route.fulfill(
        found
          ? {
              contentType: "image/png",
              body: Buffer.from(
                "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jRZkAAAAASUVORK5CYII=",
                "base64",
              ),
            }
          : { status: 404 },
      );
    });
    const avatarResponse = page.waitForResponse(avatarUrl);
    await page.goto(base);
    await avatarResponse;
    await expect(page.locator("#current-user")).toHaveText(
      "test.user@example.test",
    );
    await expect(page.locator("#avatar-initials")).toHaveText("TU");
    if (found) {
      await expect(page.locator("#user-avatar")).toBeVisible();
      await expect(page.locator("#avatar-initials")).toBeHidden();
    } else {
      await expect(page.locator("#user-avatar")).toBeHidden();
      await expect(page.locator("#avatar-initials")).toBeVisible();
    }
  });
}

test("email opens the account menu and only a confirmed current version gets a checkmark", async ({
  page,
}) => {
  let release = { latest_version: "0.7.0", update_available: false };
  await page.route("**/api/server", (route) =>
    route.fulfill({
      json: {
        version: "0.7.0",
        user: { id: "member", email: "member@example.test" },
        logout_url: "/oauth2/sign_out",
        can_upgrade: false,
        ...release,
      },
    }),
  );
  await page.goto(base);
  await expect(page.locator("#server-current")).toBeVisible();
  await page
    .getByRole("button", { name: "member@example.test", exact: true })
    .click();
  await expect(
    page.getByRole("menuitem", { name: "Manage projects" }),
  ).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.locator("#account-menu")).toBeHidden();
  await expect(page.locator("#current-user")).toBeFocused();
  for (const next of [
    { latest_version: "0.8.0", update_available: true },
    {
      latest_version: null,
      update_available: false,
      check_error: "Could not check",
    },
    { latest_version: "0.7.0", update_available: false, upgrade_pending: true },
  ]) {
    release = next;
    await page.reload();
    await expect(page.locator("#current-user")).toHaveText(
      "member@example.test",
    );
    await expect(page.locator("#server-current")).toBeHidden();
  }
});

for (const phone of [false, true]) {
  test(`inline rename saves and cancels without a sidebar (${phone ? "phone" : "desktop"})`, async ({
    page,
  }) => {
    if (phone) await page.setViewportSize({ width: 390, height: 844 });
    const task = cli(
      "create",
      "--content",
      "Quick task",
      "--description",
      "Keep details",
    );
    await page.goto(base);
    const row = page.locator(`[data-id="${task.id}"]`);
    await row.locator(".task-open").click();
    await expect(page.locator("#details")).not.toBeVisible();
    const input = row.getByRole("textbox", { name: "Task name", exact: true });
    await expect(input).toBeFocused();
    expect(
      await input.evaluate((el) => [el.selectionStart, el.selectionEnd]),
    ).toEqual(["Quick task".length, "Quick task".length]);
    await input.fill("Renamed inline");
    await page.screenshot({
      path: `test-results/inline-${phone ? "phone" : "desktop"}.png`,
    });
    await input.press("Enter");
    await expect(row.locator(".task-title")).toHaveText("Renamed inline");
    expect(cli("get", task.id).description).toBe("Keep details");
    await row.locator(".task-open").click();
    await input.fill("Discard this");
    await input.press("Escape");
    await expect(row.locator(".task-title")).toHaveText("Renamed inline");
    await row.locator(".task-edit").click();
    await expect(page.locator("#content")).toHaveValue("Renamed inline");
    await page.locator("#close-editor").click();
    await row.click({ button: "right" });
    await page
      .getByRole("menuitem", { name: "Edit task", exact: true })
      .click();
    await expect(page.locator("#content")).toHaveValue("Renamed inline");
  });
}

test("inline rename retains text on conflict without overwriting newer edits", async ({
  page,
}) => {
  const task = cli("create", "--content", "Original");
  await page.goto(base);
  const row = page.locator(`[data-id="${task.id}"]`);
  await row.locator(".task-open").click();
  const input = row.getByRole("textbox", { name: "Task name", exact: true });
  await input.fill("My draft");
  cli("update", task.id, "--content", "Changed elsewhere");
  await input.press("Enter");
  await expect(row.getByRole("status")).toContainText("Not saved");
  await expect(input).toHaveValue("My draft");
  expect(cli("get", task.id).content).toBe("Changed elsewhere");
  await expect(page.locator("#details")).not.toBeVisible();
});

for (const phone of [false, true]) {
  test(`inline editor fits existing text and grows and shrinks (${phone ? "phone" : "desktop"})`, async ({
    page,
  }) => {
    if (phone) await page.setViewportSize({ width: 390, height: 844 });
    const content =
      "A long task with plenty of detail to keep visible while editing. "
        .repeat(5)
        .trim();
    const task = cli("create", "--content", content);
    await page.goto(base);
    await page.locator(`[data-id="${task.id}"] .task-open`).click();
    const input = page.locator(".inline-edit textarea");
    const fits = () =>
      input.evaluate((el) => el.scrollHeight <= el.clientHeight + 1);
    await expect.poll(fits).toBe(true);
    const initial = await input.evaluate((el) => el.clientHeight);
    expect(initial).toBeGreaterThan(40);
    await input.fill(content.repeat(3));
    await expect
      .poll(() => input.evaluate((el) => el.clientHeight))
      .toBeGreaterThan(initial);
    await expect.poll(fits).toBe(true);
    await input.fill("Short task");
    await expect
      .poll(() => input.evaluate((el) => el.clientHeight))
      .toBeLessThan(initial);
    await input.fill(content);
    await page.setViewportSize({ width: phone ? 600 : 700, height: 900 });
    await expect.poll(fits).toBe(true);
    await page.screenshot({
      path: `test-results/inline-growing-${phone ? "phone" : "desktop"}.png`,
    });
    await input.press("Enter");
    await expect(input).toHaveCount(0);
    expect(cli("get", task.id).content).toBe(content);
  });
}
