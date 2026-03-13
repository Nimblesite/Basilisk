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

const EXTENSION_ID = 'basilisk-lang.basilisk';

/** Maximum time (ms) to wait for the LSP server to fully start. */
const SERVER_START_WAIT_MS = 10_000;

/** Maximum time (ms) for a debug session to start. */
const DEBUG_SESSION_TIMEOUT_MS = 15_000;

/** Maximum time (ms) to wait for a stopped event (breakpoint/step). */
const STOPPED_EVENT_TIMEOUT_MS = 10_000;

/** Path to the debug stepping fixture. */
const FIXTURE_DIR = path.resolve(__dirname, '../../src/test/fixtures');
const STEPPING_FIXTURE = path.join(FIXTURE_DIR, 'debug_stepping.py');

// ── Helpers ──────────────────────────────────────────────────────────────────

/**
 * Resolves the absolute path to the basilisk binary built from Cargo.
 */
function findBasiliskBinary(): string | undefined {
    const workspaceRoot = path.resolve(__dirname, '../../../..');
    const debugBinary = path.join(workspaceRoot, 'target', 'debug', 'basilisk');
    if (fs.existsSync(debugBinary)) {
        return debugBinary;
    }
    try {
        execFileSync('basilisk', ['--version'], { timeout: 5000 });
        return 'basilisk';
    } catch {
        return undefined;
    }
}

/**
 * Check if debugpy is installed in the system Python.
 */
function isDebugpyInstalled(): boolean {
    for (const python of ['python3', 'python']) {
        try {
            execFileSync(python, ['-c', 'import debugpy'], {
                timeout: 5000,
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
            execFileSync(python, ['--version'], { timeout: 5000, stdio: 'pipe' });
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
function checkPortListening(host: string, port: number, timeoutMs: number = 3000): Promise<boolean> {
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
function waitForDebugSessionStart(timeoutMs: number = DEBUG_SESSION_TIMEOUT_MS): Promise<vscode.DebugSession> {
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
function waitForDebugSessionEnd(timeoutMs: number = DEBUG_SESSION_TIMEOUT_MS): Promise<void> {
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
    stackFrames: Array<{
        id: number;
        name: string;
        source?: { path?: string };
        line: number;
        column: number;
    }>;
    totalFrames: number;
}> {
    return session.customRequest('stackTrace', {
        threadId,
        startFrame: 0,
        levels: 20,
    });
}

/**
 * Get the scopes for a given stack frame.
 */
async function getScopes(session: vscode.DebugSession, frameId: number): Promise<{
    scopes: Array<{
        name: string;
        variablesReference: number;
        expensive: boolean;
    }>;
}> {
    return session.customRequest('scopes', { frameId });
}

/**
 * Get variables for a given variables reference (scope or structured variable).
 */
async function getVariables(session: vscode.DebugSession, variablesReference: number): Promise<{
    variables: Array<{
        name: string;
        value: string;
        type?: string;
        variablesReference: number;
    }>;
}> {
    return session.customRequest('variables', { variablesReference });
}

/**
 * Evaluate an expression in the context of a stack frame (watch expression).
 */
async function evaluateExpression(
    session: vscode.DebugSession,
    expression: string,
    frameId: number,
    context: 'watch' | 'repl' | 'hover' = 'watch'
): Promise<{
    result: string;
    type?: string;
    variablesReference: number;
}> {
    return session.customRequest('evaluate', {
        expression,
        frameId,
        context,
    });
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
function waitForStop(timeoutMs: number = STOPPED_EVENT_TIMEOUT_MS): Promise<number> {
    return new Promise((resolve, reject) => {
        const timer = setTimeout(() => {
            clearInterval(poll);
            reject(new Error(`Timed out waiting for debugger to stop after ${timeoutMs}ms`));
        }, timeoutMs);

        const poll = setInterval(async () => {
            const session = vscode.debug.activeDebugSession;
            if (!session) {
                return;
            }
            try {
                const threadsResponse = await session.customRequest('threads');
                if (threadsResponse?.threads?.length > 0) {
                    const threadId = threadsResponse.threads[0].id;
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
        }, 100);
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

/**
 * Helper: Assert a local variable has the expected value string.
 */
async function assertLocalVariable(
    session: vscode.DebugSession,
    threadId: number,
    varName: string,
    expectedValue: string,
    message?: string
): Promise<void> {
    const variable = await getLocalVariable(session, threadId, varName);
    assert.ok(variable, `Variable '${varName}' not found in locals`);
    assert.strictEqual(
        variable.value,
        expectedValue,
        message ?? `Expected ${varName} = ${expectedValue}, got ${variable.value}`
    );
}

/**
 * Helper: Assert a watch expression evaluates to the expected result.
 */
async function assertWatch(
    session: vscode.DebugSession,
    threadId: number,
    expression: string,
    expectedResult: string,
    message?: string
): Promise<void> {
    const stack = await getStackTrace(session, threadId);
    const frameId = stack.stackFrames[0].id;
    const result = await evaluateExpression(session, expression, frameId, 'watch');
    assert.strictEqual(
        result.result,
        expectedResult,
        message ?? `Watch '${expression}': expected ${expectedResult}, got ${result.result}`
    );
}

/**
 * Helper: Assert the current line number in the top frame.
 */
async function assertCurrentLine(
    session: vscode.DebugSession,
    threadId: number,
    expectedLine: number,
    message?: string
): Promise<void> {
    const stack = await getStackTrace(session, threadId);
    assert.ok(stack.stackFrames.length > 0, 'Expected at least one stack frame');
    assert.strictEqual(
        stack.stackFrames[0].line,
        expectedLine,
        message ?? `Expected to be on line ${expectedLine}, but on line ${stack.stackFrames[0].line}`
    );
}

/**
 * Helper: Assert the current function name in the top frame.
 */
async function assertCurrentFunction(
    session: vscode.DebugSession,
    threadId: number,
    expectedName: string,
    message?: string
): Promise<void> {
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
    assert.ok(session, 'Debug session should start');

    const threadId = await stoppedPromise;
    assert.ok(threadId > 0, `Thread ID should be positive, got ${threadId}`);

    return { session, threadId };
}

// ── Test Suite ──────────────────────────────────────────────────────────────

suite('Debug Integration E2E Tests', () => {
    let basiliskBinary: string | undefined;
    let debugpyAvailable: boolean;
    let pythonPath: string | undefined;
    let tmpDir: string;

    suiteSetup(async function () {
        this.timeout(SERVER_START_WAIT_MS + 10_000);

        basiliskBinary = findBasiliskBinary();
        debugpyAvailable = isDebugpyInstalled();
        pythonPath = findPython();

        if (!basiliskBinary) {
            throw new Error(
                'Basilisk binary not found. Build with: cargo build -p basilisk-cli'
            );
        }
        if (!debugpyAvailable) {
            throw new Error(
                'debugpy not installed. Install with: pip install debugpy'
            );
        }
        if (!pythonPath) {
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
        if (vscode.debug.activeDebugSession) {
            await vscode.debug.stopDebugging();
        }
        if (tmpDir && fs.existsSync(tmpDir)) {
            fs.rmSync(tmpDir, { recursive: true, force: true });
        }
    });

    teardown(async () => {
        clearAllBreakpoints();
        if (vscode.debug.activeDebugSession) {
            await vscode.debug.stopDebugging();
            await new Promise<void>((resolve) => setTimeout(resolve, 500));
        }
    });

    // ────────────────────────────────────────────────────────────────────────
    // 1. Package.json contributes basilisk-debug
    // ────────────────────────────────────────────────────────────────────────

    test('LSP advertises startDebugSession and stopDebugSession commands', async function () {
        this.timeout(5_000);
        const ext = vscode.extensions.getExtension(EXTENSION_ID);
        assert.ok(ext, 'Extension must be installed');
        const debuggers = ext.packageJSON.contributes?.debuggers;
        assert.ok(debuggers, 'Extension must contribute debuggers');
        assert.ok(
            debuggers.some((d: { type: string }) => d.type === 'basilisk-debug'),
            'Extension must contribute basilisk-debug debugger type'
        );
    });

    test('basilisk-debug type has correct configuration attributes', async function () {
        this.timeout(5_000);
        const ext = vscode.extensions.getExtension(EXTENSION_ID);
        assert.ok(ext, 'Extension must be installed');

        const debuggerContrib = ext.packageJSON.contributes?.debuggers?.find(
            (d: { type: string }) => d.type === 'basilisk-debug'
        );
        assert.ok(debuggerContrib, 'basilisk-debug debugger must be contributed');
        assert.strictEqual(debuggerContrib.label, 'Python (Basilisk)');
        assert.ok(debuggerContrib.configurationAttributes?.launch, 'Launch config must be defined');
        assert.ok(debuggerContrib.configurationAttributes?.attach, 'Attach config must be defined');
        assert.ok(
            debuggerContrib.configurationAttributes?.launch?.properties?.program,
            'Launch must have program property'
        );
        assert.ok(
            debuggerContrib.configurationAttributes?.launch?.properties?.args,
            'Launch must have args property'
        );
        assert.ok(
            debuggerContrib.configurationAttributes?.launch?.properties?.justMyCode,
            'Launch must have justMyCode property'
        );
        assert.ok(
            debuggerContrib.configurationAttributes?.launch?.properties?.stopOnEntry,
            'Launch must have stopOnEntry property'
        );
        assert.ok(
            debuggerContrib.configurationAttributes?.launch?.properties?.python,
            'Launch must have python property'
        );
        assert.ok(
            debuggerContrib.configurationAttributes?.attach?.properties?.connect,
            'Attach must have connect property'
        );
    });

    // ────────────────────────────────────────────────────────────────────────
    // 2. LSP-level: start/stop debug session via raw LSP commands
    // ────────────────────────────────────────────────────────────────────────

    test('startDebugSession spawns debugpy on a TCP port', async function () {
        this.timeout(DEBUG_SESSION_TIMEOUT_MS);

        const result = await startDebugSession(pythonPath);
        assert.ok(result, 'Expected startDebugSession to return a result');
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

    test('stopDebugSession kills the debugpy process', async function () {
        this.timeout(DEBUG_SESSION_TIMEOUT_MS);

        const result = await startDebugSession(pythonPath);
        assert.ok(result.port > 0);

        const stopResult = await stopDebugSession(result.sessionId);
        assert.strictEqual(stopResult.stopped, true, 'Session should be reported as stopped');

        await new Promise<void>((resolve) => setTimeout(resolve, 500));
        const stillListening = await checkPortListening(result.host, result.port, 1000);
        assert.strictEqual(stillListening, false, `Port ${result.port} should stop listening`);
    });

    test('stopDebugSession with invalid sessionId returns stopped: false', async function () {
        this.timeout(5_000);
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

    test('startDebugSession with bad Python path returns error', async function () {
        this.timeout(DEBUG_SESSION_TIMEOUT_MS);
        try {
            await startDebugSession('/nonexistent/python3.99');
            assert.fail('Expected startDebugSession to throw with a bad Python path');
        } catch (err: unknown) {
            assert.ok(err, 'Expected an error to be thrown');
            const message = err instanceof Error ? err.message : String(err);
            assert.ok(message.length > 0, `Expected a meaningful error message, got: "${message}"`);
        }
    });

    // ────────────────────────────────────────────────────────────────────────
    // 3. Full DAP handshake test
    // ────────────────────────────────────────────────────────────────────────

    test('full debug lifecycle: start, verify DAP handshake, stop', async function () {
        this.timeout(DEBUG_SESSION_TIMEOUT_MS + 5_000);

        const session = await startDebugSession(pythonPath);

        const dapResponse = await new Promise<string>((resolve, reject) => {
            const socket = new net.Socket();
            const timer = setTimeout(() => {
                socket.destroy();
                reject(new Error('DAP handshake timed out'));
            }, 5000);

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
                if (headerEnd === -1) return;
                const header = data.slice(0, headerEnd);
                const match = header.match(/Content-Length:\s*(\d+)/i);
                if (!match) return;
                const contentLength = parseInt(match[1], 10);
                const bodyStart = headerEnd + 4;
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

        const parsed = JSON.parse(dapResponse);
        assert.ok(parsed, 'Expected a valid JSON DAP response');
        assert.ok(
            parsed.type === 'response' || parsed.type === 'event',
            `Expected DAP response or event, got type: ${parsed.type}`
        );

        if (parsed.type === 'response') {
            assert.strictEqual(parsed.command, 'initialize', 'Should be initialize response');
            assert.strictEqual(parsed.success, true, 'Initialize should succeed');
            assert.ok(parsed.body, 'Initialize response should have a body');
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
        this.timeout(30_000);

        // Break on line 11: x = 10
        const { session, threadId } = await launchAndWaitForBreakpoint([11], pythonPath);

        // Stopped at line 11: x = 10 (not yet executed)
        await assertCurrentLine(session, threadId, 11);
        await assertCurrentFunction(session, threadId, 'arithmetic');

        // Step over: execute x = 10, now on line 12
        await stepOver(session, threadId);
        const tid2 = await waitForStop();
        await assertCurrentLine(session, tid2, 12);
        await assertLocalVariable(session, tid2, 'x', '10');

        // Step over: execute y = 20, now on line 13
        await stepOver(session, tid2);
        const tid3 = await waitForStop();
        await assertCurrentLine(session, tid3, 13);
        await assertLocalVariable(session, tid3, 'x', '10');
        await assertLocalVariable(session, tid3, 'y', '20');

        // Step over: execute z = x + y, now on line 14
        await stepOver(session, tid3);
        const tid4 = await waitForStop();
        await assertCurrentLine(session, tid4, 14);
        await assertLocalVariable(session, tid4, 'z', '30');

        // Watch expressions
        await assertWatch(session, tid4, 'x + y', '30');
        await assertWatch(session, tid4, 'z == 30', 'True');
        await assertWatch(session, tid4, 'type(z).__name__', "'int'");

        // Step over: execute w = z * 2, now on line 15
        await stepOver(session, tid4);
        const tid5 = await waitForStop();
        await assertCurrentLine(session, tid5, 15);
        await assertLocalVariable(session, tid5, 'w', '60');

        // Step over: execute result = w - 5, now on line 16
        await stepOver(session, tid5);
        const tid6 = await waitForStop();
        await assertCurrentLine(session, tid6, 16);
        await assertLocalVariable(session, tid6, 'result', '55');

        // Watch: verify final computed value
        await assertWatch(session, tid6, 'result == 55', 'True');
        await assertWatch(session, tid6, 'result * 2', '110');
    });

    // ────────────────────────────────────────────────────────────────────────
    // 5. REAL DEBUG SESSION: String operations
    // ────────────────────────────────────────────────────────────────────────

    test('string_ops: step through and assert string values', async function () {
        this.timeout(30_000);

        const { session, threadId } = await launchAndWaitForBreakpoint([21], pythonPath);

        await assertCurrentLine(session, threadId, 21);
        await assertCurrentFunction(session, threadId, 'string_ops');

        // Step: greeting = "hello"
        await stepOver(session, threadId);
        const t1 = await waitForStop();
        await assertLocalVariable(session, t1, 'greeting', "'hello'");

        // Step: name = "world"
        await stepOver(session, t1);
        const t2 = await waitForStop();
        await assertLocalVariable(session, t2, 'name', "'world'");

        // Step: message = greeting + " " + name
        await stepOver(session, t2);
        const t3 = await waitForStop();
        await assertLocalVariable(session, t3, 'message', "'hello world'");

        // Watch: string operations
        await assertWatch(session, t3, 'len(message)', '11');
        await assertWatch(session, t3, 'message.startswith("hello")', 'True');
        await assertWatch(session, t3, '"world" in message', 'True');

        // Step: upper = message.upper()
        await stepOver(session, t3);
        const t4 = await waitForStop();
        await assertLocalVariable(session, t4, 'upper', "'HELLO WORLD'");

        // Step: length = len(upper)
        await stepOver(session, t4);
        const t5 = await waitForStop();
        await assertLocalVariable(session, t5, 'length', '11');

        // Watch: verify everything
        await assertWatch(session, t5, 'upper == "HELLO WORLD"', 'True');
        await assertWatch(session, t5, 'length == len(upper)', 'True');
    });

    // ────────────────────────────────────────────────────────────────────────
    // 6. REAL DEBUG SESSION: List operations
    // ────────────────────────────────────────────────────────────────────────

    test('list_ops: step through and assert list contents', async function () {
        this.timeout(30_000);

        const { session, threadId } = await launchAndWaitForBreakpoint([31], pythonPath);

        await assertCurrentLine(session, threadId, 31);
        await assertCurrentFunction(session, threadId, 'list_ops');

        // Step: items = [1, 2, 3]
        await stepOver(session, threadId);
        const t1 = await waitForStop();
        await assertLocalVariable(session, t1, 'items', '[1, 2, 3]');

        // Watch: list properties
        await assertWatch(session, t1, 'len(items)', '3');
        await assertWatch(session, t1, 'items[0]', '1');
        await assertWatch(session, t1, 'items[-1]', '3');
        await assertWatch(session, t1, 'sum(items)', '6');

        // Step: items.append(4)
        await stepOver(session, t1);
        const t2 = await waitForStop();
        await assertWatch(session, t2, 'len(items)', '4');
        await assertWatch(session, t2, 'items[-1]', '4');
        await assertWatch(session, t2, '4 in items', 'True');

        // Step: items.insert(0, 0)
        await stepOver(session, t2);
        const t3 = await waitForStop();
        await assertWatch(session, t3, 'items[0]', '0');
        await assertWatch(session, t3, 'len(items)', '5');

        // Step: total = sum(items)
        await stepOver(session, t3);
        const t4 = await waitForStop();
        await assertLocalVariable(session, t4, 'total', '10');

        // Step: count = len(items)
        await stepOver(session, t4);
        const t5 = await waitForStop();
        await assertLocalVariable(session, t5, 'count', '5');

        // Watch: final assertions
        await assertWatch(session, t5, 'total == sum(items)', 'True');
        await assertWatch(session, t5, 'count == len(items)', 'True');
    });

    // ────────────────────────────────────────────────────────────────────────
    // 7. REAL DEBUG SESSION: Dictionary operations
    // ────────────────────────────────────────────────────────────────────────

    test('dict_ops: step through and assert dict contents', async function () {
        this.timeout(30_000);

        const { session, threadId } = await launchAndWaitForBreakpoint([41], pythonPath);

        await assertCurrentLine(session, threadId, 41);
        await assertCurrentFunction(session, threadId, 'dict_ops');

        // Step: data = {"a": 1, "b": 2}
        await stepOver(session, threadId);
        const t1 = await waitForStop();

        // Watch: dict operations
        await assertWatch(session, t1, 'len(data)', '2');
        await assertWatch(session, t1, 'data["a"]', '1');
        await assertWatch(session, t1, 'data["b"]', '2');
        await assertWatch(session, t1, '"a" in data', 'True');
        await assertWatch(session, t1, '"c" in data', 'False');

        // Step: data["c"] = 3
        await stepOver(session, t1);
        const t2 = await waitForStop();
        await assertWatch(session, t2, 'len(data)', '3');
        await assertWatch(session, t2, 'data["c"]', '3');
        await assertWatch(session, t2, '"c" in data', 'True');

        // Step: keys = list(data.keys())
        await stepOver(session, t2);
        const t3 = await waitForStop();
        await assertWatch(session, t3, 'len(keys)', '3');

        // Step: total = sum(data.values())
        await stepOver(session, t3);
        const t4 = await waitForStop();
        await assertLocalVariable(session, t4, 'total', '6');
        await assertWatch(session, t4, 'total == sum(data.values())', 'True');

        // Step: has_a = "a" in data
        await stepOver(session, t4);
        const t5 = await waitForStop();
        await assertLocalVariable(session, t5, 'has_a', 'True');
    });

    // ────────────────────────────────────────────────────────────────────────
    // 8. REAL DEBUG SESSION: Step into nested function calls
    // ────────────────────────────────────────────────────────────────────────

    test('nested_call: step into function, verify call stack', async function () {
        this.timeout(30_000);

        const { session, threadId } = await launchAndWaitForBreakpoint([51], pythonPath);

        await assertCurrentLine(session, threadId, 51);
        await assertCurrentFunction(session, threadId, 'nested_call');

        // Step: a = 5
        await stepOver(session, threadId);
        const t1 = await waitForStop();
        await assertLocalVariable(session, t1, 'a', '5');

        // Step INTO: b = double(a) — should enter the double() function
        await stepIn(session, t1);
        const t2 = await waitForStop();
        await assertCurrentFunction(session, t2, 'double');

        // We're inside double(). Check the parameter.
        await assertLocalVariable(session, t2, 'n', '5');

        // Step over inside double: result = n * 2
        await stepOver(session, t2);
        const t3 = await waitForStop();
        await assertLocalVariable(session, t3, 'result', '10');
        await assertWatch(session, t3, 'result == n * 2', 'True');

        // Step out back to nested_call
        await stepOut(session, t3);
        const t4 = await waitForStop();
        await assertCurrentFunction(session, t4, 'nested_call');
        await assertLocalVariable(session, t4, 'b', '10');

        // Verify stack depth
        const stack = await getStackTrace(session, t4);
        assert.ok(stack.stackFrames.length >= 1, 'Should have at least 1 frame');
        assert.strictEqual(stack.stackFrames[0].name, 'nested_call');
    });

    // ────────────────────────────────────────────────────────────────────────
    // 9. REAL DEBUG SESSION: Loop stepping and accumulator verification
    // ────────────────────────────────────────────────────────────────────────

    test('loop_and_accumulate: step through loop, verify accumulator', async function () {
        this.timeout(45_000);

        const { session, threadId } = await launchAndWaitForBreakpoint([65], pythonPath);

        await assertCurrentLine(session, threadId, 65);
        await assertCurrentFunction(session, threadId, 'loop_and_accumulate');

        // Step: total = 0
        await stepOver(session, threadId);
        const t1 = await waitForStop();
        await assertLocalVariable(session, t1, 'total', '0');

        // Step into the for loop header
        await stepOver(session, t1);
        const t2 = await waitForStop();

        // Step through the loop body: total += i  (i=0)
        await stepOver(session, t2);
        const t3 = await waitForStop();
        await assertWatch(session, t3, 'total', '0'); // 0 + 0 = 0

        // Continue through iterations — step over the for line + body for i=1
        await stepOver(session, t3);
        const t4 = await waitForStop();
        await stepOver(session, t4);
        const t5 = await waitForStop();
        await assertWatch(session, t5, 'total', '1'); // 0 + 1 = 1

        // i=2
        await stepOver(session, t5);
        const t6 = await waitForStop();
        await stepOver(session, t6);
        const t7 = await waitForStop();
        await assertWatch(session, t7, 'total', '3'); // 1 + 2 = 3

        // i=3
        await stepOver(session, t7);
        const t8 = await waitForStop();
        await stepOver(session, t8);
        const t9 = await waitForStop();
        await assertWatch(session, t9, 'total', '6'); // 3 + 3 = 6

        // i=4
        await stepOver(session, t9);
        const t10 = await waitForStop();
        await stepOver(session, t10);
        const t11 = await waitForStop();
        await assertWatch(session, t11, 'total', '10'); // 6 + 4 = 10
    });

    // ────────────────────────────────────────────────────────────────────────
    // 10. REAL DEBUG SESSION: Conditional branches
    // ────────────────────────────────────────────────────────────────────────

    test('conditional_branches: verify correct branch taken', async function () {
        this.timeout(30_000);

        const { session, threadId } = await launchAndWaitForBreakpoint([74], pythonPath);

        await assertCurrentLine(session, threadId, 74);
        await assertCurrentFunction(session, threadId, 'conditional_branches');

        // Step: x = 42
        await stepOver(session, threadId);
        const t1 = await waitForStop();
        await assertLocalVariable(session, t1, 'x', '42');
        await assertWatch(session, t1, 'x > 100', 'False');
        await assertWatch(session, t1, 'x > 10', 'True');

        // Step: if x > 100 — should go to elif
        await stepOver(session, t1);
        const t2 = await waitForStop();

        // Step: elif x > 10 — should be true, enter that branch
        await stepOver(session, t2);
        const t3 = await waitForStop();

        // Step: label = "medium"
        await stepOver(session, t3);
        const t4 = await waitForStop();
        await assertLocalVariable(session, t4, 'label', "'medium'");

        // Watch: verify the branch result
        await assertWatch(session, t4, 'label == "medium"', 'True');
        await assertWatch(session, t4, 'label != "big"', 'True');
        await assertWatch(session, t4, 'label != "small"', 'True');
    });

    // ────────────────────────────────────────────────────────────────────────
    // 11. REAL DEBUG SESSION: Exception handling
    // ────────────────────────────────────────────────────────────────────────

    test('exception_handling: step through try/except, verify caught state', async function () {
        this.timeout(30_000);

        const { session, threadId } = await launchAndWaitForBreakpoint([86], pythonPath);

        await assertCurrentLine(session, threadId, 86);
        await assertCurrentFunction(session, threadId, 'exception_handling');

        // Step: caught = False
        await stepOver(session, threadId);
        const t1 = await waitForStop();
        await assertLocalVariable(session, t1, 'caught', 'False');

        // Step: error_msg = ""
        await stepOver(session, t1);
        const t2 = await waitForStop();
        await assertLocalVariable(session, t2, 'error_msg', "''");

        // Step into try block: value = 1 / 0 — this raises ZeroDivisionError
        await stepOver(session, t2);
        const t3 = await waitForStop();

        // Step: the exception is caught, now in the except handler
        await stepOver(session, t3);
        const t4 = await waitForStop();

        // Step: caught = True
        await stepOver(session, t4);
        const t5 = await waitForStop();
        await assertLocalVariable(session, t5, 'caught', 'True');

        // Step: error_msg = str(exc)
        await stepOver(session, t5);
        const t6 = await waitForStop();
        await assertWatch(session, t6, 'caught', 'True');
        await assertWatch(session, t6, 'len(error_msg) > 0', 'True');
    });

    // ────────────────────────────────────────────────────────────────────────
    // 12. REAL DEBUG SESSION: Type variety — verify type representations
    // ────────────────────────────────────────────────────────────────────────

    test('type_variety: verify different Python types in debugger', async function () {
        this.timeout(30_000);

        const { session, threadId } = await launchAndWaitForBreakpoint([98], pythonPath);

        await assertCurrentLine(session, threadId, 98);
        await assertCurrentFunction(session, threadId, 'type_variety');

        // an_int = 42
        await stepOver(session, threadId);
        const t1 = await waitForStop();
        await assertLocalVariable(session, t1, 'an_int', '42');
        await assertWatch(session, t1, 'type(an_int).__name__', "'int'");

        // a_float = 3.14
        await stepOver(session, t1);
        const t2 = await waitForStop();
        await assertLocalVariable(session, t2, 'a_float', '3.14');
        await assertWatch(session, t2, 'type(a_float).__name__', "'float'");

        // a_bool = True
        await stepOver(session, t2);
        const t3 = await waitForStop();
        await assertLocalVariable(session, t3, 'a_bool', 'True');
        await assertWatch(session, t3, 'type(a_bool).__name__', "'bool'");

        // a_none = None
        await stepOver(session, t3);
        const t4 = await waitForStop();
        await assertLocalVariable(session, t4, 'a_none', 'None');
        await assertWatch(session, t4, 'a_none is None', 'True');

        // a_tuple = (1, "two", 3.0)
        await stepOver(session, t4);
        const t5 = await waitForStop();
        await assertWatch(session, t5, 'len(a_tuple)', '3');
        await assertWatch(session, t5, 'a_tuple[0]', '1');
        await assertWatch(session, t5, 'type(a_tuple).__name__', "'tuple'");

        // a_set = {10, 20, 30}
        await stepOver(session, t5);
        const t6 = await waitForStop();
        await assertWatch(session, t6, 'len(a_set)', '3');
        await assertWatch(session, t6, '10 in a_set', 'True');
        await assertWatch(session, t6, 'type(a_set).__name__', "'set'");

        // a_bytes = b"hello"
        await stepOver(session, t6);
        const t7 = await waitForStop();
        await assertWatch(session, t7, 'len(a_bytes)', '5');
        await assertWatch(session, t7, 'type(a_bytes).__name__', "'bytes'");
    });

    // ────────────────────────────────────────────────────────────────────────
    // 13. REAL DEBUG SESSION: Class instance — check object attributes
    // ────────────────────────────────────────────────────────────────────────

    test('class_instance: step through, inspect object attributes', async function () {
        this.timeout(30_000);

        // Break at line 119: p = Point(3, 4)
        const { session, threadId } = await launchAndWaitForBreakpoint([119], pythonPath);

        await assertCurrentLine(session, threadId, 119);
        await assertCurrentFunction(session, threadId, 'class_instance');

        // Step over: p = Point(3, 4)
        await stepOver(session, threadId);
        const t1 = await waitForStop();

        // Verify object attributes via watch
        await assertWatch(session, t1, 'p.x', '3');
        await assertWatch(session, t1, 'p.y', '4');
        await assertWatch(session, t1, 'type(p).__name__', "'Point'");

        // Step: mag = p.magnitude()
        await stepOver(session, t1);
        const t2 = await waitForStop();
        await assertLocalVariable(session, t2, 'mag', '5.0');

        // Watch: verify computed value
        await assertWatch(session, t2, 'mag == 5.0', 'True');
        await assertWatch(session, t2, 'p.x ** 2 + p.y ** 2', '25');
    });

    // ────────────────────────────────────────────────────────────────────────
    // 14. REAL DEBUG SESSION: Multiple breakpoints, continue between them
    // ────────────────────────────────────────────────────────────────────────

    test('continue between multiple breakpoints', async function () {
        this.timeout(30_000);

        // Set breakpoints in arithmetic() and string_ops()
        const { session, threadId } = await launchAndWaitForBreakpoint([13, 23], pythonPath);

        // Should stop at line 13 first (z = x + y in arithmetic())
        await assertCurrentLine(session, threadId, 13);
        await assertCurrentFunction(session, threadId, 'arithmetic');

        // Verify x and y are set
        await assertLocalVariable(session, threadId, 'x', '10');
        await assertLocalVariable(session, threadId, 'y', '20');

        // Continue to next breakpoint — line 23 (message = ... in string_ops())
        await continueExecution(session, threadId);
        const t2 = await waitForStop();

        await assertCurrentLine(session, t2, 23);
        await assertCurrentFunction(session, t2, 'string_ops');
        await assertLocalVariable(session, t2, 'greeting', "'hello'");
        await assertLocalVariable(session, t2, 'name', "'world'");
    });

    // ────────────────────────────────────────────────────────────────────────
    // 15. REAL DEBUG SESSION: Stack trace depth verification
    // ────────────────────────────────────────────────────────────────────────

    test('stack trace shows correct call hierarchy', async function () {
        this.timeout(30_000);

        // Break inside double(), called from nested_call()
        const { session, threadId } = await launchAndWaitForBreakpoint([59], pythonPath);

        await assertCurrentLine(session, threadId, 59);
        await assertCurrentFunction(session, threadId, 'double');

        // Verify the call stack
        const stack = await getStackTrace(session, threadId);
        assert.ok(stack.stackFrames.length >= 2, `Stack should have >= 2 frames, got ${stack.stackFrames.length}`);

        // Top frame: double
        assert.strictEqual(stack.stackFrames[0].name, 'double');
        assert.strictEqual(stack.stackFrames[0].line, 59);

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
        this.timeout(30_000);

        const { session, threadId } = await launchAndWaitForBreakpoint([13], pythonPath);

        await assertCurrentLine(session, threadId, 13);

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
        this.timeout(30_000);

        // Stop at line 15 in arithmetic where x=10, y=20, z=30, w=60
        const { session, threadId } = await launchAndWaitForBreakpoint([15], pythonPath);

        await assertCurrentLine(session, threadId, 15);

        // Arithmetic watch expressions
        await assertWatch(session, threadId, 'x', '10');
        await assertWatch(session, threadId, 'y', '20');
        await assertWatch(session, threadId, 'z', '30');
        await assertWatch(session, threadId, 'w', '60');

        // Computed expressions
        await assertWatch(session, threadId, 'x + y + z', '60');
        await assertWatch(session, threadId, 'w // x', '6');
        await assertWatch(session, threadId, 'w % 7', '4');
        await assertWatch(session, threadId, 'w ** 0', '1');
        await assertWatch(session, threadId, 'abs(-w)', '60');
        await assertWatch(session, threadId, 'min(x, y, z, w)', '10');
        await assertWatch(session, threadId, 'max(x, y, z, w)', '60');
        await assertWatch(session, threadId, 'sorted([w, z, y, x])', '[10, 20, 30, 60]');

        // Boolean expressions
        await assertWatch(session, threadId, 'x < y', 'True');
        await assertWatch(session, threadId, 'x > y', 'False');
        await assertWatch(session, threadId, 'x == 10 and y == 20', 'True');
        await assertWatch(session, threadId, 'z == x + y', 'True');
        await assertWatch(session, threadId, 'w == z * 2', 'True');

        // Type checking via watch
        await assertWatch(session, threadId, 'isinstance(x, int)', 'True');
        await assertWatch(session, threadId, 'isinstance(x, str)', 'False');

        // String formatting via watch
        await assertWatch(session, threadId, 'f"{x} + {y} = {z}"', "'10 + 20 = 30'");

        // List comprehension via watch
        await assertWatch(session, threadId, '[v * 2 for v in [x, y, z]]', '[20, 40, 60]');
    });

    // ────────────────────────────────────────────────────────────────────────
    // 18. REAL DEBUG SESSION: Hover-style evaluation
    // ────────────────────────────────────────────────────────────────────────

    test('hover evaluation: evaluate expressions in hover context', async function () {
        this.timeout(30_000);

        const { session, threadId } = await launchAndWaitForBreakpoint([13], pythonPath);

        const stack = await getStackTrace(session, threadId);
        const frameId = stack.stackFrames[0].id;

        // Hover evaluation (simulates mouse hover in editor)
        const hoverResult = await evaluateExpression(session, 'x', frameId, 'hover');
        assert.strictEqual(hoverResult.result, '10');
        assert.ok(hoverResult.type, 'Hover result should include type info');

        const hoverResult2 = await evaluateExpression(session, 'y', frameId, 'hover');
        assert.strictEqual(hoverResult2.result, '20');
    });

    // ────────────────────────────────────────────────────────────────────────
    // 19. REAL DEBUG SESSION: REPL evaluation
    // ────────────────────────────────────────────────────────────────────────

    test('REPL evaluation: evaluate expressions in debug console context', async function () {
        this.timeout(30_000);

        const { session, threadId } = await launchAndWaitForBreakpoint([13], pythonPath);

        const stack = await getStackTrace(session, threadId);
        const frameId = stack.stackFrames[0].id;

        // REPL evaluation (simulates Debug Console)
        const replResult = await evaluateExpression(session, 'x + y', frameId, 'repl');
        assert.strictEqual(replResult.result, '30');

        const replResult2 = await evaluateExpression(session, '[x, y]', frameId, 'repl');
        assert.strictEqual(replResult2.result, '[10, 20]');

        const replResult3 = await evaluateExpression(session, 'dict(a=x, b=y)', frameId, 'repl');
        assert.ok(replResult3.result.includes('a'), 'REPL dict result should contain key a');
        assert.ok(replResult3.result.includes('b'), 'REPL dict result should contain key b');
    });

    // ────────────────────────────────────────────────────────────────────────
    // 20. Debug session terminates cleanly
    // ────────────────────────────────────────────────────────────────────────

    test('debug session terminates cleanly after continue past end', async function () {
        this.timeout(30_000);

        // Break at the return of arithmetic()
        const { session, threadId } = await launchAndWaitForBreakpoint([16], pythonPath);

        await assertCurrentLine(session, threadId, 16);

        const endPromise = waitForDebugSessionEnd(15_000);

        // Continue — the program will run through remaining functions and exit
        await continueExecution(session, threadId);

        await endPromise;

        // VS Code may not clear activeDebugSession synchronously with the
        // terminate event — poll briefly to let the runtime settle.
        for (let i = 0; i < 20 && vscode.debug.activeDebugSession; i++) {
            await new Promise<void>((r) => setTimeout(r, 100));
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

    test('attach to manually spawned debugpy server', async function () {
        this.timeout(30_000);

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
        assert.ok(attachSession, 'Attach session should be created');

        // Clean up
        if (vscode.debug.activeDebugSession) {
            await vscode.debug.stopDebugging();
            await new Promise<void>((resolve) => setTimeout(resolve, 500));
        }
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
});
