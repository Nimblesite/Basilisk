// Tests for [LSPDEBUG]. See docs/specs/LSP-DEBUG-INTEGRATION-SPEC.md#LSPDEBUG
/* eslint-disable max-lines */
/**
 * Debug Integration E2E Tests for the Basilisk VS Code Extension.
 *
 * These tests exercise REAL debug sessions by:
 *   1. Asking the LSP to spawn debugpy via basilisk.startDebugSession
 *   2. Starting actual VS Code debug sessions with vscode.debug.startDebugging
 *   3. Setting breakpoints, stepping through code, and asserting variable values
 *   4. Evaluating watch expressions and verifying results
 *   5. Testing error handling (missing debugpy, missing Python)
 *
 * Prerequisites:
 *   - The `basilisk` binary must be built: `cargo build -p basilisk-cli`
 *   - Python 3 must be available on PATH or in a workspace venv
 *   - `debugpy` must be installed: `pip install debugpy`
 */

import * as assert from 'assert';
import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import * as os from 'os';
import * as net from 'net';
import { execFileSync } from 'child_process';

import { findBasiliskBinary } from './test-helpers';
import { getStore } from '../../extension';
import { currentStoppedFrameId, evaluateInDebugSession } from '../../dap-evaluate';
import { debugOutputCursor, debugOutputSince } from '../../dap-output';
import { applyDebugConfigDefaults } from '../../debug-adapter';

const EXTENSION_ID = 'Nimblesite.basilisk';

/** Maximum time (ms) to wait for the LSP server to fully start. */
const SERVER_START_WAIT_MS = 10_000;

/** Maximum time (ms) for a debug session to start. */
const DEBUG_SESSION_TIMEOUT_MS = 15_000;

/** Maximum time (ms) to wait for a stopped event (breakpoint/step). */
const STOPPED_EVENT_TIMEOUT_MS = 10_000;

/** Path to the debug stepping fixture. */
const FIXTURE_DIR = path.resolve(__dirname, '../../src/test/fixtures');
const STEPPING_FIXTURE = path.join(FIXTURE_DIR, 'debug_stepping.py');

/** Shape of the package.json fields we inspect in tests. */
interface DebuggerConfigAttributes {
    launch?: { properties?: Record<string, unknown> };
    attach?: { properties?: Record<string, unknown> };
}

interface DebuggerContribution {
    type: string;
    label?: string;
    configurationAttributes?: DebuggerConfigAttributes;
}

interface PackageJSON {
    contributes?: {
        debuggers?: DebuggerContribution[];
    };
}

/** Timeout (ms) for subprocess commands (binary/python detection). */
const SUBPROCESS_TIMEOUT_MS = 5_000;

/** Timeout (ms) for TCP port checks. */
const PORT_CHECK_TIMEOUT_MS = 3_000;

/** Short timeout (ms) for port-closed verification. */
const PORT_CLOSED_CHECK_MS = 1_000;

/** Maximum stack trace levels to request from DAP. */
const MAX_STACK_LEVELS = 20;

/** Polling interval (ms) for stop detection. */
const STOP_POLL_INTERVAL_MS = 100;

/** Short settle time (ms) after debug session stops. */
const SESSION_SETTLE_MS = 500;

/** Timeout (ms) for DAP handshake. */
const DAP_HANDSHAKE_TIMEOUT_MS = 5_000;

/** Timeout (ms) for individual debug tests. */
const DEBUG_TEST_TIMEOUT_MS = 30_000;

/** Timeout (ms) for the loop/accumulate test (more stepping). */
const LOOP_TEST_TIMEOUT_MS = 45_000;

/** Timeout (ms) to wait for debug session end. */
const SESSION_END_WAIT_MS = 15_000;

/** Max poll iterations waiting for debug session to clear. */
const SESSION_CLEAR_MAX_POLLS = 20;

// ── Fixture line numbers ────────────────────────────────────────────────────
// These refer to 1-based line numbers in debug_stepping.py.

/** Line: `x = 10` in arithmetic(). */
const ARITH_X_LINE = 11;
/** Line: `y = 20` in arithmetic(). */
const ARITH_Y_LINE = 12;
/** Line: `z = x + y` in arithmetic(). */
const ARITH_Z_LINE = 13;
/** Line: `w = z * 2` in arithmetic(). */
const ARITH_W_LINE = 14;
/** Line: `result = w - 5` in arithmetic(). */
const ARITH_RESULT_LINE = 15;
/** Line: `return result` in arithmetic(). */
const ARITH_RETURN_LINE = 16;

/** Line: `greeting = "hello"` in string_ops(). */
const STRING_OPS_START_LINE = 21;
/** Line: `message = ...` in string_ops(). */
const STRING_OPS_MESSAGE_LINE = 23;

/** Line: `items = [1, 2, 3]` in list_ops(). */
const LIST_OPS_START_LINE = 31;

/** Line: `data = {"a": 1, "b": 2}` in dict_ops(). */
const DICT_OPS_START_LINE = 41;

/** Line: `a = 5` in nested_call(). */
const NESTED_CALL_START_LINE = 51;

/** Line: `result = n * 2` in double(). */
const DOUBLE_RESULT_LINE = 59;

/** Line: `total = 0` in loop_and_accumulate(). */
const LOOP_START_LINE = 65;

/** Line: `x = 42` in conditional_branches(). */
const COND_START_LINE = 74;

/** Line: `caught = False` in exception_handling(). */
const EXCEPT_START_LINE = 86;

/** Line: `an_int = 42` in type_variety(). */
const TYPE_VARIETY_START_LINE = 98;

/** Line: `p = Point(3, 4)` in class_instance(). */
const CLASS_INSTANCE_START_LINE = 119;

// ── Large-heap memory evaluation-budget fixture (written at test time) ───────

/** Line `anchor_start = 1` in the generated bigheap_main.py (tracemalloc start). */
const BIGHEAP_START_LINE = 4;
/** Line `anchor_ready = len(keep)` in bigheap_main.py (snapshot point). */
const BIGHEAP_READY_LINE = 6;
/** Overall budget for the large-heap snapshot test (heap build + round-trip). */
const LARGE_HEAP_MEM_TIMEOUT_MS = 90_000;
/** Wait for the traced ~600k-item heap build between the two breakpoints. */
const HEAP_BUILD_STOP_TIMEOUT_MS = 45_000;
/** pydevd's default PYDEVD_WARN_EVALUATION_TIMEOUT (seconds). */
const PYDEVD_WARN_TIMEOUT_SECS = 3;
/** The stable core of pydevd's evaluation-stall warning text. */
const PYDEVD_STALL_WARNING = 'did not finish after';

// ── Helpers ──────────────────────────────────────────────────────────────────

/**
 * Check if debugpy is installed in the system Python.
 */
function isDebugpyInstalled(): boolean {
    for (const python of ['python3', 'python']) {
        try {
            execFileSync(python, ['-c', 'import debugpy'], {
                timeout: SUBPROCESS_TIMEOUT_MS,
                stdio: 'pipe',
            });
            return true;
        } catch {
            // try next
        }
    }
    return false;
}

/**
 * Find a working Python 3 interpreter.
 */
function findPython(): string | undefined {
    for (const python of ['python3', 'python']) {
        try {
            execFileSync(python, ['--version'], { timeout: SUBPROCESS_TIMEOUT_MS, stdio: 'pipe' });
            return python;
        } catch {
            // try next
        }
    }
    return undefined;
}

/**
 * Attempt a TCP connection to verify a port is accepting connections.
 */
async function checkPortListening(host: string, port: number, timeoutMs = PORT_CHECK_TIMEOUT_MS): Promise<boolean> {
    return new Promise((resolve) => {
        const socket = new net.Socket();
        const timer = setTimeout(() => {
            socket.destroy();
            resolve(false);
        }, timeoutMs);
        socket.connect(port, host, () => {
            clearTimeout(timer);
            socket.destroy();
            resolve(true);
        });
        socket.on('error', () => {
            clearTimeout(timer);
            socket.destroy();
            resolve(false);
        });
    });
}

/**
 * Send a basilisk.startDebugSession command through the LSP.
 */
async function startDebugSession(
    pythonOverride?: string
): Promise<{ host: string; port: number; sessionId: string }> {
    const result = await vscode.commands.executeCommand(
        'basilisk.startDebugSession',
        { python: pythonOverride ?? null }
    );
    return result as { host: string; port: number; sessionId: string };
}

/**
 * Send a basilisk.stopDebugSession command through the LSP.
 */
async function stopDebugSession(sessionId: string): Promise<{ stopped: boolean }> {
    const result = await vscode.commands.executeCommand(
        'basilisk.stopDebugSession',
        { sessionId }
    );
    return result as { stopped: boolean };
}

/**
 * Wait for the debug session to be fully started.
 */
async function waitForDebugSessionStart(timeoutMs: number = DEBUG_SESSION_TIMEOUT_MS): Promise<vscode.DebugSession> {
    return new Promise((resolve, reject) => {
        const timer = setTimeout(() => {
            disposable.dispose();
            reject(new Error(`Debug session did not start within ${timeoutMs}ms`));
        }, timeoutMs);

        const disposable = vscode.debug.onDidStartDebugSession((session) => {
            clearTimeout(timer);
            disposable.dispose();
            resolve(session);
        });
    });
}

/**
 * Wait for the debug session to terminate.
 */
async function waitForDebugSessionEnd(timeoutMs: number = DEBUG_SESSION_TIMEOUT_MS): Promise<void> {
    return new Promise((resolve, reject) => {
        const timer = setTimeout(() => {
            disposable.dispose();
            reject(new Error(`Debug session did not terminate within ${timeoutMs}ms`));
        }, timeoutMs);

        const disposable = vscode.debug.onDidTerminateDebugSession(() => {
            clearTimeout(timer);
            disposable.dispose();
            resolve();
        });
    });
}

/**
 * Get the stack trace for the given thread.
 */
async function getStackTrace(session: vscode.DebugSession, threadId: number): Promise<{
    stackFrames: {
        id: number;
        name: string;
        source?: { path?: string };
        line: number;
        column: number;
    }[];
    totalFrames: number;
}> {
    return session.customRequest('stackTrace', {
        threadId,
        startFrame: 0,
        levels: MAX_STACK_LEVELS,
    }) as Promise<{
        stackFrames: {
            id: number;
            name: string;
            source?: { path?: string };
            line: number;
            column: number;
        }[];
        totalFrames: number;
    }>;
}

/**
 * Get the scopes for a given stack frame.
 */
async function getScopes(session: vscode.DebugSession, frameId: number): Promise<{
    scopes: {
        name: string;
        variablesReference: number;
        expensive: boolean;
    }[];
}> {
    return session.customRequest('scopes', { frameId }) as Promise<{
        scopes: {
            name: string;
            variablesReference: number;
            expensive: boolean;
        }[];
    }>;
}

/**
 * Get variables for a given variables reference (scope or structured variable).
 */
async function getVariables(session: vscode.DebugSession, variablesReference: number): Promise<{
    variables: {
        name: string;
        value: string;
        type?: string;
        variablesReference: number;
    }[];
}> {
    return session.customRequest('variables', { variablesReference }) as Promise<{
        variables: {
            name: string;
            value: string;
            type?: string;
            variablesReference: number;
        }[];
    }>;
}

/** Options for evaluating an expression in a debug session. */
interface EvaluateExpressionOptions {
    session: vscode.DebugSession;
    expression: string;
    frameId: number;
    context?: 'watch' | 'repl' | 'hover';
}

/**
 * Evaluate an expression in the context of a stack frame (watch expression).
 */
async function evaluateExpression(options: EvaluateExpressionOptions): Promise<{
    result: string;
    type?: string;
    variablesReference: number;
}> {
    const { session, expression, frameId, context = 'watch' } = options;
    return session.customRequest('evaluate', {
        expression,
        frameId,
        context,
    }) as Promise<{
        result: string;
        type?: string;
        variablesReference: number;
    }>;
}

/**
 * Step over (next) in the given thread.
 */
async function stepOver(session: vscode.DebugSession, threadId: number): Promise<void> {
    await session.customRequest('next', { threadId });
}

/**
 * Step into in the given thread.
 */
async function stepIn(session: vscode.DebugSession, threadId: number): Promise<void> {
    await session.customRequest('stepIn', { threadId });
}

/**
 * Step out of the current function.
 */
async function stepOut(session: vscode.DebugSession, threadId: number): Promise<void> {
    await session.customRequest('stepOut', { threadId });
}

/**
 * Continue execution.
 */
async function continueExecution(session: vscode.DebugSession, threadId: number): Promise<void> {
    await session.customRequest('continue', { threadId });
}

/**
 * Wait for the debugger to stop (after a step or continue), returning the thread ID.
 * Uses polling on the active session's stack trace availability.
 */
async function waitForStop(timeoutMs: number = STOPPED_EVENT_TIMEOUT_MS): Promise<number> {
    return new Promise((resolve, reject) => {
        const timer = setTimeout(() => {
            clearInterval(poll);
            reject(new Error(`Timed out waiting for debugger to stop after ${timeoutMs}ms`));
        }, timeoutMs);

        const poll = setInterval(() => {
            void (async () => {
            const session = vscode.debug.activeDebugSession;
            if (session === undefined) {
                return;
            }
            try {
                const threadsResponse = (await session.customRequest('threads')) as {
                    threads?: { id: number }[];
                };
                const threads = threadsResponse.threads;
                if (threads !== undefined && threads.length > 0) {
                    const threadId: number = threads[0].id;
                    try {
                        const stack = await getStackTrace(session, threadId);
                        if (stack.stackFrames.length > 0) {
                            clearInterval(poll);
                            clearTimeout(timer);
                            resolve(threadId);
                        }
                    } catch {
                        // Thread is running, not stopped yet.
                    }
                }
            } catch {
                // Session not ready yet.
            }
            })();
        }, STOP_POLL_INTERVAL_MS);
    });
}

/**
 * Helper: Find a local variable by name in the current frame's local scope.
 */
async function getLocalVariable(
    session: vscode.DebugSession,
    threadId: number,
    varName: string
): Promise<{ name: string; value: string; type?: string } | undefined> {
    const stack = await getStackTrace(session, threadId);
    assert.ok(stack.stackFrames.length > 0, 'Expected at least one stack frame');
    const frameId = stack.stackFrames[0].id;
    const scopesResponse = await getScopes(session, frameId);
    const localsScope = scopesResponse.scopes.find(
        (s) => s.name === 'Locals' || s.name === 'Local'
    );
    assert.ok(localsScope, `Expected a Locals scope, got: ${scopesResponse.scopes.map(s => s.name).join(', ')}`);
    const varsResponse = await getVariables(session, localsScope.variablesReference);
    return varsResponse.variables.find((v) => v.name === varName);
}

/** Options for asserting a local variable's value. */
interface AssertLocalVariableOptions {
    session: vscode.DebugSession;
    threadId: number;
    varName: string;
    expectedValue: string;
    message?: string;
}

/**
 * Helper: Assert a local variable has the expected value string.
 */
async function assertLocalVariable(options: AssertLocalVariableOptions): Promise<void> {
    const { session, threadId, varName, expectedValue, message } = options;
    const variable = await getLocalVariable(session, threadId, varName);
    assert.ok(variable, `Variable '${varName}' not found in locals`);
    assert.strictEqual(
        variable.value,
        expectedValue,
        message ?? `Expected ${varName} = ${expectedValue}, got ${variable.value}`
    );
}

/** Options for asserting a watch expression's result. */
interface AssertWatchOptions {
    session: vscode.DebugSession;
    threadId: number;
    expression: string;
    expectedResult: string;
    message?: string;
}

/**
 * Helper: Assert a watch expression evaluates to the expected result.
 */
async function assertWatch(options: AssertWatchOptions): Promise<void> {
    const { session, threadId, expression, expectedResult, message } = options;
    const stack = await getStackTrace(session, threadId);
    const frameId = stack.stackFrames[0].id;
    const result = await evaluateExpression({ session, expression, frameId, context: 'watch' });
    assert.strictEqual(
        result.result,
        expectedResult,
        message ?? `Watch '${expression}': expected ${expectedResult}, got ${result.result}`
    );
}

/** Options for asserting the current line number. */
interface AssertCurrentLineOptions {
    session: vscode.DebugSession;
    threadId: number;
    expectedLine: number;
    message?: string;
}

/**
 * Helper: Assert the current line number in the top frame.
 */
async function assertCurrentLine(options: AssertCurrentLineOptions): Promise<void> {
    const { session, threadId, expectedLine, message } = options;
    const stack = await getStackTrace(session, threadId);
    assert.ok(stack.stackFrames.length > 0, 'Expected at least one stack frame');
    assert.strictEqual(
        stack.stackFrames[0].line,
        expectedLine,
        message ?? `Expected to be on line ${expectedLine}, but on line ${stack.stackFrames[0].line}`
    );
}

/** Options for asserting the current function name. */
interface AssertCurrentFunctionOptions {
    session: vscode.DebugSession;
    threadId: number;
    expectedName: string;
    message?: string;
}

/**
 * Helper: Assert the current function name in the top frame.
 */
async function assertCurrentFunction(options: AssertCurrentFunctionOptions): Promise<void> {
    const { session, threadId, expectedName, message } = options;
    const stack = await getStackTrace(session, threadId);
    assert.ok(stack.stackFrames.length > 0, 'Expected at least one stack frame');
    assert.strictEqual(
        stack.stackFrames[0].name,
        expectedName,
        message ?? `Expected function '${expectedName}', got '${stack.stackFrames[0].name}'`
    );
}

/**
 * Set breakpoints on specific lines of a file.
 */
function setBreakpoints(filePath: string, lines: number[]): void {
    const uri = vscode.Uri.file(filePath);
    const breakpoints = lines.map(
        (line) => new vscode.SourceBreakpoint(new vscode.Location(uri, new vscode.Position(line - 1, 0)))
    );
    vscode.debug.addBreakpoints(breakpoints);
}

/**
 * Clear all breakpoints.
 */
function clearAllBreakpoints(): void {
    vscode.debug.removeBreakpoints(vscode.debug.breakpoints);
}

/**
 * Stop the active debug session during cleanup, then let the runtime settle.
 *
 * VS Code rejects the terminate/disconnect request with "Canceled" when the
 * session ends on its own before the request resolves — a benign shutdown race.
 * Cleanup hooks must never fail the suite over that race, so it is swallowed;
 * any other rejection is re-thrown. [LSPDEBUG]
 */
async function stopActiveDebugSession(): Promise<void> {
    if (vscode.debug.activeDebugSession === undefined) {
        return;
    }
    try {
        await vscode.debug.stopDebugging();
    } catch (error) {
        const message = (error instanceof Error ? error.message : String(error)).toLowerCase();
        if (!message.includes('cancel')) {
            throw error;
        }
    }
    await new Promise<void>((resolve) => setTimeout(resolve, SESSION_SETTLE_MS));
}

/**
 * Start a debug session on the stepping fixture, wait for it to stop, return session + threadId.
 */
async function launchAndWaitForBreakpoint(
    breakpointLines: number[],
    pythonPath?: string
): Promise<{ session: vscode.DebugSession; threadId: number }> {
    clearAllBreakpoints();
    setBreakpoints(STEPPING_FIXTURE, breakpointLines);

    const sessionPromise = waitForDebugSessionStart();
    const stoppedPromise = waitForStop();

    const started = await vscode.debug.startDebugging(undefined, {
        name: 'Basilisk Debug Test',
        type: 'basilisk-debug',
        request: 'launch',
        program: STEPPING_FIXTURE,
        python: pythonPath,
        stopOnEntry: false,
        justMyCode: true,
        console: 'internalConsole',
    });
    assert.ok(started, 'vscode.debug.startDebugging should return true');

    const session = await sessionPromise;
    assert.ok(session !== undefined, 'Debug session should start');

    const threadId = await stoppedPromise;
    assert.ok(threadId > 0, `Thread ID should be positive, got ${threadId}`);

    return { session, threadId };
}

// ── Test Suite ──────────────────────────────────────────────────────────────
// Exercises [VSIX-PYTHON-DEBUGGER-DAP] / [VSIX-PYTHON-DEBUGGER-DAP-FEATURES]
// end-to-end against real debugpy: launch/attach, breakpoints, step in/over/out,
// variable inspection, watch expressions, call stack, hover/REPL evaluation, and
// clean termination. The PID-capture test covers [VSIX-PYTHON-DEBUGGER-DAP-TRACKER].

// eslint-disable-next-line max-lines-per-function
suite('Debug Integration E2E Tests', () => {
    let basiliskBinary: string | undefined;
    let debugpyAvailable: boolean;
    let pythonPath: string | undefined;
    let tmpDir: string;

    suiteSetup(async function () {
        this.timeout(SERVER_START_WAIT_MS + STOPPED_EVENT_TIMEOUT_MS);

        basiliskBinary = findBasiliskBinary();
        debugpyAvailable = isDebugpyInstalled();
        pythonPath = findPython();

        if (basiliskBinary === undefined) {
            throw new Error(
                'Basilisk binary not found. Build with: cargo build -p basilisk-cli'
            );
        }
        if (!debugpyAvailable) {
            throw new Error(
                'debugpy not installed. Install with: pip install debugpy'
            );
        }
        if (pythonPath === undefined) {
            throw new Error('Python not found. Install Python 3.12+.');
        }

        tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'basilisk-debug-test-'));

        // Ensure fixture exists.
        assert.ok(
            fs.existsSync(STEPPING_FIXTURE),
            `Fixture not found: ${STEPPING_FIXTURE}`
        );

        // Ensure the extension is activated.
        const ext = vscode.extensions.getExtension(EXTENSION_ID);
        if (ext && !ext.isActive) {
            await ext.activate();
        }

        // Give the LSP server time to fully initialize.
        await new Promise<void>((resolve) => setTimeout(resolve, SERVER_START_WAIT_MS));
    });

    suiteTeardown(async () => {
        clearAllBreakpoints();
        await stopActiveDebugSession();
        if (tmpDir !== undefined && tmpDir !== '' && fs.existsSync(tmpDir)) {
            fs.rmSync(tmpDir, { recursive: true, force: true });
        }
    });

    teardown(async () => {
        clearAllBreakpoints();
        await stopActiveDebugSession();
    });

    // ────────────────────────────────────────────────────────────────────────
    // 1. Package.json contributes basilisk-debug
    // ────────────────────────────────────────────────────────────────────────

    // [LSPDEBUG-WIRE], [VSIX-PYTHON-DEBUGGER-DAP-ARCHITECTURE]: both commands are
    // advertised by the extension contribution.
    test('LSP advertises startDebugSession and stopDebugSession commands', function () {
        this.timeout(SUBPROCESS_TIMEOUT_MS);
        const ext = vscode.extensions.getExtension(EXTENSION_ID);
        assert.ok(ext, 'Extension must be installed');
        const pkg = ext.packageJSON as PackageJSON;
        const debuggers = pkg.contributes?.debuggers;
        assert.ok(debuggers !== undefined, 'Extension must contribute debuggers');
        assert.ok(
            debuggers.some((d) => d.type === 'basilisk-debug'),
            'Extension must contribute basilisk-debug debugger type'
        );
    });

    // [VSIX-PYTHON-DEBUGGER-DAP-LAUNCH-CONFIGURATIONS]: the contributed launch/
    // attach config attribute schema for the basilisk-debug debugger.
    // eslint-disable-next-line complexity
    test('basilisk-debug type has correct configuration attributes', function () {
        this.timeout(SUBPROCESS_TIMEOUT_MS);
        const ext = vscode.extensions.getExtension(EXTENSION_ID);
        assert.ok(ext, 'Extension must be installed');

        const pkg = ext.packageJSON as PackageJSON;
        const debuggerContrib = pkg.contributes?.debuggers?.find(
            (d) => d.type === 'basilisk-debug'
        );
        assert.ok(debuggerContrib !== undefined, 'basilisk-debug debugger must be contributed');
        assert.strictEqual(debuggerContrib.label, 'Python (Basilisk)');
        assert.ok(debuggerContrib.configurationAttributes?.launch !== undefined, 'Launch config must be defined');
        assert.ok(debuggerContrib.configurationAttributes?.attach !== undefined, 'Attach config must be defined');
        assert.ok(
            debuggerContrib.configurationAttributes?.launch?.properties?.program !== undefined,
            'Launch must have program property'
        );
        assert.ok(
            debuggerContrib.configurationAttributes?.launch?.properties?.args !== undefined,
            'Launch must have args property'
        );
        assert.ok(
            debuggerContrib.configurationAttributes?.launch?.properties?.justMyCode !== undefined,
            'Launch must have justMyCode property'
        );
        assert.ok(
            debuggerContrib.configurationAttributes?.launch?.properties?.stopOnEntry !== undefined,
            'Launch must have stopOnEntry property'
        );
        assert.ok(
            debuggerContrib.configurationAttributes?.launch?.properties?.python !== undefined,
            'Launch must have python property'
        );
        assert.ok(
            debuggerContrib.configurationAttributes?.attach?.properties?.connect !== undefined,
            'Attach must have connect property'
        );
    });

    // ────────────────────────────────────────────────────────────────────────
    // 2. LSP-level: start/stop debug session via raw LSP commands
    // ────────────────────────────────────────────────────────────────────────

    // [LSPDEBUG-START]: response shape (host=localhost, port>0, sessionId dbg-…)
    // and debugpy actually listening on the returned port.
    test('startDebugSession spawns debugpy on a TCP port', async function () {
        this.timeout(DEBUG_SESSION_TIMEOUT_MS);

        const result = await startDebugSession(pythonPath);
        assert.ok(result !== undefined, 'Expected startDebugSession to return a result');
        assert.strictEqual(result.host, 'localhost', 'Host should be localhost');
        assert.ok(result.port > 0, `Port should be positive, got ${result.port}`);
        assert.ok(
            result.sessionId.startsWith('dbg-'),
            `Session ID should start with "dbg-", got "${result.sessionId}"`
        );

        const listening = await checkPortListening(result.host, result.port);
        assert.ok(listening, `Expected debugpy to be listening on ${result.host}:${result.port}`);

        await stopDebugSession(result.sessionId);
    });

    // [LSPDEBUG-STOP]: stop returns { stopped: true } and the port stops listening.
    test('stopDebugSession kills the debugpy process', async function () {
        this.timeout(DEBUG_SESSION_TIMEOUT_MS);

        const result = await startDebugSession(pythonPath);
        assert.ok(result.port > 0);

        const stopResult = await stopDebugSession(result.sessionId);
        assert.strictEqual(stopResult.stopped, true, 'Session should be reported as stopped');

        await new Promise<void>((resolve) => setTimeout(resolve, SESSION_SETTLE_MS));
        const stillListening = await checkPortListening(result.host, result.port, PORT_CLOSED_CHECK_MS);
        assert.strictEqual(stillListening, false, `Port ${result.port} should stop listening`);
    });

    // [LSPDEBUG-STOP]: an unknown sessionId resolves to { stopped: false }.
    test('stopDebugSession with invalid sessionId returns stopped: false', async function () {
        this.timeout(SUBPROCESS_TIMEOUT_MS);
        const result = await stopDebugSession('nonexistent-session-id');
        assert.strictEqual(result.stopped, false);
    });

    test('can start multiple debug sessions on different ports', async function () {
        this.timeout(DEBUG_SESSION_TIMEOUT_MS * 2);

        const session1 = await startDebugSession(pythonPath);
        const session2 = await startDebugSession(pythonPath);

        assert.notStrictEqual(session1.port, session2.port, 'Different ports');
        assert.notStrictEqual(session1.sessionId, session2.sessionId, 'Different IDs');

        const listening1 = await checkPortListening(session1.host, session1.port);
        const listening2 = await checkPortListening(session2.host, session2.port);
        assert.ok(listening1, `Session 1 listening on ${session1.port}`);
        assert.ok(listening2, `Session 2 listening on ${session2.port}`);

        await stopDebugSession(session1.sessionId);
        await stopDebugSession(session2.sessionId);
    });

    // [LSPDEBUG-ERRORS] / [LSPDEBUG-PYRES]: a bad interpreter yields a structured
    // error (debugpy/python not found) rather than a crash.
    test('startDebugSession with bad Python path returns error', async function () {
        this.timeout(DEBUG_SESSION_TIMEOUT_MS);
        try {
            await startDebugSession('/nonexistent/python3.99');
            assert.fail('Expected startDebugSession to throw with a bad Python path');
        } catch (err: unknown) {
            assert.ok(err !== null && err !== undefined, 'Expected an error to be thrown');
            const message = err instanceof Error ? err.message : JSON.stringify(err);
            assert.ok(message.length > 0, `Expected a meaningful error message, got: "${message}"`);
        }
    });

    // ────────────────────────────────────────────────────────────────────────
    // 3. Full DAP handshake test
    // ────────────────────────────────────────────────────────────────────────

    test('full debug lifecycle: start, verify DAP handshake, stop', async function () {
        this.timeout(DEBUG_SESSION_TIMEOUT_MS + SUBPROCESS_TIMEOUT_MS);

        const session = await startDebugSession(pythonPath);

        const dapResponse = await new Promise<string>((resolve, reject) => {
            const socket = new net.Socket();
            const timer = setTimeout(() => {
                socket.destroy();
                reject(new Error('DAP handshake timed out'));
            }, DAP_HANDSHAKE_TIMEOUT_MS);

            socket.connect(session.port, session.host, () => {
                const initRequest = JSON.stringify({
                    seq: 1,
                    type: 'request',
                    command: 'initialize',
                    arguments: {
                        clientID: 'basilisk-test',
                        adapterID: 'debugpy',
                        pathFormat: 'path',
                        linesStartAt1: true,
                        columnsStartAt1: true,
                    },
                });
                const header = `Content-Length: ${Buffer.byteLength(initRequest)}\r\n\r\n`;
                socket.write(header + initRequest);
            });

            let data = '';
            socket.on('data', (chunk) => {
                data += chunk.toString();
                // Parse the Content-Length header so we extract exactly one
                // DAP message, even if multiple arrive back-to-back.
                const headerEnd = data.indexOf('\r\n\r\n');
                if (headerEnd === -1) {return;}
                const header = data.slice(0, headerEnd);
                const match = /Content-Length:\s*(\d+)/i.exec(header);
                if (!match) {return;}
                const contentLength = parseInt(match[1], 10);
                const httpHeaderTerminatorLength = 4; // \r\n\r\n
                const bodyStart = headerEnd + httpHeaderTerminatorLength;
                if (data.length >= bodyStart + contentLength) {
                    const body = data.slice(bodyStart, bodyStart + contentLength);
                    clearTimeout(timer);
                    socket.destroy();
                    resolve(body);
                }
            });

            socket.on('error', (err) => {
                clearTimeout(timer);
                reject(err);
            });
        });

        const parsed = JSON.parse(dapResponse) as {
            type: string;
            command?: string;
            success?: boolean;
            body?: { supportsConfigurationDoneRequest?: boolean };
        };
        assert.ok(parsed !== undefined, 'Expected a valid JSON DAP response');
        assert.ok(
            parsed.type === 'response' || parsed.type === 'event',
            `Expected DAP response or event, got type: ${parsed.type}`
        );

        if (parsed.type === 'response') {
            assert.strictEqual(parsed.command, 'initialize', 'Should be initialize response');
            assert.strictEqual(parsed.success, true, 'Initialize should succeed');
            assert.ok(parsed.body !== undefined, 'Initialize response should have a body');
            assert.ok(
                parsed.body.supportsConfigurationDoneRequest !== undefined,
                'Should report supportsConfigurationDoneRequest'
            );
        }

        await stopDebugSession(session.sessionId);
    });

    // ────────────────────────────────────────────────────────────────────────
    // 4. REAL DEBUG SESSION: Arithmetic — step through, check every variable
    // ────────────────────────────────────────────────────────────────────────

    test('arithmetic: step through and assert variable values at each line', async function () {
        this.timeout(DEBUG_TEST_TIMEOUT_MS);

        // Break on line 11: x = 10
        const { session, threadId } = await launchAndWaitForBreakpoint([ARITH_X_LINE], pythonPath);

        // Stopped at line 11: x = 10 (not yet executed)
        await assertCurrentLine({ session: session, threadId: threadId, expectedLine: ARITH_X_LINE });
        await assertCurrentFunction({ session: session, threadId: threadId, expectedName: 'arithmetic' });

        // Step over: execute x = 10, now on line 12
        await stepOver(session, threadId);
        const tid2 = await waitForStop();
        await assertCurrentLine({ session: session, threadId: tid2, expectedLine: ARITH_Y_LINE });
        await assertLocalVariable({ session: session, threadId: tid2, varName: 'x', expectedValue: '10' });

        // Step over: execute y = 20, now on line 13
        await stepOver(session, tid2);
        const tid3 = await waitForStop();
        await assertCurrentLine({ session: session, threadId: tid3, expectedLine: ARITH_Z_LINE });
        await assertLocalVariable({ session: session, threadId: tid3, varName: 'x', expectedValue: '10' });
        await assertLocalVariable({ session: session, threadId: tid3, varName: 'y', expectedValue: '20' });

        // Step over: execute z = x + y, now on line 14
        await stepOver(session, tid3);
        const tid4 = await waitForStop();
        await assertCurrentLine({ session: session, threadId: tid4, expectedLine: ARITH_W_LINE });
        await assertLocalVariable({ session: session, threadId: tid4, varName: 'z', expectedValue: '30' });

        // Watch expressions
        await assertWatch({ session: session, threadId: tid4, expression: 'x + y', expectedResult: '30' });
        await assertWatch({ session: session, threadId: tid4, expression: 'z == 30', expectedResult: 'True' });
        await assertWatch({ session: session, threadId: tid4, expression: 'type(z).__name__', expectedResult: "'int'" });

        // Step over: execute w = z * 2, now on line 15
        await stepOver(session, tid4);
        const tid5 = await waitForStop();
        await assertCurrentLine({ session: session, threadId: tid5, expectedLine: ARITH_RESULT_LINE });
        await assertLocalVariable({ session: session, threadId: tid5, varName: 'w', expectedValue: '60' });

        // Step over: execute result = w - 5, now on line 16
        await stepOver(session, tid5);
        const tid6 = await waitForStop();
        await assertCurrentLine({ session: session, threadId: tid6, expectedLine: ARITH_RETURN_LINE });
        await assertLocalVariable({ session: session, threadId: tid6, varName: 'result', expectedValue: '55' });

        // Watch: verify final computed value
        await assertWatch({ session: session, threadId: tid6, expression: 'result == 55', expectedResult: 'True' });
        await assertWatch({ session: session, threadId: tid6, expression: 'result * 2', expectedResult: '110' });
    });

    // ────────────────────────────────────────────────────────────────────────
    // 5. REAL DEBUG SESSION: String operations
    // ────────────────────────────────────────────────────────────────────────

    test('string_ops: step through and assert string values', async function () {
        this.timeout(DEBUG_TEST_TIMEOUT_MS);

        const { session, threadId } = await launchAndWaitForBreakpoint([STRING_OPS_START_LINE], pythonPath);

        await assertCurrentLine({ session: session, threadId: threadId, expectedLine: STRING_OPS_START_LINE });
        await assertCurrentFunction({ session: session, threadId: threadId, expectedName: 'string_ops' });

        // Step: greeting = "hello"
        await stepOver(session, threadId);
        const t1 = await waitForStop();
        await assertLocalVariable({ session: session, threadId: t1, varName: 'greeting', expectedValue: "'hello'" });

        // Step: name = "world"
        await stepOver(session, t1);
        const t2 = await waitForStop();
        await assertLocalVariable({ session: session, threadId: t2, varName: 'name', expectedValue: "'world'" });

        // Step: message = greeting + " " + name
        await stepOver(session, t2);
        const t3 = await waitForStop();
        await assertLocalVariable({ session: session, threadId: t3, varName: 'message', expectedValue: "'hello world'" });

        // Watch: string operations
        await assertWatch({ session: session, threadId: t3, expression: 'len(message)', expectedResult: '11' });
        await assertWatch({ session: session, threadId: t3, expression: 'message.startswith("hello")', expectedResult: 'True' });
        await assertWatch({ session: session, threadId: t3, expression: '"world" in message', expectedResult: 'True' });

        // Step: upper = message.upper()
        await stepOver(session, t3);
        const t4 = await waitForStop();
        await assertLocalVariable({ session: session, threadId: t4, varName: 'upper', expectedValue: "'HELLO WORLD'" });

        // Step: length = len(upper)
        await stepOver(session, t4);
        const t5 = await waitForStop();
        await assertLocalVariable({ session: session, threadId: t5, varName: 'length', expectedValue: '11' });

        // Watch: verify everything
        await assertWatch({ session: session, threadId: t5, expression: 'upper == "HELLO WORLD"', expectedResult: 'True' });
        await assertWatch({ session: session, threadId: t5, expression: 'length == len(upper)', expectedResult: 'True' });
    });

    // ────────────────────────────────────────────────────────────────────────
    // 6. REAL DEBUG SESSION: List operations
    // ────────────────────────────────────────────────────────────────────────

    test('list_ops: step through and assert list contents', async function () {
        this.timeout(DEBUG_TEST_TIMEOUT_MS);

        const { session, threadId } = await launchAndWaitForBreakpoint([LIST_OPS_START_LINE], pythonPath);

        await assertCurrentLine({ session: session, threadId: threadId, expectedLine: LIST_OPS_START_LINE });
        await assertCurrentFunction({ session: session, threadId: threadId, expectedName: 'list_ops' });

        // Step: items = [1, 2, 3]
        await stepOver(session, threadId);
        const t1 = await waitForStop();
        await assertLocalVariable({ session: session, threadId: t1, varName: 'items', expectedValue: '[1, 2, 3]' });

        // Watch: list properties
        await assertWatch({ session: session, threadId: t1, expression: 'len(items)', expectedResult: '3' });
        await assertWatch({ session: session, threadId: t1, expression: 'items[0]', expectedResult: '1' });
        await assertWatch({ session: session, threadId: t1, expression: 'items[-1]', expectedResult: '3' });
        await assertWatch({ session: session, threadId: t1, expression: 'sum(items)', expectedResult: '6' });

        // Step: items.append(4)
        await stepOver(session, t1);
        const t2 = await waitForStop();
        await assertWatch({ session: session, threadId: t2, expression: 'len(items)', expectedResult: '4' });
        await assertWatch({ session: session, threadId: t2, expression: 'items[-1]', expectedResult: '4' });
        await assertWatch({ session: session, threadId: t2, expression: '4 in items', expectedResult: 'True' });

        // Step: items.insert(0, 0)
        await stepOver(session, t2);
        const t3 = await waitForStop();
        await assertWatch({ session: session, threadId: t3, expression: 'items[0]', expectedResult: '0' });
        await assertWatch({ session: session, threadId: t3, expression: 'len(items)', expectedResult: '5' });

        // Step: total = sum(items)
        await stepOver(session, t3);
        const t4 = await waitForStop();
        await assertLocalVariable({ session: session, threadId: t4, varName: 'total', expectedValue: '10' });

        // Step: count = len(items)
        await stepOver(session, t4);
        const t5 = await waitForStop();
        await assertLocalVariable({ session: session, threadId: t5, varName: 'count', expectedValue: '5' });

        // Watch: final assertions
        await assertWatch({ session: session, threadId: t5, expression: 'total == sum(items)', expectedResult: 'True' });
        await assertWatch({ session: session, threadId: t5, expression: 'count == len(items)', expectedResult: 'True' });
    });

    // ────────────────────────────────────────────────────────────────────────
    // 7. REAL DEBUG SESSION: Dictionary operations
    // ────────────────────────────────────────────────────────────────────────

    test('dict_ops: step through and assert dict contents', async function () {
        this.timeout(DEBUG_TEST_TIMEOUT_MS);

        const { session, threadId } = await launchAndWaitForBreakpoint([DICT_OPS_START_LINE], pythonPath);

        await assertCurrentLine({ session: session, threadId: threadId, expectedLine: DICT_OPS_START_LINE });
        await assertCurrentFunction({ session: session, threadId: threadId, expectedName: 'dict_ops' });

        // Step: data = {"a": 1, "b": 2}
        await stepOver(session, threadId);
        const t1 = await waitForStop();

        // Watch: dict operations
        await assertWatch({ session: session, threadId: t1, expression: 'len(data)', expectedResult: '2' });
        await assertWatch({ session: session, threadId: t1, expression: 'data["a"]', expectedResult: '1' });
        await assertWatch({ session: session, threadId: t1, expression: 'data["b"]', expectedResult: '2' });
        await assertWatch({ session: session, threadId: t1, expression: '"a" in data', expectedResult: 'True' });
        await assertWatch({ session: session, threadId: t1, expression: '"c" in data', expectedResult: 'False' });

        // Step: data["c"] = 3
        await stepOver(session, t1);
        const t2 = await waitForStop();
        await assertWatch({ session: session, threadId: t2, expression: 'len(data)', expectedResult: '3' });
        await assertWatch({ session: session, threadId: t2, expression: 'data["c"]', expectedResult: '3' });
        await assertWatch({ session: session, threadId: t2, expression: '"c" in data', expectedResult: 'True' });

        // Step: keys = list(data.keys())
        await stepOver(session, t2);
        const t3 = await waitForStop();
        await assertWatch({ session: session, threadId: t3, expression: 'len(keys)', expectedResult: '3' });

        // Step: total = sum(data.values())
        await stepOver(session, t3);
        const t4 = await waitForStop();
        await assertLocalVariable({ session: session, threadId: t4, varName: 'total', expectedValue: '6' });
        await assertWatch({ session: session, threadId: t4, expression: 'total == sum(data.values())', expectedResult: 'True' });

        // Step: has_a = "a" in data
        await stepOver(session, t4);
        const t5 = await waitForStop();
        await assertLocalVariable({ session: session, threadId: t5, varName: 'has_a', expectedValue: 'True' });
    });

    // ────────────────────────────────────────────────────────────────────────
    // 8. REAL DEBUG SESSION: Step into nested function calls
    // ────────────────────────────────────────────────────────────────────────

    test('nested_call: step into function, verify call stack', async function () {
        this.timeout(DEBUG_TEST_TIMEOUT_MS);

        const { session, threadId } = await launchAndWaitForBreakpoint([NESTED_CALL_START_LINE], pythonPath);

        await assertCurrentLine({ session: session, threadId: threadId, expectedLine: NESTED_CALL_START_LINE });
        await assertCurrentFunction({ session: session, threadId: threadId, expectedName: 'nested_call' });

        // Step: a = 5
        await stepOver(session, threadId);
        const t1 = await waitForStop();
        await assertLocalVariable({ session: session, threadId: t1, varName: 'a', expectedValue: '5' });

        // Step INTO: b = double(a) — should enter the double() function
        await stepIn(session, t1);
        const t2 = await waitForStop();
        await assertCurrentFunction({ session: session, threadId: t2, expectedName: 'double' });

        // We're inside double(). Check the parameter.
        await assertLocalVariable({ session: session, threadId: t2, varName: 'n', expectedValue: '5' });

        // Step over inside double: result = n * 2
        await stepOver(session, t2);
        const t3 = await waitForStop();
        await assertLocalVariable({ session: session, threadId: t3, varName: 'result', expectedValue: '10' });
        await assertWatch({ session: session, threadId: t3, expression: 'result == n * 2', expectedResult: 'True' });

        // Step out back to nested_call
        await stepOut(session, t3);
        const t4 = await waitForStop();
        await assertCurrentFunction({ session: session, threadId: t4, expectedName: 'nested_call' });
        await assertLocalVariable({ session: session, threadId: t4, varName: 'b', expectedValue: '10' });

        // Verify stack depth
        const stack = await getStackTrace(session, t4);
        assert.ok(stack.stackFrames.length >= 1, 'Should have at least 1 frame');
        assert.strictEqual(stack.stackFrames[0].name, 'nested_call');
    });

    // ────────────────────────────────────────────────────────────────────────
    // 9. REAL DEBUG SESSION: Loop stepping and accumulator verification
    // ────────────────────────────────────────────────────────────────────────

    test('loop_and_accumulate: step through loop, verify accumulator', async function () {
        this.timeout(LOOP_TEST_TIMEOUT_MS);

        const { session, threadId } = await launchAndWaitForBreakpoint([LOOP_START_LINE], pythonPath);

        await assertCurrentLine({ session: session, threadId: threadId, expectedLine: LOOP_START_LINE });
        await assertCurrentFunction({ session: session, threadId: threadId, expectedName: 'loop_and_accumulate' });

        // Step: total = 0
        await stepOver(session, threadId);
        const t1 = await waitForStop();
        await assertLocalVariable({ session: session, threadId: t1, varName: 'total', expectedValue: '0' });

        // Step into the for loop header
        await stepOver(session, t1);
        const t2 = await waitForStop();

        // Step through the loop body: total += i  (i=0)
        await stepOver(session, t2);
        const t3 = await waitForStop();
        await assertWatch({ session: session, threadId: t3, expression: 'total', expectedResult: '0' }); // 0 + 0 = 0

        // Continue through iterations — step over the for line + body for i=1
        await stepOver(session, t3);
        const t4 = await waitForStop();
        await stepOver(session, t4);
        const t5 = await waitForStop();
        await assertWatch({ session: session, threadId: t5, expression: 'total', expectedResult: '1' }); // 0 + 1 = 1

        // i=2
        await stepOver(session, t5);
        const t6 = await waitForStop();
        await stepOver(session, t6);
        const t7 = await waitForStop();
        await assertWatch({ session: session, threadId: t7, expression: 'total', expectedResult: '3' }); // 1 + 2 = 3

        // i=3
        await stepOver(session, t7);
        const t8 = await waitForStop();
        await stepOver(session, t8);
        const t9 = await waitForStop();
        await assertWatch({ session: session, threadId: t9, expression: 'total', expectedResult: '6' }); // 3 + 3 = 6

        // i=4
        await stepOver(session, t9);
        const t10 = await waitForStop();
        await stepOver(session, t10);
        const t11 = await waitForStop();
        await assertWatch({ session: session, threadId: t11, expression: 'total', expectedResult: '10' }); // 6 + 4 = 10
    });

    // ────────────────────────────────────────────────────────────────────────
    // 10. REAL DEBUG SESSION: Conditional branches
    // ────────────────────────────────────────────────────────────────────────

    test('conditional_branches: verify correct branch taken', async function () {
        this.timeout(DEBUG_TEST_TIMEOUT_MS);

        const { session, threadId } = await launchAndWaitForBreakpoint([COND_START_LINE], pythonPath);

        await assertCurrentLine({ session: session, threadId: threadId, expectedLine: COND_START_LINE });
        await assertCurrentFunction({ session: session, threadId: threadId, expectedName: 'conditional_branches' });

        // Step: x = 42
        await stepOver(session, threadId);
        const t1 = await waitForStop();
        await assertLocalVariable({ session: session, threadId: t1, varName: 'x', expectedValue: '42' });
        await assertWatch({ session: session, threadId: t1, expression: 'x > 100', expectedResult: 'False' });
        await assertWatch({ session: session, threadId: t1, expression: 'x > 10', expectedResult: 'True' });

        // Step: if x > 100 — should go to elif
        await stepOver(session, t1);
        const t2 = await waitForStop();

        // Step: elif x > 10 — should be true, enter that branch
        await stepOver(session, t2);
        const t3 = await waitForStop();

        // Step: label = "medium"
        await stepOver(session, t3);
        const t4 = await waitForStop();
        await assertLocalVariable({ session: session, threadId: t4, varName: 'label', expectedValue: "'medium'" });

        // Watch: verify the branch result
        await assertWatch({ session: session, threadId: t4, expression: 'label == "medium"', expectedResult: 'True' });
        await assertWatch({ session: session, threadId: t4, expression: 'label != "big"', expectedResult: 'True' });
        await assertWatch({ session: session, threadId: t4, expression: 'label != "small"', expectedResult: 'True' });
    });

    // ────────────────────────────────────────────────────────────────────────
    // 11. REAL DEBUG SESSION: Exception handling
    // ────────────────────────────────────────────────────────────────────────

    test('exception_handling: step through try/except, verify caught state', async function () {
        this.timeout(DEBUG_TEST_TIMEOUT_MS);

        const { session, threadId } = await launchAndWaitForBreakpoint([EXCEPT_START_LINE], pythonPath);

        await assertCurrentLine({ session: session, threadId: threadId, expectedLine: EXCEPT_START_LINE });
        await assertCurrentFunction({ session: session, threadId: threadId, expectedName: 'exception_handling' });

        // Step: caught = False
        await stepOver(session, threadId);
        const t1 = await waitForStop();
        await assertLocalVariable({ session: session, threadId: t1, varName: 'caught', expectedValue: 'False' });

        // Step: error_msg = ""
        await stepOver(session, t1);
        const t2 = await waitForStop();
        await assertLocalVariable({ session: session, threadId: t2, varName: 'error_msg', expectedValue: "''" });

        // Step into try block: value = 1 / 0 — this raises ZeroDivisionError
        await stepOver(session, t2);
        const t3 = await waitForStop();

        // Step: the exception is caught, now in the except handler
        await stepOver(session, t3);
        const t4 = await waitForStop();

        // Step: caught = True
        await stepOver(session, t4);
        const t5 = await waitForStop();
        await assertLocalVariable({ session: session, threadId: t5, varName: 'caught', expectedValue: 'True' });

        // Step: error_msg = str(exc)
        await stepOver(session, t5);
        const t6 = await waitForStop();
        await assertWatch({ session: session, threadId: t6, expression: 'caught', expectedResult: 'True' });
        await assertWatch({ session: session, threadId: t6, expression: 'len(error_msg) > 0', expectedResult: 'True' });
    });

    // ────────────────────────────────────────────────────────────────────────
    // 12. REAL DEBUG SESSION: Type variety — verify type representations
    // ────────────────────────────────────────────────────────────────────────

    test('type_variety: verify different Python types in debugger', async function () {
        this.timeout(DEBUG_TEST_TIMEOUT_MS);

        const { session, threadId } = await launchAndWaitForBreakpoint([TYPE_VARIETY_START_LINE], pythonPath);

        await assertCurrentLine({ session: session, threadId: threadId, expectedLine: TYPE_VARIETY_START_LINE });
        await assertCurrentFunction({ session: session, threadId: threadId, expectedName: 'type_variety' });

        // an_int = 42
        await stepOver(session, threadId);
        const t1 = await waitForStop();
        await assertLocalVariable({ session: session, threadId: t1, varName: 'an_int', expectedValue: '42' });
        await assertWatch({ session: session, threadId: t1, expression: 'type(an_int).__name__', expectedResult: "'int'" });

        // a_float = 3.14
        await stepOver(session, t1);
        const t2 = await waitForStop();
        await assertLocalVariable({ session: session, threadId: t2, varName: 'a_float', expectedValue: '3.14' });
        await assertWatch({ session: session, threadId: t2, expression: 'type(a_float).__name__', expectedResult: "'float'" });

        // a_bool = True
        await stepOver(session, t2);
        const t3 = await waitForStop();
        await assertLocalVariable({ session: session, threadId: t3, varName: 'a_bool', expectedValue: 'True' });
        await assertWatch({ session: session, threadId: t3, expression: 'type(a_bool).__name__', expectedResult: "'bool'" });

        // a_none = None
        await stepOver(session, t3);
        const t4 = await waitForStop();
        await assertLocalVariable({ session: session, threadId: t4, varName: 'a_none', expectedValue: 'None' });
        await assertWatch({ session: session, threadId: t4, expression: 'a_none is None', expectedResult: 'True' });

        // a_tuple = (1, "two", 3.0)
        await stepOver(session, t4);
        const t5 = await waitForStop();
        await assertWatch({ session: session, threadId: t5, expression: 'len(a_tuple)', expectedResult: '3' });
        await assertWatch({ session: session, threadId: t5, expression: 'a_tuple[0]', expectedResult: '1' });
        await assertWatch({ session: session, threadId: t5, expression: 'type(a_tuple).__name__', expectedResult: "'tuple'" });

        // a_set = {10, 20, 30}
        await stepOver(session, t5);
        const t6 = await waitForStop();
        await assertWatch({ session: session, threadId: t6, expression: 'len(a_set)', expectedResult: '3' });
        await assertWatch({ session: session, threadId: t6, expression: '10 in a_set', expectedResult: 'True' });
        await assertWatch({ session: session, threadId: t6, expression: 'type(a_set).__name__', expectedResult: "'set'" });

        // a_bytes = b"hello"
        await stepOver(session, t6);
        const t7 = await waitForStop();
        await assertWatch({ session: session, threadId: t7, expression: 'len(a_bytes)', expectedResult: '5' });
        await assertWatch({ session: session, threadId: t7, expression: 'type(a_bytes).__name__', expectedResult: "'bytes'" });
    });

    // ────────────────────────────────────────────────────────────────────────
    // 13. REAL DEBUG SESSION: Class instance — check object attributes
    // ────────────────────────────────────────────────────────────────────────

    test('class_instance: step through, inspect object attributes', async function () {
        this.timeout(DEBUG_TEST_TIMEOUT_MS);

        // Break at line 119: p = Point(3, 4)
        const { session, threadId } = await launchAndWaitForBreakpoint([CLASS_INSTANCE_START_LINE], pythonPath);

        await assertCurrentLine({ session: session, threadId: threadId, expectedLine: CLASS_INSTANCE_START_LINE });
        await assertCurrentFunction({ session: session, threadId: threadId, expectedName: 'class_instance' });

        // Step over: p = Point(3, 4)
        await stepOver(session, threadId);
        const t1 = await waitForStop();

        // Verify object attributes via watch
        await assertWatch({ session: session, threadId: t1, expression: 'p.x', expectedResult: '3' });
        await assertWatch({ session: session, threadId: t1, expression: 'p.y', expectedResult: '4' });
        await assertWatch({ session: session, threadId: t1, expression: 'type(p).__name__', expectedResult: "'Point'" });

        // Step: mag = p.magnitude()
        await stepOver(session, t1);
        const t2 = await waitForStop();
        await assertLocalVariable({ session: session, threadId: t2, varName: 'mag', expectedValue: '5.0' });

        // Watch: verify computed value
        await assertWatch({ session: session, threadId: t2, expression: 'mag == 5.0', expectedResult: 'True' });
        await assertWatch({ session: session, threadId: t2, expression: 'p.x ** 2 + p.y ** 2', expectedResult: '25' });
    });

    // ────────────────────────────────────────────────────────────────────────
    // 14. REAL DEBUG SESSION: Multiple breakpoints, continue between them
    // ────────────────────────────────────────────────────────────────────────

    test('continue between multiple breakpoints', async function () {
        this.timeout(DEBUG_TEST_TIMEOUT_MS);

        // Set breakpoints in arithmetic() and string_ops()
        const { session, threadId } = await launchAndWaitForBreakpoint([ARITH_Z_LINE, STRING_OPS_MESSAGE_LINE], pythonPath);

        // Should stop at line 13 first (z = x + y in arithmetic())
        await assertCurrentLine({ session: session, threadId: threadId, expectedLine: ARITH_Z_LINE });
        await assertCurrentFunction({ session: session, threadId: threadId, expectedName: 'arithmetic' });

        // Verify x and y are set
        await assertLocalVariable({ session: session, threadId: threadId, varName: 'x', expectedValue: '10' });
        await assertLocalVariable({ session: session, threadId: threadId, varName: 'y', expectedValue: '20' });

        // Continue to next breakpoint — line 23 (message = ... in string_ops())
        await continueExecution(session, threadId);
        const t2 = await waitForStop();

        await assertCurrentLine({ session: session, threadId: t2, expectedLine: STRING_OPS_MESSAGE_LINE });
        await assertCurrentFunction({ session: session, threadId: t2, expectedName: 'string_ops' });
        await assertLocalVariable({ session: session, threadId: t2, varName: 'greeting', expectedValue: "'hello'" });
        await assertLocalVariable({ session: session, threadId: t2, varName: 'name', expectedValue: "'world'" });
    });

    // ────────────────────────────────────────────────────────────────────────
    // 15. REAL DEBUG SESSION: Stack trace depth verification
    // ────────────────────────────────────────────────────────────────────────

    test('stack trace shows correct call hierarchy', async function () {
        this.timeout(DEBUG_TEST_TIMEOUT_MS);

        // Break inside double(), called from nested_call()
        const { session, threadId } = await launchAndWaitForBreakpoint([DOUBLE_RESULT_LINE], pythonPath);

        await assertCurrentLine({ session: session, threadId: threadId, expectedLine: DOUBLE_RESULT_LINE });
        await assertCurrentFunction({ session: session, threadId: threadId, expectedName: 'double' });

        // Verify the call stack
        const stack = await getStackTrace(session, threadId);
        assert.ok(stack.stackFrames.length >= 2, `Stack should have >= 2 frames, got ${stack.stackFrames.length}`);

        // Top frame: double
        assert.strictEqual(stack.stackFrames[0].name, 'double');
        assert.strictEqual(stack.stackFrames[0].line, DOUBLE_RESULT_LINE);

        // Second frame: nested_call (the caller)
        assert.strictEqual(stack.stackFrames[1].name, 'nested_call');

        // Both frames should reference the fixture file
        assert.ok(
            stack.stackFrames[0].source?.path?.includes('debug_stepping.py'),
            'Top frame should be in debug_stepping.py'
        );
        assert.ok(
            stack.stackFrames[1].source?.path?.includes('debug_stepping.py'),
            'Caller frame should be in debug_stepping.py'
        );
    });

    // ────────────────────────────────────────────────────────────────────────
    // 16. REAL DEBUG SESSION: Scopes enumeration
    // ────────────────────────────────────────────────────────────────────────

    test('scopes show Locals and variable details', async function () {
        this.timeout(DEBUG_TEST_TIMEOUT_MS);

        const { session, threadId } = await launchAndWaitForBreakpoint([ARITH_Z_LINE], pythonPath);

        await assertCurrentLine({ session: session, threadId: threadId, expectedLine: ARITH_Z_LINE });

        const stack = await getStackTrace(session, threadId);
        const frameId = stack.stackFrames[0].id;
        const scopesResponse = await getScopes(session, frameId);

        assert.ok(scopesResponse.scopes.length >= 1, 'Should have at least 1 scope');

        const scopeNames = scopesResponse.scopes.map((s) => s.name);
        assert.ok(
            scopeNames.some((n) => n === 'Locals' || n === 'Local'),
            `Should have a Locals scope, got: ${scopeNames.join(', ')}`
        );

        // Locals scope should have variables
        const localsScope = scopesResponse.scopes.find(
            (s) => s.name === 'Locals' || s.name === 'Local'
        );
        assert.ok(localsScope, 'Locals scope must exist');
        assert.ok(localsScope.variablesReference > 0, 'Locals must have variablesReference > 0');

        const varsResponse = await getVariables(session, localsScope.variablesReference);
        assert.ok(varsResponse.variables.length > 0, 'Locals should have variables');

        // x and y should be visible (set before line 13)
        const xVar = varsResponse.variables.find((v) => v.name === 'x');
        const yVar = varsResponse.variables.find((v) => v.name === 'y');
        assert.ok(xVar, 'x should be in locals');
        assert.ok(yVar, 'y should be in locals');
        assert.strictEqual(xVar.value, '10');
        assert.strictEqual(yVar.value, '20');
    });

    // ────────────────────────────────────────────────────────────────────────
    // 17. REAL DEBUG SESSION: Watch expressions — complex evaluations
    // ────────────────────────────────────────────────────────────────────────

    test('watch expressions: evaluate complex expressions', async function () {
        this.timeout(DEBUG_TEST_TIMEOUT_MS);

        // Stop at line 15 in arithmetic where x=10, y=20, z=30, w=60
        const { session, threadId } = await launchAndWaitForBreakpoint([ARITH_RESULT_LINE], pythonPath);

        await assertCurrentLine({ session: session, threadId: threadId, expectedLine: ARITH_RESULT_LINE });

        // Arithmetic watch expressions
        await assertWatch({ session: session, threadId: threadId, expression: 'x', expectedResult: '10' });
        await assertWatch({ session: session, threadId: threadId, expression: 'y', expectedResult: '20' });
        await assertWatch({ session: session, threadId: threadId, expression: 'z', expectedResult: '30' });
        await assertWatch({ session: session, threadId: threadId, expression: 'w', expectedResult: '60' });

        // Computed expressions
        await assertWatch({ session: session, threadId: threadId, expression: 'x + y + z', expectedResult: '60' });
        await assertWatch({ session: session, threadId: threadId, expression: 'w // x', expectedResult: '6' });
        await assertWatch({ session: session, threadId: threadId, expression: 'w % 7', expectedResult: '4' });
        await assertWatch({ session: session, threadId: threadId, expression: 'w ** 0', expectedResult: '1' });
        await assertWatch({ session: session, threadId: threadId, expression: 'abs(-w)', expectedResult: '60' });
        await assertWatch({ session: session, threadId: threadId, expression: 'min(x, y, z, w)', expectedResult: '10' });
        await assertWatch({ session: session, threadId: threadId, expression: 'max(x, y, z, w)', expectedResult: '60' });
        await assertWatch({ session: session, threadId: threadId, expression: 'sorted([w, z, y, x])', expectedResult: '[10, 20, 30, 60]' });

        // Boolean expressions
        await assertWatch({ session: session, threadId: threadId, expression: 'x < y', expectedResult: 'True' });
        await assertWatch({ session: session, threadId: threadId, expression: 'x > y', expectedResult: 'False' });
        await assertWatch({ session: session, threadId: threadId, expression: 'x == 10 and y == 20', expectedResult: 'True' });
        await assertWatch({ session: session, threadId: threadId, expression: 'z == x + y', expectedResult: 'True' });
        await assertWatch({ session: session, threadId: threadId, expression: 'w == z * 2', expectedResult: 'True' });

        // Type checking via watch
        await assertWatch({ session: session, threadId: threadId, expression: 'isinstance(x, int)', expectedResult: 'True' });
        await assertWatch({ session: session, threadId: threadId, expression: 'isinstance(x, str)', expectedResult: 'False' });

        // String formatting via watch
        await assertWatch({ session: session, threadId: threadId, expression: 'f"{x} + {y} = {z}"', expectedResult: "'10 + 20 = 30'" });

        // List comprehension via watch
        await assertWatch({ session: session, threadId: threadId, expression: '[v * 2 for v in [x, y, z]]', expectedResult: '[20, 40, 60]' });
    });

    // ────────────────────────────────────────────────────────────────────────
    // 18. REAL DEBUG SESSION: Hover-style evaluation
    // ────────────────────────────────────────────────────────────────────────

    test('hover evaluation: evaluate expressions in hover context', async function () {
        this.timeout(DEBUG_TEST_TIMEOUT_MS);

        const { session, threadId } = await launchAndWaitForBreakpoint([ARITH_Z_LINE], pythonPath);

        const stack = await getStackTrace(session, threadId);
        const frameId = stack.stackFrames[0].id;

        // Hover evaluation (simulates mouse hover in editor)
        const hoverResult = await evaluateExpression({ session: session, expression: 'x', frameId: frameId, context: 'hover' });
        assert.strictEqual(hoverResult.result, '10');
        assert.ok(hoverResult.type !== undefined && hoverResult.type !== '', 'Hover result should include type info');

        const hoverResult2 = await evaluateExpression({ session: session, expression: 'y', frameId: frameId, context: 'hover' });
        assert.strictEqual(hoverResult2.result, '20');
    });

    // ────────────────────────────────────────────────────────────────────────
    // 19. REAL DEBUG SESSION: REPL evaluation
    // ────────────────────────────────────────────────────────────────────────

    test('REPL evaluation: evaluate expressions in debug console context', async function () {
        this.timeout(DEBUG_TEST_TIMEOUT_MS);

        const { session, threadId } = await launchAndWaitForBreakpoint([ARITH_Z_LINE], pythonPath);

        const stack = await getStackTrace(session, threadId);
        const frameId = stack.stackFrames[0].id;

        // REPL evaluation (simulates Debug Console)
        const replResult = await evaluateExpression({ session: session, expression: 'x + y', frameId: frameId, context: 'repl' });
        assert.strictEqual(replResult.result, '30');

        const replResult2 = await evaluateExpression({ session: session, expression: '[x, y]', frameId: frameId, context: 'repl' });
        assert.strictEqual(replResult2.result, '[10, 20]');

        const replResult3 = await evaluateExpression({ session: session, expression: 'dict(a=x, b=y)', frameId: frameId, context: 'repl' });
        assert.ok(replResult3.result.includes('a'), 'REPL dict result should contain key a');
        assert.ok(replResult3.result.includes('b'), 'REPL dict result should contain key b');
    });

    // ────────────────────────────────────────────────────────────────────────
    // 20. Debug session terminates cleanly
    // [VSIX-PYTHON-DEBUGGER-DAP-PROXY] Quirk 4: exited-before-terminated ordering
    // so activeDebugSession clears when the session ends.
    // ────────────────────────────────────────────────────────────────────────

    test('debug session terminates cleanly after continue past end', async function () {
        this.timeout(DEBUG_TEST_TIMEOUT_MS);

        // Break at the return of arithmetic()
        const { session, threadId } = await launchAndWaitForBreakpoint([ARITH_RETURN_LINE], pythonPath);

        await assertCurrentLine({ session: session, threadId: threadId, expectedLine: ARITH_RETURN_LINE });

        const endPromise = waitForDebugSessionEnd(SESSION_END_WAIT_MS);

        // Continue — the program will run through remaining functions and exit
        await continueExecution(session, threadId);

        await endPromise;

        // VS Code may not clear activeDebugSession synchronously with the
        // terminate event — poll briefly to let the runtime settle.
        for (let i = 0; i < SESSION_CLEAR_MAX_POLLS && vscode.debug.activeDebugSession; i++) {
            await new Promise<void>((r) => setTimeout(r, STOP_POLL_INTERVAL_MS));
        }

        assert.strictEqual(
            vscode.debug.activeDebugSession,
            undefined,
            'No debug session should be active after program completes'
        );
    });

    // ────────────────────────────────────────────────────────────────────────
    // 21. Attach mode test
    // ────────────────────────────────────────────────────────────────────────

    // [LSPDEBUG-ATTACH], [VSIX-PYTHON-DEBUGGER-DAP-PROXY] Quirk 3 (non-destructive
    // single-connection slot check): attach connects the editor's DAP client
    // directly to a running debugpy server (the LSP is not involved in attach
    // traffic).
    test('attach to manually spawned debugpy server', async function () {
        this.timeout(DEBUG_TEST_TIMEOUT_MS);

        // Start debugpy via LSP command to get a running server
        const lspSession = await startDebugSession(pythonPath);
        assert.ok(lspSession.port > 0);

        const listening = await checkPortListening(lspSession.host, lspSession.port);
        assert.ok(listening, 'debugpy should be listening before attach');

        // Now try attach mode
        const sessionPromise = waitForDebugSessionStart();

        const started = await vscode.debug.startDebugging(undefined, {
            name: 'Basilisk Attach Test',
            type: 'basilisk-debug',
            request: 'attach',
            connect: {
                host: lspSession.host,
                port: lspSession.port,
            },
        });
        assert.ok(started, 'Attach debug session should start');

        const attachSession = await sessionPromise;
        assert.ok(attachSession !== undefined, 'Attach session should be created');

        // Clean up
        await stopActiveDebugSession();
        await stopDebugSession(lspSession.sessionId);
    });

    // ────────────────────────────────────────────────────────────────────────
    // 22. Error notification: bad python path
    // ────────────────────────────────────────────────────────────────────────

    test('startDebugSession with bad python shows error', async function () {
        this.timeout(DEBUG_SESSION_TIMEOUT_MS);

        try {
            await startDebugSession('/nonexistent/python_for_debugpy_test');
            assert.fail('Expected an error');
        } catch (err: unknown) {
            const message = err instanceof Error ? err.message : String(err);
            assert.ok(
                message.length > 0,
                'Error should have a message explaining what went wrong'
            );
        }
    });

    // ────────────────────────────────────────────────────────────────────────
    // 23. Profiler "same process": the debuggee PID is captured from the DAP
    //     `process` event so CPU profiling can target the same process. [LSPPROF],
    //     [VSIX-PYTHON-DEBUGGER-DAP-TRACKER]
    // ────────────────────────────────────────────────────────────────────────

    test('captures debuggee PID from the debug session for same-process profiling', async function () {
        this.timeout(DEBUG_SESSION_TIMEOUT_MS + STOPPED_EVENT_TIMEOUT_MS);

        const { session } = await launchAndWaitForBreakpoint([34], pythonPath);

        // The DAP `process` event carries systemProcessId; the proxy captures it
        // into the store keyed by VS Code session id. Poll briefly because the
        // event can arrive shortly after the first stop.
        let pid: number | undefined;
        for (let i = 0; i < 40 && pid === undefined; i++) {
            pid = getStore()?.getDebuggeeProcessId(session.id);
            if (pid === undefined) {
                await new Promise<void>((resolve) => setTimeout(resolve, 50));
            }
        }

        assert.ok(
            pid !== undefined && pid > 0,
            `debuggee PID should be captured for session ${session.id}, got ${String(pid)}`
        );

        await stopActiveDebugSession();
    });

    // ────────────────────────────────────────────────────────────────────────
    // 24. Memory profiling round-trip against REAL debugpy: the editor couriers
    //     the LSP's injection scripts via DAP `evaluate` and posts the output
    //     back to `basilisk.memory.ingest`, which parses a real tracemalloc
    //     snapshot. [LSPPROF] PROFILE-MEMORY
    // ────────────────────────────────────────────────────────────────────────

    test('memory round-trip: tracemalloc start + snapshot via DAP evaluate', async function () {
        this.timeout(DEBUG_SESSION_TIMEOUT_MS + STOPPED_EVENT_TIMEOUT_MS);

        await launchAndWaitForBreakpoint([34], pythonPath);

        // Exercise the real bridge (dap-evaluate.ts): resolve the stopped frame
        // the same way the memory commands do.
        const frameId = await currentStoppedFrameId();
        if (frameId === null) {
            assert.fail('currentStoppedFrameId should resolve a frame while paused');
        }

        // 1. Mint a memory session + fetch the start script from the LSP.
        const start = await vscode.commands.executeCommand<{ memorySessionId?: string; script?: string }>(
            'basilisk.memory.start',
            { tracebackDepth: 25 }
        );
        assert.ok(start.memorySessionId !== undefined, 'start should return a memorySessionId');
        assert.ok(start.script?.includes('tracemalloc.start'), 'start script should start tracemalloc');

        // 2. Inject tracemalloc into the live debuggee, then allocate ~2 MB so the
        //    snapshot has something concrete to report.
        await evaluateInDebugSession(start.script ?? '', frameId);
        await evaluateInDebugSession(
            'global _bsk_leak\n_bsk_leak = [bytearray(1024) for _ in range(2000)]',
            frameId
        );

        // 3. Fetch the snapshot script, run it in the debuggee, courier output back.
        const snapCmd = await vscode.commands.executeCommand<{ script?: string }>(
            'basilisk.memory.snapshot',
            { memorySessionId: start.memorySessionId }
        );
        assert.ok(snapCmd.script?.includes('__BASILISK_MEM__'), 'snapshot script should print the marker');

        const output = await evaluateInDebugSession(snapCmd.script ?? '', frameId);
        assert.ok(output !== null, 'evaluate should return the snapshot output');
        const result = await vscode.commands.executeCommand<{ kind?: string; currentMemory?: number; snapshotId?: string }>(
            'basilisk.memory.ingest',
            { memorySessionId: start.memorySessionId, output }
        );

        assert.strictEqual(result.kind, 'snapshot', 'ingest should yield a snapshot');
        assert.ok(typeof result.snapshotId === 'string', 'snapshot should have an id');
        assert.ok(
            typeof result.currentMemory === 'number' && result.currentMemory > 0,
            `tracemalloc should report tracked memory, got ${String(result.currentMemory)}`
        );

        await stopActiveDebugSession();
    });

    // ────────────────────────────────────────────────────────────────────────
    // 25. Memory snapshot on a LARGE heap must not stall the paused evaluate:
    //     pydevd warns after PYDEVD_WARN_EVALUATION_TIMEOUT (3 s) with a wall
    //     of timeout text in the debug console — the user-reported "take a
    //     snapshot → did not finish after 3.00 seconds" bug. The snapshot
    //     round-trip must both succeed AND never trip that warning.
    //     [LSPPROF] PROFILE-MEMORY-HOWTO
    // ────────────────────────────────────────────────────────────────────────

    test('large-heap memory snapshot does not trip the pydevd evaluation stall warning', async function () {
        this.timeout(LARGE_HEAP_MEM_TIMEOUT_MS);

        // A helper module with NO breakpoints builds the heap, so pydevd's
        // line-tracing of breakpoint files doesn't dominate the build; the
        // ~600k-item comprehension yields ~3M live tracemalloc traces.
        const heapHelper = path.join(tmpDir, 'bigheap_build.py');
        const heapMain = path.join(tmpDir, 'bigheap_main.py');
        fs.writeFileSync(heapHelper, [
            '"""Builds a large traced heap (~3M tracemalloc traces)."""',
            'HEAP_ITEMS = 600000',
            '',
            '',
            'def build():',
            '    return [(str(i), [i]) for i in range(HEAP_ITEMS)]',
            '',
        ].join('\n'));
        fs.writeFileSync(heapMain, [
            '"""Large-heap fixture for the memory evaluation-budget regression."""',
            'import bigheap_build',
            '',
            'anchor_start = 1',                 // line 4: BP A — inject tracemalloc
            'keep = bigheap_build.build()',     // line 5: heap built while traced
            'anchor_ready = len(keep)',         // line 6: BP B — take the snapshot
            'print(anchor_ready)',
            '',
        ].join('\n'));

        clearAllBreakpoints();
        setBreakpoints(heapMain, [BIGHEAP_START_LINE, BIGHEAP_READY_LINE]);
        const sessionPromise = waitForDebugSessionStart();
        const stoppedPromise = waitForStop();
        const started = await vscode.debug.startDebugging(undefined, {
            name: 'Basilisk Big Heap Memory Test',
            type: 'basilisk-debug',
            request: 'launch',
            program: heapMain,
            python: pythonPath,
            stopOnEntry: false,
            justMyCode: true,
            console: 'internalConsole',
        });
        assert.ok(started, 'debug session should start');
        const session = await sessionPromise;
        const threadId = await stoppedPromise;

        // BP A: start tracemalloc in the live debuggee (the real start leg).
        const frameA = await currentStoppedFrameId();
        assert.ok(frameA !== null, 'should resolve a frame at the start anchor');
        const start = await vscode.commands.executeCommand<{ memorySessionId?: string; script?: string }>(
            'basilisk.memory.start',
            { tracebackDepth: 25 }
        );
        assert.ok(start.memorySessionId !== undefined && start.script !== undefined, 'start leg should mint a session + script');
        await evaluateInDebugSession(start.script, frameA);

        // Run to BP B — the big heap is built under tracemalloc on the way.
        // waitForStop() would resolve early (debugpy answers stackTrace even
        // for a RUNNING thread with a sampled frame), so poll the tracker-gated
        // currentStoppedFrameId until the breakpoint genuinely lands.
        await continueExecution(session, threadId);
        let frameB: number | null = null;
        const buildDeadline = Date.now() + HEAP_BUILD_STOP_TIMEOUT_MS;
        while (frameB === null && Date.now() < buildDeadline) {
            frameB = await currentStoppedFrameId();
            if (frameB === null) {
                await new Promise<void>((resolve) => setTimeout(resolve, STOP_POLL_INTERVAL_MS));
            }
        }
        assert.ok(frameB !== null, 'should resolve a frame at the ready anchor');

        // BP B: take the snapshot exactly as the command path does, and watch
        // the debug console for pydevd's evaluation-stall warning.
        const consoleCursor = debugOutputCursor(session.id);
        const snapCmd = await vscode.commands.executeCommand<{ script?: string }>(
            'basilisk.memory.snapshot',
            { memorySessionId: start.memorySessionId }
        );
        assert.ok(snapCmd.script !== undefined, 'snapshot leg should return a script');
        const output = await evaluateInDebugSession(snapCmd.script, frameB);
        assert.ok(output !== null, 'evaluate should return the snapshot output');
        const result = await vscode.commands.executeCommand<{ kind?: string; currentMemory?: number }>(
            'basilisk.memory.ingest',
            { memorySessionId: start.memorySessionId, output }
        );

        assert.strictEqual(result.kind, 'snapshot', 'the large-heap snapshot must still ingest');
        assert.ok(
            typeof result.currentMemory === 'number' && result.currentMemory > 50_000_000,
            `the ~600k-item heap must be measured, got ${String(result.currentMemory)}`
        );
        const consoleOut = debugOutputSince(session.id, consoleCursor);
        assert.ok(
            !consoleOut.includes(PYDEVD_STALL_WARNING),
            `the snapshot evaluate stalled past pydevd's ${String(PYDEVD_WARN_TIMEOUT_SECS)}s budget — the debug console got the ` +
            `user-visible timeout wall of text:\n${consoleOut.slice(0, 800)}`
        );

        await stopActiveDebugSession();
    });
});

// ── Zero-config debug start [VSIX-PYTHON-DEBUGGER-START] ─────────────────────
// Pure tests for the DebugConfigurationProvider's defaulting logic that lets
// "Run and Debug" / F5 start without a launch.json.
suite('Basilisk Debug Config Provider', () => {
    test('empty config + Python file synthesizes a current-file launch', () => {
        // VS Code passes a truly-empty {} when starting with no launch.json.
        const resolved = applyDebugConfigDefaults({} as vscode.DebugConfiguration, 'python');
        assert.strictEqual(resolved.type, 'basilisk-debug');
        assert.strictEqual(resolved.request, 'launch');
        assert.strictEqual(resolved.program, '${file}');
    });

    test('empty config + non-Python file is left untouched', () => {
        const empty = {} as vscode.DebugConfiguration;
        const resolved = applyDebugConfigDefaults(empty, 'rust');
        assert.strictEqual(resolved.type, undefined);
        assert.strictEqual(resolved.program, undefined);
    });

    test('launch config missing program defaults to the current file', () => {
        const resolved = applyDebugConfigDefaults(
            { name: 'x', type: 'basilisk-debug', request: 'launch' },
            'python'
        );
        assert.strictEqual(resolved.program, '${file}');
    });

    test('a complete config passes through unchanged', () => {
        const full = {
            name: 'x', type: 'basilisk-debug', request: 'launch', program: '/tmp/a.py',
        } as vscode.DebugConfiguration;
        const resolved = applyDebugConfigDefaults(full, 'python');
        assert.strictEqual(resolved.program, '/tmp/a.py');
    });

    // [PROFILE-LAUNCH-NOSTOP] #145: the global `basilisk.profiler.profileOnLaunch`
    // setting is a second, equivalent trigger of an auto-profiling run (see
    // shouldProfileOnLaunch). It must mark the launch so the DAP proxy strips
    // breakpoints — otherwise a plain F5 with the global setting on still halts
    // at user breakpoints, the exact dead-stop #145 forbids.
    test('global profiler.profileOnLaunch marks an ordinary launch as a profiling run (#145)', () => {
        const resolved = applyDebugConfigDefaults({} as vscode.DebugConfiguration, 'python', true);
        assert.strictEqual(resolved.type, 'basilisk-debug', 'still synthesizes a current-file launch');
        assert.strictEqual(
            resolved.profileOnLaunch,
            true,
            'global profile-on-launch must mark the launch so the proxy neutralises breakpoints (#145)',
        );
    });

    // dap-1: a "Run & Track Memory" launch is NOT a CPU run. The global CPU
    // setting must not stamp it `profileOnLaunch` — otherwise the proxy strips
    // its breakpoints and the CPU sampler auto-starts alongside tracemalloc,
    // both fighting over the single entry pause.
    test('global profiler.profileOnLaunch does NOT contaminate a memory-tracking launch (dap-1)', () => {
        const memory = {
            name: 'Run & Track Memory (Current File)', type: 'basilisk-debug', request: 'launch',
            program: '/tmp/a.py', memoryTrackOnLaunch: true,
        } as unknown as vscode.DebugConfiguration;
        const resolved = applyDebugConfigDefaults(memory, 'python', true);
        assert.notStrictEqual(
            resolved.profileOnLaunch,
            true,
            'a memory launch must never be stamped as a CPU profiling run, even with the global setting on (dap-1)',
        );
        assert.strictEqual(resolved.memoryTrackOnLaunch, true, 'the memory-tracking flag is preserved');
    });

    test('an explicit profiling launch stays marked (idempotent) and a normal launch is not', () => {
        const explicit = applyDebugConfigDefaults(
            { name: 'x', type: 'basilisk-debug', request: 'launch', program: '/tmp/a.py', profileOnLaunch: true },
            'python',
            false,
        );
        assert.strictEqual(explicit.profileOnLaunch, true, 'an explicit Run & Profile launch stays a profiling run');

        const normal = applyDebugConfigDefaults({} as vscode.DebugConfiguration, 'python', false);
        assert.notStrictEqual(normal.profileOnLaunch, true, 'ordinary F5 (global off) must remain a real debug session with breakpoints');
    });
});
