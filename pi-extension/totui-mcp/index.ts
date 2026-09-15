import { StringEnum } from "@earendil-works/pi-ai";
import { truncateHead, type ExtensionAPI, type ExtensionContext } from "@earendil-works/pi-coding-agent";
import { Type, type TSchema } from "typebox";
import { mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describeDestination, TotuiCliClient } from "./client.js";
import { TotuiDataSource } from "./data-source.js";
import { TotuiPanelComponent } from "./panel.js";
import { formatFocusStatus } from "./focus.js";
import { matchesTotuiPanelShortcut, TOTUI_PANEL_SHORTCUT_FALLBACK, TOTUI_PANEL_SHORTCUT_PRIMARY } from "./shortcuts.js";
import { clearWidget, renderWidget, startWidgetPoller } from "./widget.js";
import type { TotuiTodoList } from "./types.js";

export default function (pi: ExtensionAPI) {
	pi.registerFlag("totui-command", { description: "Remote-aware totui CLI binary (default: totui)", type: "string" });
	pi.registerFlag("totui-remote", { description: "Named totui remote (default: CLI configuration)", type: "string" });
	pi.registerFlag("totui-local", { description: "Explicitly select local totui storage", type: "boolean" });
	pi.registerFlag("totui-project", { description: "Project override (default: CLI folder mapping)", type: "string" });
	pi.registerFlag("totui-poll-ms", { description: "Widget refresh interval (default: 5000 ms)", type: "string" });
	pi.registerFlag("totui-widget", { description: "Show focused todos above editor", type: "boolean", default: true });
	pi.registerFlag("totui-api-command", { description: "Compatibility alias for --totui-command", type: "string" });
	for (const flag of ["totui-api-url", "totui-mcp-command", "totui-mcp-args", "totui-widget-roots"]) {
		pi.registerFlag(flag, { description: "Removed: see totui extension README for CLI migration", type: "string" });
	}
	pi.registerFlag("totui-auto-api", { description: "Removed: the CLI does not start a local API", type: "boolean" });

	const stringFlag = (name: string, env: string): string | undefined => {
		const value = pi.getFlag(name);
		return (typeof value === "string" && value) || process.env[env] || undefined;
	};
	const sources = new Map<string, TotuiDataSource>();
	let sessionCtx: ExtensionContext | undefined;
	let generation = 0;
	let poller: ReturnType<typeof startWidgetPoller> | undefined;
	let closePanel: (() => void) | undefined;
	let panelBusy = false;
	let unsubscribeTerminalInput: (() => void) | undefined;
	const widgetEnabled = () => pi.getFlag("totui-widget") !== false;

	const getData = (ctx: ExtensionContext): TotuiDataSource => {
		let data = sources.get(ctx.cwd);
		if (data) return data;
		for (const [flag, env] of [
			["totui-api-url", "TOTUI_API_URL"],
			["totui-mcp-command", "TOTUI_MCP_COMMAND"], ["totui-mcp-args", "TOTUI_MCP_ARGS"],
		]) {
			const value = stringFlag(flag, env);
			if (flag === "totui-mcp-command" && value === "totui-mcp") continue;
			if (value) throw new Error(`Remove --${flag}/${env}; configure 'totui remote' and use --totui-command/--totui-remote instead.`);
		}
		data = new TotuiDataSource(new TotuiCliClient({
			cwd: ctx.cwd,
			command: stringFlag("totui-command", "TOTUI_COMMAND") ?? stringFlag("totui-api-command", "TOTUI_API_COMMAND"),
			remote: stringFlag("totui-remote", "TOTUI_REMOTE"),
			local: pi.getFlag("totui-local") === true,
			project: stringFlag("totui-project", "TOTUI_PROJECT"),
		}));
		sources.set(ctx.cwd, data);
		return data;
	};

	const applyList = (ctx: ExtensionContext, list: TotuiTodoList, expectedGeneration: number) => {
		if (!sessionCtx || generation !== expectedGeneration || sessionCtx.cwd !== ctx.cwd) return;
		if (widgetEnabled()) renderWidget(ctx, list, 4);
		if (ctx.hasUI) ctx.ui.setStatus("totui-mcp",
			`totui: ${list.destination ? describeDestination(list.destination) : "unresolved"} · ${list.error ? "unavailable" : "cli"}${formatFocusStatus(list.items)}`);
	};

	const start = async (ctx: ExtensionContext) => {
		const currentGeneration = ++generation;
		closePanel?.();
		closePanel = undefined;
		panelBusy = false;
		poller?.stop();
		poller = undefined;
		const data = getData(ctx);
		if (!ctx.hasUI) return;
		const interval = Number(stringFlag("totui-poll-ms", "TOTUI_POLL_MS"));
		poller = startWidgetPoller(data, Number.isFinite(interval) && interval > 0 ? interval : 5000,
			list => applyList(ctx, list, currentGeneration));
		const list = await poller.refresh();
		if (currentGeneration !== generation) return;
		if (list.error) ctx.ui.notify(list.error, "warning");
	};

	pi.on("session_start", async (_event, ctx) => {
		sessionCtx = ctx;
		const expectedGeneration = generation + 1;
		try { await start(ctx); } catch (error) {
			if (ctx.hasUI && generation === expectedGeneration) ctx.ui.notify(String(error), "error");
		}
		if (generation !== expectedGeneration || sessionCtx !== ctx) return;
		if (ctx.mode === "tui") {
			unsubscribeTerminalInput?.();
			unsubscribeTerminalInput = ctx.ui.onTerminalInput(input => {
				if (!matchesTotuiPanelShortcut(input)) return;
				void togglePanel(ctx).catch(error => ctx.ui.notify(String(error), "error"));
				return { consume: true };
			});
		}
	});

	pi.on("session_shutdown", async (_event, ctx) => {
		generation++;
		sessionCtx = undefined;
		unsubscribeTerminalInput?.();
		unsubscribeTerminalInput = undefined;
		poller?.stop();
		poller = undefined;
		closePanel?.();
		closePanel = undefined;
		panelBusy = false;
		for (const data of sources.values()) data.client.close();
		sources.clear();
		clearWidget(ctx);
		if (ctx.hasUI) ctx.ui.setStatus("totui-mcp", undefined);
	});

	const togglePanel = async (ctx: ExtensionContext) => {
		if (closePanel) { closePanel(); return; }
		if (panelBusy || ctx.mode !== "tui") return;
		panelBusy = true;
		const currentGeneration = generation;
		try {
			const data = getData(ctx);
			const list = await data.listTodos();
			if (currentGeneration !== generation || !sessionCtx) return;
			applyList(ctx, list, currentGeneration);
			await ctx.ui.custom<void>((tui, theme, _kb, done) => {
				closePanel = () => done();
				return new TotuiPanelComponent(list, theme, data, () => done(),
					updated => applyList(ctx, updated, currentGeneration), () => tui.requestRender());
			}, { overlay: true, overlayOptions: { anchor: "top-center", margin: 1 } });
		} finally {
			if (currentGeneration === generation) { panelBusy = false; closePanel = undefined; }
		}
	};

	pi.registerCommand("totui", { description: "Toggle totui panel (remote-aware CLI, no LLM)", handler: async (_args, ctx) => togglePanel(ctx) });
	pi.registerCommand("totui-refresh", {
		description: "Refresh todos through the CLI",
		handler: async (_args, ctx) => {
			const currentGeneration = generation;
			const list = poller ? await poller.refresh() : await getData(ctx).listTodos();
			if (currentGeneration !== generation) return;
			applyList(ctx, list, currentGeneration);
			if (ctx.hasUI) ctx.ui.notify(list.error ?? `Refreshed ${list.items.length} items`, list.error ? "error" : "info");
		},
	});
	pi.registerCommand("totui-unfocus", {
		description: "Clear all focused ([*]) todos in the selected workspace",
		handler: async (_args, ctx) => {
			const currentGeneration = generation;
			const data = getData(ctx);
			const list = await data.listTodos();
			if (currentGeneration !== generation) return;
			if (list.error) throw new Error(list.error);
			const updated = await data.clearFocus(list.items, list.date);
			if (currentGeneration !== generation) return;
			applyList(ctx, updated, currentGeneration);
			if (ctx.hasUI) ctx.ui.notify(updated.error ?? "Focus cleared", updated.error ? "error" : "info");
		},
	});
	pi.registerCommand("totui-reconnect", {
		description: "Re-resolve CLI backend/project and refresh (no MCP)",
		handler: async (_args, ctx) => {
			poller?.stop();
			sources.get(ctx.cwd)?.client.close();
			sources.delete(ctx.cwd);
			await start(ctx);
		},
	});
	for (const shortcut of [TOTUI_PANEL_SHORTCUT_PRIMARY, TOTUI_PANEL_SHORTCUT_FALLBACK]) {
		pi.registerShortcut(shortcut, { description: "Toggle totui panel", handler: togglePanel });
	}

	const register = (name: string, description: string, parameters: TSchema, guidelines: string[] = []) => {
		pi.registerTool({
			name: `totui_${name}`, label: `Totui ${name.replaceAll("_", " ")}`,
			description: `${description} Uses the remote-aware CLI with the session folder's project unless overridden. Output capped at 50KB/2000 lines; full output saved to a file when truncated.`,
			promptSnippet: `${description} (totui CLI/API)`,
			promptGuidelines: guidelines,
			parameters,
			async execute(_id, rawParams, signal, onUpdate, ctx) {
				const params = rawParams as Record<string, unknown>;
				const data = getData(ctx);
				const result = name === "context"
					? { destination: await data.client.context(params.project as string | undefined, signal) }
					: await data.client.callTool(name, params, signal, destination => {
						const text = `Totui destination: ${describeDestination(destination)}`;
						onUpdate?.({ content: [{ type: "text", text }], details: { destination } });
						if (ctx.hasUI) ctx.ui.notify(text, "info");
					});
				if (!["context", "list_todos", "list_projects"].includes(name) && sessionCtx?.cwd === ctx.cwd) {
					if (poller) void poller.refresh();
				}
				const fullText = JSON.stringify(result, null, 2);
				const truncated = truncateHead(fullText);
				let text = truncated.content;
				if (truncated.truncated) {
					const path = join(await mkdtemp(join(tmpdir(), "totui-result-")), "result.json");
					await writeFile(path, fullText, { mode: 0o600 });
					text += `\n[Truncated. Full result: ${path}]`;
				}
				return { content: [{ type: "text", text }], details: { destination: result.destination } };
			},
		});
	};
	const scope = {
		project: Type.Optional(Type.String({ description: "Project override; defaults to CLI folder mapping" })),
		date: Type.Optional(Type.String({ description: "YYYY-MM-DD; defaults to today" })),
	};
	const id = Type.String({ description: "Todo UUID from the selected backend" });
	register("context", "Resolve the totui destination before writing.", Type.Object({ project: scope.project }),
		["Use totui_context to resolve and announce the backend/project before creating todos. Totui tools use the CLI/API, not legacy MCP. Never fall back to local storage on remote errors."]);
	register("list_todos", "List daily todos.", Type.Object(scope));
	register("create_todo", "Create a todo, optionally nested under parent_id.", Type.Object({
		...scope, content: Type.String(), description: Type.Optional(Type.String()),
		due_date: Type.Optional(Type.String()), parent_id: Type.Optional(Type.String()),
	}));
	register("update_todo", "Update a todo: space pending, * focus, x done, ? question, ! important, - cancelled.", Type.Object({
		...scope, id, content: Type.Optional(Type.String()), description: Type.Optional(Type.String()),
		due_date: Type.Optional(Type.String()), state: Type.Optional(StringEnum([" ", "*", "x", "?", "!", "-"] as const)),
	}));
	register("delete_todo", "Delete a todo and its children (irreversible).", Type.Object({ ...scope, id }),
		["Confirm with the user before calling totui_delete_todo; it also deletes children."]);
	register("mark_complete", "Toggle a todo between done and pending.", Type.Object({ ...scope, id }));
	register("list_projects", "List project names on the selected backend.", Type.Object({}));
}
