import type { ExtensionContext } from "@earendil-works/pi-coding-agent";
import type { TotuiDataSource } from "./data-source.js";
import { formatWidgetLines } from "./format.js";
import type { TotuiTodoList } from "./types.js";

export const WIDGET_ID = "totui-panel";

export function renderWidget(ctx: ExtensionContext, list: TotuiTodoList, maxRoots: number): void {
	if (ctx.hasUI) ctx.ui.setWidget(WIDGET_ID, formatWidgetLines(list, ctx.ui.theme, maxRoots), { placement: "aboveEditor" });
}

export function clearWidget(ctx: ExtensionContext): void {
	if (ctx.hasUI) ctx.ui.setWidget(WIDGET_ID, undefined);
}

export function startWidgetPoller(
	data: TotuiDataSource,
	intervalMs: number,
	onList: (list: TotuiTodoList) => void,
): { stop: () => void; refresh: () => Promise<TotuiTodoList> } {
	let stopped = false;
	let inFlight: Promise<TotuiTodoList> | undefined;
	const refresh = (): Promise<TotuiTodoList> => {
		if (!inFlight) {
			inFlight = data.listTodos().then(list => {
				if (!stopped) onList(list);
				return list;
			}).finally(() => { inFlight = undefined; });
		}
		return inFlight;
	};
	const timer = setInterval(() => { void refresh(); }, intervalMs);
	timer.unref?.();
	return { stop: () => { stopped = true; clearInterval(timer); }, refresh };
}
