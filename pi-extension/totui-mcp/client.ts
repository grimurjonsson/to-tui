import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";
import type { CallToolResult } from "@modelcontextprotocol/sdk/types.js";

export interface TotuiMcpConfig {
	command: string;
	args: string[];
}

export class TotuiMcpClient {
	private client: Client | null = null;
	private transport: StdioClientTransport | null = null;
	private connecting: Promise<void> | null = null;
	private readonly config: TotuiMcpConfig;

	constructor(config: TotuiMcpConfig) {
		this.config = config;
	}

	async connect(): Promise<void> {
		if (this.client) return;
		if (this.connecting) return this.connecting;

		this.connecting = this.doConnect();
		try {
			await this.connecting;
		} finally {
			this.connecting = null;
		}
	}

	private async doConnect(): Promise<void> {
		this.transport = new StdioClientTransport({
			command: this.config.command,
			args: this.config.args,
			stderr: "pipe",
		});
		this.client = new Client({ name: "pi-totui-mcp", version: "0.1.0" });
		await this.client.connect(this.transport);
	}

	async callTool(name: string, args: Record<string, unknown>, signal?: AbortSignal): Promise<CallToolResult> {
		await this.connect();
		if (!this.client) throw new Error("totui-mcp client not connected");

		if (signal?.aborted) throw new Error("Cancelled");

		return this.client.callTool({ name, arguments: args }, undefined, { signal });
	}

	async close(): Promise<void> {
		if (this.client) {
			await this.client.close();
			this.client = null;
		}
		this.transport = null;
	}
}

export function mcpResultToText(result: CallToolResult): string {
	const parts: string[] = [];
	for (const block of result.content ?? []) {
		if (block.type === "text") parts.push(block.text);
		else parts.push(JSON.stringify(block));
	}
	return parts.join("\n") || "(empty response)";
}
