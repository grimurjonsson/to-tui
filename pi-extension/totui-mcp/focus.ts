import type { TotuiTodoItem, TotuiTodoList } from "./types.js";

/** Totui `[*]` = in progress / active / focused for pi extension. */
export const FOCUS_STATE = "*";

export function findFocusedItems(items: TotuiTodoItem[]): TotuiTodoItem[] {
	return items.filter((item) => item.state === FOCUS_STATE);
}

export function findFocusedItem(list: TotuiTodoList): TotuiTodoItem | undefined {
	return findFocusedItems(list.items)[0];
}

export function focusedLabel(item: TotuiTodoItem): string {
	const indent = "  ".repeat(item.indent_level);
	return `${indent}${item.content}`;
}

export function formatFocusStatus(items: TotuiTodoItem[], maxLen = 36): string {
	const focused = findFocusedItems(items);
	if (focused.length === 0) return "";

	if (focused.length === 1) {
		const c = focused[0]!.content;
		return ` · 🔸 ${c.length > maxLen ? `${c.slice(0, maxLen - 1)}…` : c}`;
	}

	const first = focused[0]!.content;
	const short = first.length > 20 ? `${first.slice(0, 19)}…` : first;
	return ` · 🔸 ${short} +${focused.length - 1}`;
}
