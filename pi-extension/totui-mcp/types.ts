export interface TotuiTodoItem {
	id: string;
	content: string;
	state: string;
	indent_level: number;
	parent_id?: string | null;
	due_date?: string | null;
	description?: string | null;
}

export interface TotuiTodoList {
	date: string;
	items: TotuiTodoItem[];
	source: "api" | "empty" | "error";
	error?: string;
}

export interface TotuiDataConfig {
	apiBaseUrl: string;
	project: string;
}
