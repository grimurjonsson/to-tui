import { parseItems, todayLocal, TotuiCliClient } from "./client.js";
import type { TotuiTodoItem, TotuiTodoList } from "./types.js";

export class TotuiDataSource {
	constructor(readonly client: TotuiCliClient) {}

	async listTodos(date = todayLocal()): Promise<TotuiTodoList> {
		let destination;
		try {
			destination = await this.client.context();
			const response = await this.client.callTool("list_todos", { date });
			return { date, items: parseItems(response.result), source: "cli", destination };
		} catch (err) {
			return { date, items: [], source: "error", destination, error: err instanceof Error ? err.message : String(err) };
		}
	}

	async updateTodoState(id: string, state: string, date = todayLocal()): Promise<void> {
		await this.client.callTool("update_todo", { id, state, date });
	}

	async setFocus(id: string, items: TotuiTodoItem[], date = todayLocal()): Promise<TotuiTodoList> {
		const target = items.find(t => t.id === id);
		if (!target) throw new Error("Todo not found");
		for (const item of items) {
			if (item.state === "*" && item.id !== id) await this.updateTodoState(item.id, " ", date);
		}
		if (target.state !== "*") await this.updateTodoState(id, "*", date);
		return this.listTodos(date);
	}

	async clearFocus(items: TotuiTodoItem[], date = todayLocal()): Promise<TotuiTodoList> {
		for (const item of items) {
			if (item.state === "*") await this.updateTodoState(item.id, " ", date);
		}
		return this.listTodos(date);
	}

	async unfocusTodo(id: string, date = todayLocal()): Promise<TotuiTodoList> {
		await this.updateTodoState(id, " ", date);
		return this.listTodos(date);
	}

	async toggleFocusMulti(id: string, items: TotuiTodoItem[], date = todayLocal()): Promise<TotuiTodoList> {
		const target = items.find(t => t.id === id);
		if (!target) throw new Error("Todo not found");
		await this.updateTodoState(id, target.state === "*" ? " " : "*", date);
		return this.listTodos(date);
	}

	async toggleTodo(id: string, items: TotuiTodoItem[], date = todayLocal()): Promise<TotuiTodoList> {
		if (!items.some(t => t.id === id)) throw new Error("Todo not found");
		await this.client.callTool("mark_complete", { id, date });
		return this.listTodos(date);
	}
}
