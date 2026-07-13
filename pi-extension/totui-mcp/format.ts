import type { Theme } from "@earendil-works/pi-coding-agent";
import { truncateToWidth } from "@earendil-works/pi-tui";
import { findFocusedItems } from "./focus.js";
import type { TotuiTodoItem, TotuiTodoList } from "./types.js";

export function stateIcon(state: string, items: TotuiTodoItem[], index: number): string {
	switch (state) {
		case "x":
			return "✅";
		case "?":
			return "❔";
		case "!":
			return "❗";
		case "*":
			return "🔸";
		default:
			return hasCompletedDescendants(items, index) ? "🔳" : "⬜";
	}
}

function hasCompletedDescendants(items: TotuiTodoItem[], idx: number): boolean {
	const base = items[idx]?.indent_level ?? 0;
	for (let i = idx + 1; i < items.length; i++) {
		const item = items[i]!;
		if (item.indent_level <= base) break;
		if (item.state === "x") return true;
	}
	return false;
}

export function countDone(items: TotuiTodoItem[]): { done: number; total: number } {
	let done = 0;
	for (const item of items) {
		if (item.state === "x") done++;
	}
	return { done, total: items.length };
}

export function formatTodoLine(item: TotuiTodoItem, items: TotuiTodoItem[], index: number, theme: Theme, width: number): string {
	const indent = "  ".repeat(item.indent_level);
	const icon = stateIcon(item.state, items, index);
	const done = item.state === "x";
	const text = done ? theme.fg("dim", item.content) : theme.fg("text", item.content);
	return truncateToWidth(`${indent}${icon} ${text}`, width);
}

export function formatWidgetLines(list: TotuiTodoList, theme: Theme, _maxRoots: number): string[] {
	const { done, total } = countDone(list.items);
	const focused = findFocusedItems(list.items);
	const header =
		theme.fg("accent", " totui ") +
		theme.fg("muted", ` ${list.date} (${done}/${total})`) +
		theme.fg("dim", "  ⌘⇧T · ctrl+⇧T");

	const lines = [header];

	if (focused.length > 0) {
		const preview = focused.map((item) => {
			const idx = list.items.indexOf(item);
			const icon = stateIcon(item.state, list.items, idx);
			const indent = "  ".repeat(item.indent_level);
			const label =
				item.state === "x" ? theme.fg("dim", item.content) : theme.fg("accent", item.content);
			return `${icon} ${indent}${label}`;
		});
		lines.push(theme.fg("dim", " focus ") + preview.join("  "));
	}

	if (list.error) {
		lines.push(theme.fg("warning", list.error));
	} else if (list.source === "error") {
		lines.push(theme.fg("warning", "API unavailable"));
	}

	return lines;
}

export function formatPanelHeader(list: TotuiTodoList, theme: Theme, width: number): string {
	const { done, total } = countDone(list.items);
	const title = theme.fg("accent", " totui ");
	const stats = theme.fg("muted", `${list.date} · ${done}/${total}`);
	const hint = theme.fg("dim", " ⌘⇧T · ctrl+⇧T");
	return truncateToWidth(`${title}${stats}${hint}`, width);
}
