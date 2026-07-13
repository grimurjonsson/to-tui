import type { Theme } from "@earendil-works/pi-coding-agent";
import { matchesKey, visibleWidth } from "@earendil-works/pi-tui";
import {
	collapseIndicator,
	getVisibleIndices,
	hasChildren,
	toggleCollapsed,
} from "./collapse.js";
import type { TotuiDataSource } from "./data-source.js";
import { countDone, formatPanelHeader, formatTodoLine } from "./format.js";
import type { TotuiTodoList } from "./types.js";

export class TotuiPanelComponent {
	readonly width = 76;

	private scroll = 0;
	private selectedVisible = 0;
	private selectedId: string | null = null;
	private list: TotuiTodoList;
	private visible: number[] = [];
	private readonly visibleRows: number;
	private cachedWidth?: number;
	private cachedLines?: string[];
	private refreshing = false;
	private readonly theme: Theme;
	private readonly data: TotuiDataSource;
	private readonly onClose: () => void;
	private readonly onRefresh: (list: TotuiTodoList) => void;

	constructor(
		list: TotuiTodoList,
		theme: Theme,
		data: TotuiDataSource,
		onClose: () => void,
		onRefresh: (list: TotuiTodoList) => void,
	) {
		this.list = list;
		this.theme = theme;
		this.data = data;
		this.onClose = onClose;
		this.onRefresh = onRefresh;
		this.visibleRows = 18;
		this.rebuildVisible();
	}

	private rebuildVisible(): void {
		this.visible = getVisibleIndices(this.list.items);
		if (this.selectedId) {
			const idx = this.visible.findIndex((raw) => this.list.items[raw]?.id === this.selectedId);
			if (idx >= 0) this.selectedVisible = idx;
		}
		this.clampSelection();
	}

	private clampSelection(): void {
		if (this.visible.length === 0) {
			this.selectedVisible = 0;
			this.scroll = 0;
			return;
		}
		this.selectedVisible = Math.min(this.selectedVisible, this.visible.length - 1);
		const raw = this.visible[this.selectedVisible];
		if (raw !== undefined) this.selectedId = this.list.items[raw]?.id ?? null;

		const maxScroll = Math.max(0, this.visible.length - this.visibleRows);
		this.scroll = Math.min(this.scroll, maxScroll);
		if (this.selectedVisible < this.scroll) this.scroll = this.selectedVisible;
		if (this.selectedVisible >= this.scroll + this.visibleRows) {
			this.scroll = this.selectedVisible - this.visibleRows + 1;
		}
	}

	private rawSelected(): number | undefined {
		return this.visible[this.selectedVisible];
	}

	updateList(list: TotuiTodoList): void {
		this.list = list;
		this.rebuildVisible();
		this.invalidate();
	}

	handleInput(data: string): void {
		if (matchesKey(data, "escape") || matchesKey(data, "ctrl+c")) {
			this.onClose();
			return;
		}

		if (matchesKey(data, "up")) {
			this.selectedVisible = Math.max(0, this.selectedVisible - 1);
			this.clampSelection();
			this.invalidate();
			return;
		}

		if (matchesKey(data, "down")) {
			this.selectedVisible = Math.min(this.visible.length - 1, this.selectedVisible + 1);
			this.clampSelection();
			this.invalidate();
			return;
		}

		if (matchesKey(data, "return") || data === " ") {
			void this.toggleSelected();
			return;
		}

		if (data === "e" || data === "E" || matchesKey(data, "right") || matchesKey(data, "left")) {
			this.toggleCollapseSelected();
			return;
		}

		if (data === "r" || data === "R") {
			void this.refresh();
			return;
		}

		if (data === "f") {
			void this.focusExclusive();
			return;
		}

		if (data === "F") {
			void this.focusMulti();
			return;
		}

		if (data === "u" || data === "U") {
			void this.unfocusAll();
		}
	}

	private toggleCollapseSelected(): void {
		const raw = this.rawSelected();
		if (raw === undefined) return;
		const item = this.list.items[raw];
		if (!item?.id || !hasChildren(this.list.items, raw)) return;

		toggleCollapsed(item.id);
		this.rebuildVisible();
		this.invalidate();
	}

	private async refresh(): Promise<void> {
		if (this.refreshing) return;
		this.refreshing = true;
		this.invalidate();
		try {
			const list = await this.data.listTodos(this.list.date);
			this.updateList(list);
			this.onRefresh(list);
		} finally {
			this.refreshing = false;
			this.invalidate();
		}
	}

	private async mutateList(mutator: () => Promise<TotuiTodoList>): Promise<void> {
		this.refreshing = true;
		this.invalidate();
		try {
			const list = await mutator();
			this.updateList(list);
			this.onRefresh(list);
		} catch (err) {
			const msg = err instanceof Error ? err.message : String(err);
			this.list = { ...this.list, error: msg };
			this.invalidate();
		} finally {
			this.refreshing = false;
			this.invalidate();
		}
	}

	private async focusExclusive(): Promise<void> {
		const raw = this.rawSelected();
		if (raw === undefined) return;
		const item = this.list.items[raw];
		if (!item?.id) {
			this.list = { ...this.list, error: "Todo has no ID — API may be offline" };
			this.invalidate();
			return;
		}

		if (item.state === "*") {
			await this.mutateList(() => this.data.unfocusTodo(item.id, this.list.date));
			return;
		}

		await this.mutateList(() => this.data.setFocus(item.id, this.list.items, this.list.date));
	}

	private async focusMulti(): Promise<void> {
		const raw = this.rawSelected();
		if (raw === undefined) return;
		const item = this.list.items[raw];
		if (!item?.id) {
			this.list = { ...this.list, error: "Todo has no ID — API may be offline" };
			this.invalidate();
			return;
		}

		await this.mutateList(() => this.data.toggleFocusMulti(item.id, this.list.items, this.list.date));
	}

	private async unfocusAll(): Promise<void> {
		await this.mutateList(() => this.data.clearFocus(this.list.items, this.list.date));
	}

	private async toggleSelected(): Promise<void> {
		const raw = this.rawSelected();
		if (raw === undefined) return;
		const item = this.list.items[raw];
		if (!item?.id) {
			this.list = { ...this.list, error: "Todo has no ID — API may be offline" };
			this.invalidate();
			return;
		}

		await this.mutateList(() => this.data.toggleTodo(item.id, this.list.items, this.list.date));
	}

	render(width: number): string[] {
		if (this.cachedLines && this.cachedWidth === width) return this.cachedLines;

		const th = this.theme;
		const boxW = Math.min(width, this.width);
		const innerW = boxW - 2;
		const pad = (s: string, len: number) => {
			const vis = visibleWidth(s);
			return s + " ".repeat(Math.max(0, len - vis));
		};
		const row = (content: string) => th.fg("border", "│") + pad(content, innerW) + th.fg("border", "│");

		const lines: string[] = [];
		lines.push(th.fg("border", `╭${"─".repeat(innerW)}╮`));
		lines.push(row(` ${formatPanelHeader(this.list, th, innerW - 1)}`));

		if (this.visible.length === 0) {
			lines.push(row(` ${th.fg("dim", `No todos for ${this.list.date}`)}`));
		} else {
			const { done, total } = countDone(this.list.items);
			const status = this.refreshing
				? th.fg("accent", " refreshing…")
				: th.fg("dim", ` ${this.list.source}${this.list.error ? ` · ${this.list.error}` : ""}`);
			lines.push(row(` ${th.fg("muted", `${done}/${total}`)}${status}`));

			const end = Math.min(this.visible.length, this.scroll + this.visibleRows);
			for (let r = 0; r < this.visibleRows; r++) {
				const vi = this.scroll + r;
				if (vi >= this.visible.length) {
					lines.push(row(""));
					continue;
				}
				const raw = this.visible[vi]!;
				const item = this.list.items[raw]!;
				const isFocused = item.state === "*";
				const fold = collapseIndicator(this.list.items, raw);
				const prefix = vi === this.selectedVisible
					? th.fg("accent", "› ")
					: isFocused
						? th.fg("accent", "◆ ")
						: "  ";
				const body = formatTodoLine(item, this.list.items, raw, th, innerW - 6);
				lines.push(row(prefix + fold + body));
			}

			const scrollLabel =
				this.visible.length > this.visibleRows
					? `${this.scroll + 1}-${end} of ${this.visible.length}`
					: `${this.visible.length} items`;
			lines.push(row(` ${th.fg("dim", scrollLabel)}`));
		}

		lines.push(row(` ${th.fg("dim", "↑↓ · sp · f/F · e fold · u · r · esc")}`));
		lines.push(th.fg("border", `╰${"─".repeat(innerW)}╯`));

		this.cachedWidth = width;
		this.cachedLines = lines;
		return lines;
	}

	invalidate(): void {
		this.cachedWidth = undefined;
		this.cachedLines = undefined;
	}
}
