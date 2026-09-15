import { execFile } from "node:child_process";
import type { TotuiDestination, TotuiTodoItem } from "./types.js";

export interface TotuiCliConfig {
	cwd: string;
	command?: string;
	project?: string;
	remote?: string;
	local?: boolean;
}

export type CliRunner = (
	command: string,
	args: string[],
	options: { cwd: string; signal?: AbortSignal },
) => Promise<unknown>;

export const runCli: CliRunner = (command, args, options) => new Promise((resolve, reject) => {
	execFile(command, args, {
		...options, encoding: "utf8", timeout: 30_000, maxBuffer: 10 * 1024 * 1024,
	}, (error, stdout, stderr) => {
		if (error) {
			reject(new Error(`totui CLI failed: ${stderr.trim() || error.message}`));
			return;
		}
		try {
			resolve(JSON.parse(stdout));
		} catch {
			reject(new Error("totui CLI returned invalid JSON; upgrade totui if 'todo context' is unavailable."));
		}
	});
});

export function todayLocal(): string {
	const d = new Date();
	return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

export function describeDestination(d: TotuiDestination): string {
	return d.backend === "remote"
		? `${d.project} on ${d.remote} (${d.server_url})`
		: `local project ${d.project}`;
}

function parseContext(value: unknown): TotuiDestination {
	const d = value as Partial<TotuiDestination> | null;
	if (!d || !["local", "remote"].includes(d.backend ?? "") ||
		typeof d.project !== "string" || !d.project || typeof d.directory !== "string" ||
		(d.backend === "remote" && (typeof d.remote !== "string" || !d.remote || typeof d.server_url !== "string" || !d.server_url))) {
		throw new Error("Invalid totui todo context; upgrade totui. No backend fallback was attempted.");
	}
	return d as TotuiDestination;
}

function backendArgs(d: TotuiDestination): string[] {
	return d.backend === "remote" ? ["--remote", d.remote!] : ["--local"];
}

export function parseItems(value: unknown): TotuiTodoItem[] {
	if (!Array.isArray(value) || !value.every(isItem)) throw new Error("Invalid totui CLI todo list");
	return value;
}

function isItem(value: unknown): value is TotuiTodoItem {
	const item = value as Partial<TotuiTodoItem> | null;
	return !!item && typeof item.id === "string" && typeof item.content === "string" &&
		typeof item.state === "string" && Number.isInteger(item.indent_level) && item.indent_level! >= 0;
}

export class TotuiCliClient {
	private resolved?: Promise<TotuiDestination>;
	private readonly lifetime = new AbortController();

	close(): void {
		this.lifetime.abort();
	}

	constructor(readonly config: TotuiCliConfig, private readonly runner: CliRunner = runCli) {
		if (config.local && config.remote) throw new Error("Choose either totui-local or totui-remote, not both.");
	}

	private run(args: string[], signal?: AbortSignal): Promise<unknown> {
		signal?.throwIfAborted();
		this.lifetime.signal.throwIfAborted();
		return this.runner(this.config.command ?? "totui", args, {
			cwd: this.config.cwd,
			signal: signal ? AbortSignal.any([signal, this.lifetime.signal]) : this.lifetime.signal,
		});
	}

	async context(project?: string, signal?: AbortSignal): Promise<TotuiDestination> {
		signal?.throwIfAborted();
		this.lifetime.signal.throwIfAborted();
		if (!this.resolved) {
			const flags = this.config.local ? ["--local"] : this.config.remote ? ["--remote", this.config.remote] : [];
			this.resolved = this.run([
				...flags, "todo", "context", ...(this.config.project ? ["--project", this.config.project] : []),
			], signal).then(parseContext).then(d => {
				if ((this.config.remote && (d.backend !== "remote" || d.remote !== this.config.remote)) ||
					(this.config.local && d.backend !== "local")) throw new Error("totui context did not honor the selected backend");
				return d;
			}).catch(error => {
				this.resolved = undefined;
				throw error;
			});
		}
		const d = await this.resolved;
		signal?.throwIfAborted();
		if (!project || project === d.project) return d;
		const selected = parseContext(await this.run([...backendArgs(d), "todo", "context", "--project", project], signal));
		if (selected.backend !== d.backend || selected.remote !== d.remote || selected.server_url !== d.server_url || selected.project !== project) {
			throw new Error("totui context changed backend or did not honor the requested project");
		}
		return selected;
	}

	async callTool(
		name: string,
		params: Record<string, unknown>,
		signal?: AbortSignal,
		onDestination?: (destination: TotuiDestination) => void,
	): Promise<{ destination: TotuiDestination; date: string; result: unknown }> {
		const destination = await this.context(params.project as string | undefined, signal);
		const date = typeof params.date === "string" ? params.date : todayLocal();
		const prefix = [...backendArgs(destination), "todo"];
		const scope = ["--project", destination.project, "--date", date];
		const { id, project: _project, date: _date, ...fields } = params;
		const payload = JSON.stringify(fields);
		const run = (args: string[]) => this.run([...prefix, ...args, ...scope], signal);
		const requireId = () => {
			if (typeof id !== "string" || !id || id.startsWith("-")) throw new Error("Todo ID is required");
			return id;
		};
		let result: unknown;
		switch (name) {
			case "list_todos": result = parseItems(await run(["list"])); break;
			case "list_projects":
				result = await this.run([...prefix, "projects"], signal);
				if (!Array.isArray(result) || !result.every(p => typeof p === "string")) throw new Error("Invalid totui CLI projects list");
				break;
			case "create_todo":
				onDestination?.(destination);
				result = await run(["create", "--json", payload]);
				break;
			case "update_todo":
				requireId();
				onDestination?.(destination);
				result = await run(["update", id as string, "--json", payload]);
				break;
			case "delete_todo":
				requireId();
				onDestination?.(destination);
				result = await run(["delete", id as string]);
				break;
			case "mark_complete": {
				const current = await run(["get", requireId()]);
				if (!isItem(current)) throw new Error("Invalid totui CLI todo");
				onDestination?.(destination);
				result = await run(["update", id as string, "--json", JSON.stringify({ state: current.state === "x" ? " " : "x" })]);
				break;
			}
			default: throw new Error(`Unknown totui operation: ${name}`);
		}
		return { destination, date, result };
	}
}
