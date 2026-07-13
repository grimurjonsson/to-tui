/**
 * Pi extension: totui panel (REST API) + optional MCP tools for the LLM.
 *
 * The widget/panel fetch todos without LLM tool calls.
 * API is auto-started on session start (totui serve start).
 *
 * Install: just setup-pi-extension
 */

import { StringEnum } from "@earendil-works/pi-ai";
import type { ExtensionAPI, ExtensionContext } from "@earendil-works/pi-coding-agent";
import { Text } from "@earendil-works/pi-tui";
import { Type } from "typebox";
import { ensureApiServer } from "./api-server.js";
import { createDataSourceFromEnv } from "./data-source.js";
import { mcpResultToText, TotuiMcpClient } from "./client.js";
import { TotuiPanelComponent } from "./panel.js";
import { formatFocusStatus } from "./focus.js";
import {
	matchesTotuiPanelShortcut,
	TOTUI_PANEL_SHORTCUT_FALLBACK,
	TOTUI_PANEL_SHORTCUT_PRIMARY,
} from "./shortcuts.js";
import { clearWidget, refreshWidget, renderWidget, startWidgetPoller } from "./widget.js";
import type { TotuiTodoList } from "./types.js";

const DEFAULT_COMMAND = "totui-mcp";
const DEFAULT_API_COMMAND = "totui";
const DEFAULT_POLL_MS = 5000;
const DEFAULT_WIDGET_ROOTS = 4;

function parseArgsFlag(raw: string | boolean | undefined): string[] {
	if (raw === undefined || raw === false || raw === true) return [];
	const trimmed = String(raw).trim();
	if (!trimmed) return [];
	return trimmed.split(/\s+/);
}

function stripUndefined<T extends Record<string, unknown>>(obj: T): Record<string, unknown> {
	const out: Record<string, unknown> = {};
	for (const [k, v] of Object.entries(obj)) {
		if (v !== undefined) out[k] = v;
	}
	return out;
}

function parseIntFlag(raw: string | boolean | undefined, fallback: number): number {
	if (typeof raw !== "string") return fallback;
	const n = Number.parseInt(raw, 10);
	return Number.isFinite(n) && n > 0 ? n : fallback;
}

export default function (pi: ExtensionAPI) {
	const command =
		(typeof pi.getFlag("totui-mcp-command") === "string" && pi.getFlag("totui-mcp-command")) ||
		process.env.TOTUI_MCP_COMMAND ||
		DEFAULT_COMMAND;
	const args =
		parseArgsFlag(pi.getFlag("totui-mcp-args")) ||
		parseArgsFlag(process.env.TOTUI_MCP_ARGS) ||
		[];

	const pollMs = parseIntFlag(pi.getFlag("totui-poll-ms"), DEFAULT_POLL_MS);
	const widgetRoots = parseIntFlag(pi.getFlag("totui-widget-roots"), DEFAULT_WIDGET_ROOTS);
	const widgetEnabled = pi.getFlag("totui-widget") !== false;
	const autoApi = pi.getFlag("totui-auto-api") !== false;

	const apiUrl =
		(typeof pi.getFlag("totui-api-url") === "string" && pi.getFlag("totui-api-url")) ||
		process.env.TOTUI_API_URL ||
		"http://127.0.0.1:3000";
	const apiCommand =
		(typeof pi.getFlag("totui-api-command") === "string" && pi.getFlag("totui-api-command")) ||
		process.env.TOTUI_API_COMMAND ||
		DEFAULT_API_COMMAND;

	const data = createDataSourceFromEnv({
		apiUrl,
		project:
			(typeof pi.getFlag("totui-project") === "string" && pi.getFlag("totui-project")) ||
			process.env.TOTUI_PROJECT ||
			undefined,
	});

	const mcp = new TotuiMcpClient({ command, args });

	let poller: ReturnType<typeof startWidgetPoller> | null = null;
	let latestList: TotuiTodoList | null = null;
	let sessionCtx: ExtensionContext | null = null;
	let panelOpen = false;
	let panelBusy = false;
	let closePanel: (() => void) | null = null;
	let unsubscribeTerminalInput: (() => void) | null = null;

	pi.registerFlag("totui-mcp-command", {
		description: "totui-mcp binary (default: totui-mcp on PATH)",
		type: "string",
		default: DEFAULT_COMMAND,
	});
	pi.registerFlag("totui-mcp-args", {
		description: "Extra args for totui-mcp (space-separated)",
		type: "string",
	});
	pi.registerFlag("totui-api-url", {
		description: "totui REST API base URL (default: http://127.0.0.1:3000)",
		type: "string",
	});
	pi.registerFlag("totui-project", {
		description: "totui project name (default: default)",
		type: "string",
	});
	pi.registerFlag("totui-poll-ms", {
		description: "Widget refresh interval in ms (default: 5000)",
		type: "string",
	});
	pi.registerFlag("totui-widget-roots", {
		description: "Unused (widget shows focused [*] items only). Kept for compat.",
		type: "string",
	});
	pi.registerFlag("totui-widget", {
		description: "Show compact totui widget above editor (default: true)",
		type: "boolean",
		default: true,
	});
	pi.registerFlag("totui-auto-api", {
		description: "Auto-start totui REST API on session start (default: true)",
		type: "boolean",
		default: true,
	});
	pi.registerFlag("totui-api-command", {
		description: "totui binary for serve start (default: totui on PATH)",
		type: "string",
		default: DEFAULT_API_COMMAND,
	});

	const applyList = (ctx: ExtensionContext, list: TotuiTodoList) => {
		latestList = list;
		if (widgetEnabled) renderWidget(ctx, list, widgetRoots);
	};

	const refreshStatus = async (ctx: ExtensionContext) => {
		const apiOk = await data.checkApiHealth();
		const focusSuffix = latestList ? formatFocusStatus(latestList.items) : "";
		ctx.ui.setStatus(
			"totui-mcp",
			apiOk
				? `totui: ${data.project} · api${focusSuffix}`
				: `totui: ${data.project} · offline${focusSuffix}`,
		);
	};

	pi.on("session_start", async (_event, ctx) => {
		sessionCtx = ctx;

		const api = await ensureApiServer({
			baseUrl: apiUrl,
			command: apiCommand,
			autoStart: autoApi,
		});
		if (!api.ok && ctx.hasUI) {
			ctx.ui.notify(api.error ?? "totui API unavailable", "warning");
		}

		if (!ctx.hasUI) return;

		try {
			await mcp.connect();
		} catch {
			// MCP optional when using direct panel
		}

		if (widgetEnabled) {
			poller = startWidgetPoller(ctx, data, pollMs, widgetRoots);
			poller.refresh().then((list) => {
				latestList = list;
				void refreshStatus(ctx);
			});
		} else {
			const list = await data.listTodos();
			applyList(ctx, list);
			void refreshStatus(ctx);
		}

		unsubscribeTerminalInput?.();
		unsubscribeTerminalInput = ctx.ui.onTerminalInput((data) => {
			if (!matchesTotuiPanelShortcut(data)) return;
			void togglePanel(ctx);
			return { consume: true };
		});
	});

	pi.on("session_shutdown", async (_event, ctx) => {
		unsubscribeTerminalInput?.();
		unsubscribeTerminalInput = null;
		poller?.stop();
		poller = null;
		panelOpen = false;
		panelBusy = false;
		closePanel = null;
		clearWidget(ctx);
		sessionCtx = null;
		await mcp.close();
	});

	const openPanel = async (ctx: ExtensionContext) => {
		if (!ctx.hasUI) {
			ctx.ui.notify("totui panel requires interactive mode", "error");
			return;
		}

		panelOpen = true;
		let list = latestList ?? (await data.listTodos());

		try {
			await ctx.ui.custom<void>(
				(_tui, theme, _kb, done) => {
					closePanel = () => done();
					const panel = new TotuiPanelComponent(
						list,
						theme,
						data,
						() => {
							closePanel = null;
							done();
						},
						(updated) => {
							list = updated;
							latestList = updated;
							if (sessionCtx && widgetEnabled) renderWidget(sessionCtx, updated, widgetRoots);
							if (sessionCtx) void refreshStatus(sessionCtx);
						},
					);
					return panel;
				},
				{
					overlay: true,
					overlayOptions: { anchor: "top-center", margin: 1 },
				},
			);
		} finally {
			panelOpen = false;
			closePanel = null;
		}
	};

	const togglePanel = async (ctx: ExtensionContext) => {
		if (panelOpen && closePanel) {
			closePanel();
			return;
		}
		if (panelBusy) return;
		panelBusy = true;
		try {
			await openPanel(ctx);
		} finally {
			panelBusy = false;
		}
	};

	pi.registerCommand("totui", {
		description: "Toggle totui todo panel (direct API, no LLM)",
		handler: async (_args, ctx) => togglePanel(ctx),
	});

	pi.registerCommand("totui-refresh", {
		description: "Refresh totui widget from API",
		handler: async (_args, ctx) => {
			if (!ctx.hasUI) return;
			const list = poller ? await poller.refresh() : await refreshWidget(ctx, data, widgetRoots);
			latestList = list;
			await refreshStatus(ctx);
			ctx.ui.notify(`Refreshed (${list.items.length} items, ${list.source})`, "info");
		},
	});

	pi.registerCommand("totui-unfocus", {
		description: "Clear all focused ([*]) todos",
		handler: async (_args, ctx) => {
			if (!ctx.hasUI) return;
			const list = latestList ?? (await data.listTodos());
			const updated = await data.clearFocus(list.items, list.date);
			latestList = updated;
			if (widgetEnabled) renderWidget(ctx, updated, widgetRoots);
			await refreshStatus(ctx);
			ctx.ui.notify("Focus cleared", "info");
		},
	});

	pi.registerCommand("totui-reconnect", {
		description: "Reconnect to totui-mcp",
		handler: async (_args, ctx) => {
			await mcp.close();
			try {
				await mcp.connect();
				ctx.ui.notify("Reconnected to totui-mcp", "info");
			} catch (err) {
				const msg = err instanceof Error ? err.message : String(err);
				ctx.ui.notify(`Reconnect failed: ${msg}`, "error");
			}
		},
	});

	const registerPanelShortcut = (shortcut: typeof TOTUI_PANEL_SHORTCUT_PRIMARY, label: string) => {
		pi.registerShortcut(shortcut, {
			description: `Toggle totui panel (${label})`,
			handler: async (ctx) => togglePanel(ctx),
		});
	};

	registerPanelShortcut(TOTUI_PANEL_SHORTCUT_PRIMARY, "⌘⇧T");
	registerPanelShortcut(TOTUI_PANEL_SHORTCUT_FALLBACK, "ctrl+shift+T fallback");

	const callMcp = async (
		toolName: string,
		params: Record<string, unknown>,
		signal?: AbortSignal,
	) => {
		const result = await mcp.callTool(toolName, stripUndefined(params), signal);
		const text = mcpResultToText(result);
		if (result.isError) throw new Error(text);
		return {
			content: [{ type: "text" as const, text }],
			details: { tool: toolName, raw: result },
		};
	};

	pi.registerTool({
		name: "totui_list_todos",
		label: "Totui List Todos",
		description: "List todos via MCP. Prefer /totui panel for viewing — faster, no LLM round-trip.",
		promptSnippet: "List daily todos from totui (totui-mcp)",
		promptGuidelines: [
			"User can view todos with /totui or the widget — do not call this for simple viewing.",
			"Use when programmatic access is needed (create/update/delete).",
		],
		parameters: Type.Object({
			date: Type.Optional(Type.String({ description: "YYYY-MM-DD (default: today)" })),
			project: Type.Optional(Type.String({ description: "Project name (default: default)" })),
		}),
		async execute(_id, params, signal) {
			return callMcp("list_todos", params, signal);
		},
		renderCall(args, theme) {
			const proj = args.project ? ` ${theme.fg("muted", args.project)}` : "";
			const date = args.date ? theme.fg("dim", ` ${args.date}`) : "";
			return new Text(theme.fg("toolTitle", "totui_list_todos") + proj + date, 0, 0);
		},
	});

	pi.registerTool({
		name: "totui_create_todo",
		label: "Totui Create Todo",
		description: "Create a todo in totui. Optionally nest under parent_id.",
		promptSnippet: "Add a todo in totui (totui-mcp)",
		promptGuidelines: ["Use totui_create_todo when the user wants to add a totui todo item."],
		parameters: Type.Object({
			content: Type.String({ description: "Todo text" }),
			project: Type.Optional(Type.String()),
			date: Type.Optional(Type.String({ description: "YYYY-MM-DD" })),
			description: Type.Optional(Type.String()),
			due_date: Type.Optional(Type.String({ description: "YYYY-MM-DD" })),
			parent_id: Type.Optional(Type.String({ description: "Parent todo UUID" })),
		}),
		async execute(_id, params, signal) {
			const result = await callMcp("create_todo", params, signal);
			if (sessionCtx && poller) void poller.refresh();
			return result;
		},
	});

	pi.registerTool({
		name: "totui_update_todo",
		label: "Totui Update Todo",
		description: "Update a totui todo. States: ' ' pending, '*' in progress/focus, 'x' done, '?' question, '!' important.",
		promptSnippet: "Update a totui todo (totui-mcp)",
		promptGuidelines: ["Use totui_update_todo to edit totui todos; use /totui panel to get IDs."],
		parameters: Type.Object({
			id: Type.String({ description: "Todo UUID" }),
			content: Type.Optional(Type.String()),
			state: Type.Optional(StringEnum([" ", "x", "?", "!", "*"] as const)),
			project: Type.Optional(Type.String()),
			date: Type.Optional(Type.String()),
			description: Type.Optional(Type.String()),
			due_date: Type.Optional(Type.String()),
		}),
		async execute(_id, params, signal) {
			const result = await callMcp("update_todo", params, signal);
			if (sessionCtx && poller) void poller.refresh();
			return result;
		},
	});

	pi.registerTool({
		name: "totui_delete_todo",
		label: "Totui Delete Todo",
		description: "Delete a totui todo and its children (irreversible).",
		promptSnippet: "Delete a totui todo (totui-mcp)",
		promptGuidelines: ["Use totui_delete_todo to remove totui todos; confirm with the user first."],
		parameters: Type.Object({
			id: Type.String({ description: "Todo UUID" }),
			project: Type.Optional(Type.String()),
			date: Type.Optional(Type.String()),
		}),
		async execute(_id, params, signal) {
			const result = await callMcp("delete_todo", params, signal);
			if (sessionCtx && poller) void poller.refresh();
			return result;
		},
	});

	pi.registerTool({
		name: "totui_mark_complete",
		label: "Totui Toggle Done",
		description: "Toggle totui todo done/pending.",
		promptSnippet: "Toggle totui todo completion (totui-mcp)",
		promptGuidelines: ["User can toggle in /totui panel. Use for agent-driven updates only."],
		parameters: Type.Object({
			id: Type.String({ description: "Todo UUID" }),
			project: Type.Optional(Type.String()),
			date: Type.Optional(Type.String()),
		}),
		async execute(_id, params, signal) {
			const result = await callMcp("mark_complete", params, signal);
			if (sessionCtx && poller) void poller.refresh();
			return result;
		},
	});

	pi.registerTool({
		name: "totui_list_projects",
		label: "Totui List Projects",
		description: "List totui projects (names, IDs, created_at).",
		promptSnippet: "List totui projects (totui-mcp)",
		promptGuidelines: ["Use totui_list_projects when the user asks which totui projects exist."],
		parameters: Type.Object({}),
		async execute(_id, _params, signal) {
			return callMcp("list_projects", {}, signal);
		},
	});
}
