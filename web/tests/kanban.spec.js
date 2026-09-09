import { test, expect } from "@playwright/test";
import { spawn, execFileSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { createServer } from "node:net";
let directory, server, base, env;
const binary = resolve("../target/debug/totui");
const action = (fields) =>
  JSON.parse(
    execFileSync(
      binary,
      [
        "--local",
        "kanban",
        "--json",
        JSON.stringify({ project: "default", actor: "test", ...fields }),
      ],
      { env, encoding: "utf8" },
    ),
  );
test.beforeEach(async () => {
  directory = mkdtempSync(resolve(tmpdir(), "totui-kanban-drag-"));
  env = { ...process.env, TOTUI_DATA_DIR: directory };
  const socket = createServer();
  await new Promise((done) => socket.listen(0, "127.0.0.1", done));
  const port = socket.address().port;
  await new Promise((done) => socket.close(done));
  base = `http://127.0.0.1:${port}`;
  action({ action: "create_board", name: "Drag test" });
  action({
    action: "create_ticket",
    title: "Drag me",
    description: "Keep details",
  });
  server = spawn(binary, ["--local", "web", "--port", String(port)], {
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
  if (server?.exitCode === null) {
    server.kill();
    await new Promise((done) => server.once("exit", done));
  }
  rmSync(directory, { recursive: true, force: true });
});
const card = (page) =>
  page.locator(".kanban-card,.kanban-row").filter({ hasText: "Drag me" });
const column = (page, status) => page.locator(`[data-status="${status}"]`);
test("copy ticket supports focused cards and editor drafts without changing the board", async ({
  page,
}) => {
  const before = action({ action: "view" });
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
  await page.goto(`${base}/kanban`);
  await card(page).locator(".kanban-card-open").focus();
  await page.keyboard.press("y");
  await expect(page.locator("#board-copy-status")).toHaveText(
    "Copied to clipboard",
  );
  expect(await page.evaluate(() => window.copiedTasks)).toEqual([
    "Drag me - Keep details",
  ]);
  await card(page).locator(".kanban-card-open").click();
  await page.locator("#title").fill("Revised ticket");
  await page.locator("#description").fill("Details\nwith Unicode ✓");
  await page.locator("#copy-ticket").click();
  await expect(page.locator("#copy-status")).toHaveText("Copied to clipboard");
  expect(await page.evaluate(() => window.copiedTasks.at(-1))).toBe(
    "Revised ticket - Details\nwith Unicode ✓",
  );
  await page.locator("#description").fill("");
  await page.locator("#close").focus();
  await page.keyboard.press("y");
  expect(await page.evaluate(() => window.copiedTasks.at(-1))).toBe(
    "Revised ticket",
  );
  await page.locator("#description").focus();
  await page.keyboard.press("y");
  await expect(page.locator("#description")).toHaveValue("y");
  expect(await page.evaluate(() => window.copiedTasks.length)).toBe(3);
  expect(action({ action: "view" })).toEqual(before);
});
test("drag moves persist and backward drops require a reason", async ({
  page,
}) => {
  await page.goto(`${base}/kanban`);
  await card(page).dragTo(column(page, "ready"));
  await expect(column(page, "ready").locator(".kanban-card")).toHaveCount(1);
  expect(action({ action: "view" }).tickets[0].status).toBe("ready");
  await expect(page.locator("#editor")).not.toBeVisible();
  await card(page).dragTo(column(page, "backlog"));
  await expect(page.locator("#editor")).toBeVisible();
  await expect(page.locator("#status")).toHaveValue("backlog");
  expect(action({ action: "view" }).tickets[0].status).toBe("ready");
  await page.locator("#move").click();
  await expect(page.locator("#form-error")).toBeVisible();
  await page.locator("#reason").fill("Needs more design");
  await page.locator("#move").click();
  await expect
    .poll(() => action({ action: "view" }).tickets[0].status)
    .toBe("backlog");
  expect(action({ action: "view" }).tickets[0].feedback).toBe(
    "Needs more design",
  );
});
test("a concurrent comment during drag is preserved and rejects the stale move", async ({
  page,
}) => {
  await page.goto(`${base}/kanban`);
  await expect(card(page)).toBeVisible();
  const transfer = await page.evaluateHandle(() => new DataTransfer());
  await card(page).dispatchEvent("dragstart", { dataTransfer: transfer });
  const ticket = action({ action: "view" }).tickets[0];
  action({
    action: "comment",
    id: ticket.id,
    expected_revision: ticket.revision,
    body: "New feedback",
  });
  await page.locator("#refresh").click();
  await column(page, "ready").dispatchEvent("drop", { dataTransfer: transfer });
  await expect(page.locator("#form-error")).toBeVisible();
  await expect(page.locator("#stale")).toBeVisible();
  const current = action({ action: "view" }).tickets[0];
  expect(current.status).toBe("backlog");
  expect(current.activity.some((event) => event.body === "New feedback")).toBe(
    true,
  );
});

test("same-column drops and cancelled backward moves leave tickets unchanged", async ({
  page,
}) => {
  const ticket = action({ action: "view" }).tickets[0];
  action({
    action: "move_ticket",
    id: ticket.id,
    expected_revision: ticket.revision,
    status: "ready",
  });
  await page.goto(`${base}/kanban`);
  const before = action({ action: "view" }).tickets[0];
  await card(page).dragTo(column(page, "ready"));
  await expect(page.locator(".dragging,.drop-target")).toHaveCount(0);
  expect(action({ action: "view" }).tickets[0].revision).toBe(before.revision);
  await card(page).dragTo(column(page, "backlog"));
  await expect(page.locator("#editor")).toBeVisible();
  await page.locator("#close").click();
  expect(action({ action: "view" }).tickets[0]).toEqual(before);
});

test("board fits small screens and backlog sits below active columns", async ({
  page,
}) => {
  action({
    action: "create_ticket",
    title: "A second backlog item with a long title ".repeat(8),
    description: "",
    assignee: "Long assignee ".repeat(12),
  });
  await page.goto(`${base}/kanban`);
  await expect(card(page)).toBeVisible();
  for (const width of [1440, 1024, 768, 390]) {
    await page.setViewportSize({ width, height: 900 });
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= innerWidth,
      ),
    ).toBe(true);
    const active = await page.locator("#columns").boundingBox();
    const backlog = await page.locator("#backlog").boundingBox();
    expect(backlog.y).toBeGreaterThanOrEqual(active.y + active.height);
    expect(backlog.width).toBe(active.width);
    const rows = page.locator("#backlog .kanban-row");
    const first = await rows.nth(0).boundingBox();
    const second = await rows.nth(1).boundingBox();
    expect(second.y).toBeGreaterThanOrEqual(first.y + first.height);
  }
  await page.screenshot({
    path: "/tmp/totui-kanban-mobile.png",
    fullPage: true,
  });
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.screenshot({
    path: "/tmp/totui-kanban-desktop.png",
    fullPage: true,
  });
});

test("backlog rows have To board and Trash, active cards do not", async ({
  page,
}) => {
  await page.goto(`${base}/kanban`);
  await expect(page.locator("#backlog .kanban-row")).toHaveCount(1);
  await card(page)
    .getByRole("button", { name: "To board", exact: true })
    .click();
  await expect(column(page, "ready").locator(".kanban-card")).toHaveCount(1);
  await expect(
    card(page).getByRole("button", { name: "Trash", exact: true }),
  ).toHaveCount(0);
  await expect(
    card(page).getByRole("button", { name: "To backlog", exact: true }),
  ).toHaveCount(0);
  await card(page).locator(".kanban-card-open").click();
  await page.locator("#status").selectOption("backlog");
  await page.locator("#reason").fill("Defer until next week");
  await page.locator("#move").click();
  await expect
    .poll(() => action({ action: "view" }).tickets[0].status)
    .toBe("backlog");
  await page.locator("#close").click();
  await card(page).getByRole("button", { name: "Trash", exact: true }).click();
  await expect(page.locator("#backlog .kanban-row")).toHaveCount(0);
  expect(action({ action: "view" }).tickets[0].trashed).toBe(true);
  await page.locator("#trash-summary").click();
  await card(page)
    .getByRole("button", { name: "Restore", exact: true })
    .click();
  await expect(page.locator("#backlog .kanban-row")).toHaveCount(1);
  const restored = action({ action: "view" }).tickets[0];
  expect(restored.trashed).toBe(false);
  expect(restored.feedback).toBe("Defer until next week");
  expect(restored.description).toBe("Keep details");
});

test("Done archives into a collapsed Completed list and restores with history", async ({
  page,
}) => {
  const ticket = action({ action: "view" }).tickets[0];
  action({
    action: "move_ticket",
    id: ticket.id,
    expected_revision: ticket.revision,
    status: "done",
  });
  await page.goto(`${base}/kanban`);
  await expect(page.locator("#completed")).not.toHaveAttribute("open", "");
  await card(page)
    .getByRole("button", { name: "Archive", exact: true })
    .click();
  await expect(column(page, "done").locator(".kanban-card")).toHaveCount(0);
  await expect(page.locator("#completed-summary")).toHaveText("Completed (1)");
  await expect(page.locator("#completed")).not.toHaveAttribute("open", "");
  expect(action({ action: "view" }).tickets[0].archived).toBe(true);
  await page.locator("#completed-summary").click();
  await expect(page.locator("#completed .kanban-row")).toBeVisible();
  await page.locator("#refresh").click();
  await expect(page.locator("#completed")).toHaveAttribute("open", "");
  await card(page).getByRole("button", { name: "Restore to Done" }).click();
  await expect(column(page, "done").locator(".kanban-card")).toHaveCount(1);
  const restored = action({ action: "view" }).tickets[0];
  expect(restored.archived).toBe(false);
  expect(restored.description).toBe("Keep details");
  expect(restored.activity.at(-1).action).toBe("unarchived");
});
