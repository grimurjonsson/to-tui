import type { ExtensionContext } from "@earendil-works/pi-coding-agent";
import type { TotuiDataSource } from "./data-source.js";
import { formatWidgetLines } from "./format.js";
import type { TotuiTodoList } from "./types.js";

export const WIDGET_ID = "totui-panel";

export function renderWidget(ctx: ExtensionContext, list: TotuiTodoList, maxRoots: number): void {
	if (!ctx.hasUI) return;

	const lines = formatWidgetLines(list, ctx.ui.theme, maxRoots);
	ctx.ui.setWidget(WIDGET_ID, lines, { placement: "aboveEditor" });
}

export function clearWidget(ctx: ExtensionContext): void {
	if (!ctx.hasUI) return;
	ctx.ui.setWidget(WIDGET_ID, undefined);
}

export async function refreshWidget(
	ctx: ExtensionContext,
	data: TotuiDataSource,
	maxRoots: number,
): Promise<TotuiTodoList> {
	const list = await data.listTodos();
	renderWidget(ctx, list, maxRoots);
	return list;
}

export function startWidgetPoller(
	ctx: ExtensionContext,
	data: TotuiDataSource,
	intervalMs: number,
	maxRoots: number,
): { stop: () => void; refresh: () => Promise<TotuiTodoList> } {
	let timer: ReturnType<typeof setInterval> | null = null;
	let currentCtx = ctx;

	const refresh = () => refreshWidget(currentCtx, data, maxRoots);

	void refresh();

	timer = setInterval(() => {
		void refresh();
	}, intervalMs);

	return {
		stop: () => {
			if (timer) clearInterval(timer);
			timer = null;
		},
		refresh,
	};
}
