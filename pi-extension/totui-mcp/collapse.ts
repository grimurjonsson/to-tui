import type { TotuiTodoItem } from "./types.js";

/** Persists across panel open/close within a pi session. */
export const collapsedIds = new Set<string>();

export function hasChildren(items: TotuiTodoItem[], index: number): boolean {
	if (index >= items.length - 1) return false;
	return items[index + 1]!.indent_level > items[index]!.indent_level;
}

export function isCollapsed(id: string): boolean {
	return collapsedIds.has(id);
}

export function toggleCollapsed(id: string): boolean {
	if (collapsedIds.has(id)) {
		collapsedIds.delete(id);
		return false;
	}
	collapsedIds.add(id);
	return true;
}

/** True when any ancestor with children is collapsed. */
export function isHiddenByCollapse(items: TotuiTodoItem[], index: number): boolean {
	let minIndent = items[index]!.indent_level;
	for (let i = index - 1; i >= 0; i--) {
		const item = items[i]!;
		if (item.indent_level < minIndent) {
			if (item.id && collapsedIds.has(item.id)) return true;
			minIndent = item.indent_level;
		}
	}
	return false;
}

export function getVisibleIndices(items: TotuiTodoItem[]): number[] {
	const out: number[] = [];
	for (let i = 0; i < items.length; i++) {
		if (!isHiddenByCollapse(items, i)) out.push(i);
	}
	return out;
}

export function collapseIndicator(items: TotuiTodoItem[], index: number): string {
	if (!hasChildren(items, index)) return "  ";
	const id = items[index]!.id;
	return id && collapsedIds.has(id) ? "▶ " : "▼ ";
}
