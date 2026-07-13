import { spawn } from "node:child_process";

function sleep(ms: number): Promise<void> {
	return new Promise((resolve) => setTimeout(resolve, ms));
}

export function parseApiPort(baseUrl: string): number {
	try {
		const u = new URL(baseUrl);
		if (u.port) return Number.parseInt(u.port, 10);
		return u.protocol === "https:" ? 443 : 80;
	} catch {
		return 3000;
	}
}

export async function checkApiHealth(baseUrl: string, timeoutMs = 800): Promise<boolean> {
	try {
		const url = new URL("/api/health", baseUrl.endsWith("/") ? baseUrl : `${baseUrl}/`);
		const res = await fetch(url, { signal: AbortSignal.timeout(timeoutMs) });
		return res.ok;
	} catch {
		return false;
	}
}

function runServeStart(command: string, port: number): Promise<void> {
	return new Promise((resolve, reject) => {
		const child = spawn(command, ["serve", "start", "--port", String(port)], {
			stdio: ["ignore", "pipe", "pipe"],
		});

		let stderr = "";
		child.stderr?.on("data", (chunk: Buffer) => {
			stderr += chunk.toString();
		});

		child.on("error", reject);
		child.on("close", (code) => {
			if (code === 0) resolve();
			else reject(new Error(stderr.trim() || `totui serve start exited ${code}`));
		});
	});
}

export async function ensureApiServer(options: {
	baseUrl: string;
	command: string;
	autoStart: boolean;
	maxWaitMs?: number;
}): Promise<{ ok: boolean; error?: string }> {
	if (await checkApiHealth(options.baseUrl)) {
		return { ok: true };
	}

	if (!options.autoStart) {
		const port = parseApiPort(options.baseUrl);
		return {
			ok: false,
			error: `API offline — run: ${options.command} serve start --port ${port}`,
		};
	}

	const port = parseApiPort(options.baseUrl);
	try {
		await runServeStart(options.command, port);
	} catch (err) {
		const msg = err instanceof Error ? err.message : String(err);
		return { ok: false, error: `Failed to start API: ${msg}` };
	}

	const deadline = Date.now() + (options.maxWaitMs ?? 8000);
	while (Date.now() < deadline) {
		if (await checkApiHealth(options.baseUrl)) {
			return { ok: true };
		}
		await sleep(300);
	}

	return { ok: false, error: `API not responding on port ${port}` };
}
