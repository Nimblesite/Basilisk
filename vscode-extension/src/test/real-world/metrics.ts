// Implements [VSIX-REALWORLD-RESOURCES]. See docs/specs/VSIX-REAL-WORLD-SPEC.md#VSIX-REALWORLD-RESOURCES
/**
 * OS-level resource measurement for the real-world e2e suites.
 *
 * Samples the basilisk LSP server process (RSS + cumulative CPU time) from
 * OUTSIDE the process — the honest external view, not self-reported stats —
 * plus the extension host's own RSS. The {@link ResourceMonitor} turns those
 * samples into hard assertions: peak memory budgets, leak ceilings, CPU
 * settle-after-analysis, and server-PID stability (a PID change mid-journey
 * means the server crashed and restarted, which is a failure, not a detail).
 */

import { delay } from '../../timeouts';
import * as assert from 'assert';
import { execFileSync } from 'child_process';
import * as fs from 'fs';

/** One point-in-time reading of a process. */
export interface ProcessSample {
    /** Resident set size, bytes. */
    readonly rssBytes: number;
    /** Cumulative CPU time (user+system), milliseconds. */
    readonly cpuMs: number;
    /** Wall-clock timestamp of the sample (ms since epoch). */
    readonly atMs: number;
}

/** Budgets a repo journey must stay inside (from the corpus manifest). */
export interface ResourceBudgets {
    readonly maxServerRssMb: number;
    readonly maxServerLeakMb: number;
    readonly maxExtHostRssMb: number;
    readonly maxIdleCpuPercent: number;
    readonly cpuSettleTimeoutMs: number;
}

const BYTES_PER_KB = 1024;
const BYTES_PER_MB = 1024 * 1024;
const MS_PER_SECOND = 1000;
const DEFAULT_LINUX_CLK_TCK = 100;
const CPU_WINDOW_MS = 2_000;
const SETTLED_WINDOWS_REQUIRED = 2;
/** Max time to lock onto a stable server PID at suite start. */
const PID_LOCK_TIMEOUT_MS = 30_000;
/** Gap between the two PID reads that establish startup stability. */
const PID_STABILITY_GAP_MS = 750;

function sampleWindows(pid: number): ProcessSample {
    // [long] casts keep the output culture-invariant (no decimal commas).
    const script =
        `$p = Get-Process -Id ${pid} -ErrorAction Stop; ` +
        `Write-Output ("{0}|{1}" -f $p.WorkingSet64, [long][math]::Round($p.TotalProcessorTime.TotalMilliseconds))`;
    const out = execFileSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', script], {
        encoding: 'utf8',
    });
    const [rss, cpu] = out.trim().split('|');
    return { rssBytes: Number(rss), cpuMs: Number(cpu), atMs: Date.now() };
}

let cachedClkTck: number | undefined;

function linuxClkTck(): number {
    if (cachedClkTck === undefined) {
        try {
            cachedClkTck = Number(execFileSync('getconf', ['CLK_TCK'], { encoding: 'utf8' }).trim());
        } catch {
            cachedClkTck = DEFAULT_LINUX_CLK_TCK;
        }
        if (!Number.isFinite(cachedClkTck) || cachedClkTck <= 0) {
            cachedClkTck = DEFAULT_LINUX_CLK_TCK;
        }
    }
    return cachedClkTck;
}

function sampleLinux(pid: number): ProcessSample {
    const status = fs.readFileSync(`/proc/${pid}/status`, 'utf8');
    const rssLine = status.split('\n').find((l) => l.startsWith('VmRSS:')) ?? '';
    const rssKb = Number(rssLine.replace('VmRSS:', '').replace('kB', '').trim());
    // Fields after the ")" of the command name: utime is index 11, stime 12.
    const stat = fs.readFileSync(`/proc/${pid}/stat`, 'utf8');
    const rest = stat.slice(stat.lastIndexOf(')') + 2).split(' ');
    const ticks = Number(rest[11]) + Number(rest[12]);
    return {
        rssBytes: rssKb * BYTES_PER_KB,
        cpuMs: (ticks * MS_PER_SECOND) / linuxClkTck(),
        atMs: Date.now(),
    };
}

/** Parse a ps(1) cputime value: `[[dd-]hh:]mm:ss[.cc]`. */
export function parsePsCpuTime(raw: string): number {
    let days = 0;
    let rest = raw.trim();
    const dash = rest.indexOf('-');
    if (dash !== -1) {
        days = Number(rest.slice(0, dash));
        rest = rest.slice(dash + 1);
    }
    const parts = rest.split(':').map(Number);
    const seconds = parts.reduce((total, part) => total * 60 + part, 0);
    const hoursFromDays = 24;
    return Math.round(((days * hoursFromDays * 3600) + seconds) * MS_PER_SECOND);
}

function sampleDarwin(pid: number): ProcessSample {
    const out = execFileSync('ps', ['-o', 'rss=,cputime=', '-p', String(pid)], { encoding: 'utf8' });
    // Split on runs of spaces without a regex (see CLAUDE.md).
    const fields = out.trim().split(' ').filter((f) => f.length > 0);
    return {
        rssBytes: Number(fields[0]) * BYTES_PER_KB,
        cpuMs: parsePsCpuTime(fields[1] ?? '0:00'),
        atMs: Date.now(),
    };
}

/** Sample RSS + cumulative CPU of an arbitrary live process. Throws if dead. */
export function sampleProcess(pid: number): ProcessSample {
    if (process.platform === 'win32') { return sampleWindows(pid); }
    if (process.platform === 'linux') { return sampleLinux(pid); }
    return sampleDarwin(pid);
}

function toMb(bytes: number): number {
    return Math.round((bytes / BYTES_PER_MB) * 10) / 10;
}

/** CPU utilisation (%) between two samples. May exceed 100 on multi-core. */
export function cpuPercentBetween(prev: ProcessSample, next: ProcessSample): number {
    const wallMs = next.atMs - prev.atMs;
    if (wallMs <= 0) { return 0; }
    return ((next.cpuMs - prev.cpuMs) / wallMs) * 100;
}

/**
 * Tracks the basilisk server + extension host across a journey and asserts
 * every budget in the corpus manifest. Every assert*() call is a real gate:
 * a budget breach fails the suite.
 */
export class ResourceMonitor {
    private readonly resolvePid: () => number;
    private readonly budgets: ResourceBudgets;
    private readonly repo: string;
    private readonly initialPid: number;
    private peakRssBytes = 0;
    private peakExtHostRssBytes = 0;
    private lastSample: ProcessSample;

    private constructor(init: {
        resolvePid: () => number;
        budgets: ResourceBudgets;
        repo: string;
        pid: number;
        first: ProcessSample;
    }) {
        this.resolvePid = init.resolvePid;
        this.budgets = init.budgets;
        this.repo = init.repo;
        this.initialPid = init.pid;
        this.lastSample = init.first;
        this.peakRssBytes = init.first.rssBytes;
        this.peakExtHostRssBytes = process.memoryUsage().rss;
    }

    /**
     * Lock onto the server process once it is STABLE: the PID must resolve
     * and sample successfully twice, {@link PID_STABILITY_GAP_MS} apart,
     * without changing. Client startup can briefly race the spawn; a server
     * that dies or restarts AFTER this lock is a hard suite failure.
     */
    public static async create(
        resolvePid: () => number,
        budgets: ResourceBudgets,
        repo: string,
    ): Promise<ResourceMonitor> {
        const deadline = Date.now() + PID_LOCK_TIMEOUT_MS;
        let lastError = 'no attempt made';
        while (Date.now() < deadline) {
            try {
                const pid = resolvePid();
                const first = sampleProcess(pid);
                await delay(PID_STABILITY_GAP_MS);
                if (resolvePid() === pid) {
                    return new ResourceMonitor({ resolvePid, budgets, repo, pid, first });
                }
                lastError = `server PID changed during startup (was ${pid})`;
            } catch (error: unknown) {
                lastError = error instanceof Error ? error.message : String(error);
            }
            await delay(PID_STABILITY_GAP_MS);
        }
        assert.fail(
            `[${repo}] could not lock onto a stable basilisk server process ` +
            `within ${PID_LOCK_TIMEOUT_MS}ms — last error: ${lastError}`,
        );
    }

    /** The PID being measured (asserts the server never restarted). */
    public pid(phase: string): number {
        const current = this.resolvePid();
        assert.strictEqual(
            current, this.initialPid,
            `[${this.repo}] ${phase}: basilisk server PID changed ` +
            `${this.initialPid} → ${current} — the server crashed or restarted mid-journey`,
        );
        return current;
    }

    /** Take a sample, update peaks, and return it. */
    public sample(phase: string): ProcessSample {
        const s = sampleProcess(this.pid(phase));
        assert.ok(
            Number.isFinite(s.rssBytes) && s.rssBytes > 0,
            `[${this.repo}] ${phase}: RSS sample is not a positive number (${s.rssBytes})`,
        );
        assert.ok(
            Number.isFinite(s.cpuMs) && s.cpuMs >= 0,
            `[${this.repo}] ${phase}: CPU-time sample is not a non-negative number (${s.cpuMs})`,
        );
        this.peakRssBytes = Math.max(this.peakRssBytes, s.rssBytes);
        this.peakExtHostRssBytes = Math.max(this.peakExtHostRssBytes, process.memoryUsage().rss);
        this.lastSample = s;
        return s;
    }

    /** Assert current AND peak server RSS + extension-host RSS are in budget. */
    public assertMemoryWithinBudget(phase: string): void {
        const s = this.sample(phase);
        const maxBytes = this.budgets.maxServerRssMb * BYTES_PER_MB;
        assert.ok(
            s.rssBytes <= maxBytes,
            `[${this.repo}] ${phase}: basilisk server RSS ${toMb(s.rssBytes)} MB ` +
            `exceeds the ${this.budgets.maxServerRssMb} MB budget`,
        );
        assert.ok(
            this.peakRssBytes <= maxBytes,
            `[${this.repo}] ${phase}: basilisk server PEAK RSS ${toMb(this.peakRssBytes)} MB ` +
            `exceeds the ${this.budgets.maxServerRssMb} MB budget`,
        );
        const extHostMax = this.budgets.maxExtHostRssMb * BYTES_PER_MB;
        assert.ok(
            this.peakExtHostRssBytes <= extHostMax,
            `[${this.repo}] ${phase}: extension host PEAK RSS ${toMb(this.peakExtHostRssBytes)} MB ` +
            `exceeds the ${this.budgets.maxExtHostRssMb} MB budget`,
        );
    }

    /** Assert server RSS grew at most maxServerLeakMb since `baseline`. */
    public assertNoLeakSince(baseline: ProcessSample, phase: string): void {
        const s = this.sample(phase);
        const growthMb = toMb(s.rssBytes - baseline.rssBytes);
        assert.ok(
            growthMb <= this.budgets.maxServerLeakMb,
            `[${this.repo}] ${phase}: basilisk server RSS grew ${growthMb} MB since baseline ` +
            `(${toMb(baseline.rssBytes)} → ${toMb(s.rssBytes)} MB) — ` +
            `leak budget is ${this.budgets.maxServerLeakMb} MB`,
        );
    }

    /**
     * Assert the server's CPU settles to idle: two consecutive ~2s windows
     * below maxIdleCpuPercent, within cpuSettleTimeoutMs. Catches busy-loop
     * and re-analysis-storm regressions that a one-shot sample would miss.
     */
    public async assertCpuSettles(phase: string): Promise<number> {
        const deadline = Date.now() + this.budgets.cpuSettleTimeoutMs;
        let prev = this.sample(phase);
        let calmWindows = 0;
        let lastPct = Number.POSITIVE_INFINITY;
        while (Date.now() < deadline) {
            await delay(CPU_WINDOW_MS);
            const next = this.sample(phase);
            lastPct = cpuPercentBetween(prev, next);
            prev = next;
            calmWindows = lastPct <= this.budgets.maxIdleCpuPercent ? calmWindows + 1 : 0;
            if (calmWindows >= SETTLED_WINDOWS_REQUIRED) {
                return lastPct;
            }
        }
        assert.fail(
            `[${this.repo}] ${phase}: basilisk server CPU never settled below ` +
            `${this.budgets.maxIdleCpuPercent}% for ${SETTLED_WINDOWS_REQUIRED} consecutive ` +
            `windows within ${this.budgets.cpuSettleTimeoutMs}ms (last window: ${lastPct.toFixed(1)}%)`,
        );
    }

    /** Peak server RSS seen so far (bytes) — for reporting in assertions. */
    public peakRss(): number {
        return this.peakRssBytes;
    }

    /** Most recent sample without re-sampling. */
    public last(): ProcessSample {
        return this.lastSample;
    }

    /** Measured numbers for the run — written to a calibration report file. */
    public report(): Record<string, number> {
        return {
            peakServerRssMb: toMb(this.peakRssBytes),
            lastServerRssMb: toMb(this.lastSample.rssBytes),
            peakExtHostRssMb: toMb(this.peakExtHostRssBytes),
        };
    }
}
