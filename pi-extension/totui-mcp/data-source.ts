import type { TotuiDataConfig, TotuiTodoItem, TotuiTodoList } from "./types.js";
import { checkApiHealth } from "./api-server.js";

function todayLocal(): string {
	const d = new Date();
	const y = d.getFullYear();
	const m = String(d.getMonth() + 1).padStart(2, "0");
	const day = String(d.getDate()).padStart(2, "0");
	return `${y}-${m}-${day}`;
}

export class TotuiDataSource {
	private readonly config: TotuiDataConfig;

	constructor(config: TotuiDataConfig) {
		this.config = config;
	}

	get project(): string {
		return this.config.project;
	}

	get apiBaseUrl(): string {
		return this.config.apiBaseUrl;
	}

	private apiUrl(pathname: string, params?: Record<string, string>): string {
		const url = new URL(
			pathname,
			this.config.apiBaseUrl.endsWith("/") ? this.config.apiBaseUrl : `${this.config.apiBaseUrl}/`,
		);
		if (params) {
			for (const [k, v] of Object.entries(params)) url.searchParams.set(k, v);
		}
		return url.toString();
	}

	async listTodos(date = todayLocal()): Promise<TotuiTodoList> {
		try {
			const res = await fetch(this.apiUrl("/api/todos", { project: this.config.project, date }), {
				signal: AbortSignal.timeout(3000),
			});
			if (res.ok) {
				const body = (await res.json()) as { date: string; items: TotuiTodoItem[] };
				return { date: body.date, items: body.items, source: "api" };
			}
			const text = await res.text().catch(() => "");
			return {
				date,
				items: [],
				source: "error",
				error: `API ${res.status}${text ? `: ${text.slice(0, 120)}` : ""}`,
			};
		} catch (err) {
			const msg = err instanceof Error ? err.message : String(err);
			return { date, items: [], source: "error", error: msg };
		}
	}

	async updateTodoState(id: string, state: string, date = todayLocal()): Promise<void> {
		const res = await fetch(this.apiUrl(`/api/todos/${id}`, { project: this.config.project, date }), {
			method: "PATCH",
			headers: { "Content-Type": "application/json" },
			body: JSON.stringify({ state }),
			signal: AbortSignal.timeout(3000),
		});

		if (!res.ok) {
			const text = await res.text().catch(() => "");
			throw new Error(`Update failed (${res.status})${text ? `: ${text}` : ""}`);
		}
	}

	/** Set exclusive focus: selected item → [*], clear [*] on all others. */
	async setFocus(id: string, items: TotuiTodoItem[], date = todayLocal()): Promise<TotuiTodoList> {
		const target = items.find((t) => t.id === id);
		if (!target) throw new Error("Todo not found");

		for (const item of items) {
			if (item.state === "*" && item.id !== id) {
				await this.updateTodoState(item.id, " ", date);
			}
		}

		if (target.state !== "*") {
			await this.updateTodoState(id, "*", date);
		}

		return this.listTodos(date);
	}

	/** Clear all in-progress / focused items. */
	async clearFocus(items: TotuiTodoItem[], date = todayLocal()): Promise<TotuiTodoList> {
		for (const item of items) {
			if (item.state === "*") {
				await this.updateTodoState(item.id, " ", date);
			}
		}
		return this.listTodos(date);
	}

	/** Clear focus on one item ([*] → pending). */
	async unfocusTodo(id: string, date = todayLocal()): Promise<TotuiTodoList> {
		await this.updateTodoState(id, " ", date);
		return this.listTodos(date);
	}

	/** Toggle focus on one item without clearing other [*] items. */
	async toggleFocusMulti(id: string, items: TotuiTodoItem[], date = todayLocal()): Promise<TotuiTodoList> {
		const target = items.find((t) => t.id === id);
		if (!target) throw new Error("Todo not found");

		if (target.state === "*") {
			await this.updateTodoState(id, " ", date);
		} else {
			await this.updateTodoState(id, "*", date);
		}

		return this.listTodos(date);
	}

	async toggleTodo(id: string, items: TotuiTodoItem[], date = todayLocal()): Promise<TotuiTodoList> {
		const item = items.find((t) => t.id === id);
		if (!item) throw new Error("Todo not found");

		const nextState = item.state === "x" ? " " : "x";

		const res = await fetch(this.apiUrl(`/api/todos/${id}`, { project: this.config.project, date }), {
			method: "PATCH",
			headers: { "Content-Type": "application/json" },
			body: JSON.stringify({ state: nextState }),
			signal: AbortSignal.timeout(3000),
		});

		if (!res.ok) {
			const text = await res.text().catch(() => "");
			throw new Error(`Toggle failed (${res.status})${text ? `: ${text}` : ""}`);
		}

		return this.listTodos(date);
	}

	async checkApiHealth(): Promise<boolean> {
		return checkApiHealth(this.config.apiBaseUrl);
	}
}

export function createDataSourceFromEnv(flags: {
	apiUrl?: string;
	project?: string;
}): TotuiDataSource {
	return new TotuiDataSource({
		apiBaseUrl: flags.apiUrl || process.env.TOTUI_API_URL || "http://127.0.0.1:3000",
		project: flags.project || process.env.TOTUI_PROJECT || "default",
	});
}
