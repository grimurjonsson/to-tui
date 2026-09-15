export interface TotuiTodoItem {
	id: string;
	content: string;
	state: string;
	indent_level: number;
	parent_id?: string | null;
	due_date?: string | null;
	description?: string | null;
}

export interface TotuiDestination {
	backend: "local" | "remote";
	project: string;
	remote?: string | null;
	server_url?: string | null;
	directory: string;
	folder?: string | null;
}

export interface TotuiTodoList {
	date: string;
	items: TotuiTodoItem[];
	source: "cli" | "error";
	destination?: TotuiDestination;
	error?: string;
}
