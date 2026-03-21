--- DAP integration test helpers for basilisk.nvim.
---
--- Wraps nvim-dap API for E2E debug session testing.
--- Requires: basilisk binary, debugpy, nvim-dap, Python 3.

local lsp_helpers = require("tests.lsp.helpers")

local M = {}

--- Timeout constants.
M.DEBUG_SESSION_TIMEOUT_MS = 15000
M.STOPPED_EVENT_TIMEOUT_MS = 10000

--- Path to the shared debug stepping fixture.
---@return string? path Absolute path to debug_stepping.py, or nil.
function M.fixture_path()
  -- Try multiple resolution strategies.
  local candidates = {
    -- From cwd (when running `make test-dap` from basilisk.nvim/).
    vim.fn.fnamemodify(".", ":p") .. "../vscode-extension/src/test/fixtures/debug_stepping.py",
    -- From BASILISK_REPO_ROOT env var (CI).
    (vim.env.BASILISK_REPO_ROOT or "") .. "/vscode-extension/src/test/fixtures/debug_stepping.py",
    -- Absolute fallback from this file.
    vim.fn.fnamemodify(debug.getinfo(1, "S").source:sub(2), ":p:h:h:h:h")
      .. "/vscode-extension/src/test/fixtures/debug_stepping.py",
  }
  for _, path in ipairs(candidates) do
    local resolved = vim.fn.fnamemodify(path, ":p")
    if vim.fn.filereadable(resolved) == 1 then
      return resolved
    end
  end
  return nil
end

--- Check if debugpy is importable by the system Python.
---@return boolean
function M.is_debugpy_installed()
  for _, python in ipairs({ "python3", "python" }) do
    local ok = os.execute(python .. " -c 'import debugpy' 2>/dev/null")
    if ok then
      return true
    end
  end
  return false
end

--- Wait for a DAP session to enter 'stopped' state (breakpoint or step).
---@param timeout_ms? integer
---@return boolean stopped
function M.wait_for_stopped(timeout_ms)
  timeout_ms = timeout_ms or M.STOPPED_EVENT_TIMEOUT_MS
  return lsp_helpers.poll_until(function()
    local session = require("dap").session()
    return session ~= nil and session.stopped_thread_id ~= nil
  end, timeout_ms, "DAP stopped event")
end

--- Wait for a DAP session to become active (initialized).
---@param timeout_ms? integer
---@return boolean active
function M.wait_for_session(timeout_ms)
  timeout_ms = timeout_ms or M.DEBUG_SESSION_TIMEOUT_MS
  return lsp_helpers.poll_until(function()
    return require("dap").session() ~= nil
  end, timeout_ms, "DAP session")
end

--- Wait for the DAP session to terminate.
---@param timeout_ms? integer
---@return boolean terminated
function M.wait_for_terminated(timeout_ms)
  timeout_ms = timeout_ms or M.DEBUG_SESSION_TIMEOUT_MS
  return lsp_helpers.poll_until(function()
    return require("dap").session() == nil
  end, timeout_ms, "DAP terminated")
end

--- Get the variables in the current (topmost) frame's locals scope.
---
--- Must be called while the session is stopped.
---@param timeout_ms? integer
---@return table<string, string> variables Map of name → value (as string).
function M.get_local_variables(timeout_ms)
  timeout_ms = timeout_ms or 5000
  local dap = require("dap")
  local session = dap.session()
  if not session then
    return {}
  end

  -- Get the topmost stack frame.
  local frames = {}
  local frames_done = false
  session:request("stackTrace", {
    threadId = session.stopped_thread_id,
    startFrame = 0,
    levels = 1,
  }, function(err, response)
    if not err and response and response.stackFrames then
      frames = response.stackFrames
    end
    frames_done = true
  end)
  vim.wait(timeout_ms, function()
    return frames_done
  end)

  if #frames == 0 then
    return {}
  end

  -- Get scopes for the top frame.
  local scopes = {}
  local scopes_done = false
  session:request("scopes", {
    frameId = frames[1].id,
  }, function(err, response)
    if not err and response and response.scopes then
      scopes = response.scopes
    end
    scopes_done = true
  end)
  vim.wait(timeout_ms, function()
    return scopes_done
  end)

  -- Find the "Locals" scope.
  local locals_ref = nil
  for _, scope in ipairs(scopes) do
    if scope.name == "Locals" then
      locals_ref = scope.variablesReference
      break
    end
  end
  if not locals_ref then
    return {}
  end

  -- Get variables.
  local vars = {}
  local vars_done = false
  session:request("variables", {
    variablesReference = locals_ref,
  }, function(err, response)
    if not err and response and response.variables then
      for _, v in ipairs(response.variables) do
        vars[v.name] = v.value
      end
    end
    vars_done = true
  end)
  vim.wait(timeout_ms, function()
    return vars_done
  end)

  return vars
end

--- Send a DAP step request and wait for the next stopped event.
---@param step_type "next"|"stepIn"|"stepOut"
---@param timeout_ms? integer
---@return boolean stopped
function M.step_and_wait(step_type, timeout_ms)
  timeout_ms = timeout_ms or M.STOPPED_EVENT_TIMEOUT_MS
  local dap = require("dap")
  local session = dap.session()
  if not session then
    return false
  end

  -- Clear stopped state.
  session.stopped_thread_id = nil

  -- Issue the step command.
  if step_type == "next" then
    dap.step_over()
  elseif step_type == "stepIn" then
    dap.step_into()
  elseif step_type == "stepOut" then
    dap.step_out()
  end

  return M.wait_for_stopped(timeout_ms)
end

--- Continue execution and wait for the next stopped event (breakpoint).
---@param timeout_ms? integer
---@return boolean stopped
function M.continue_and_wait(timeout_ms)
  timeout_ms = timeout_ms or M.STOPPED_EVENT_TIMEOUT_MS
  local dap = require("dap")
  local session = dap.session()
  if not session then
    return false
  end

  session.stopped_thread_id = nil
  dap.continue()

  return M.wait_for_stopped(timeout_ms)
end

--- Evaluate an expression in the debug console.
---@param expression string
---@param timeout_ms? integer
---@return string? result The evaluated result as a string.
function M.evaluate(expression, timeout_ms)
  timeout_ms = timeout_ms or 5000
  local dap = require("dap")
  local session = dap.session()
  if not session then
    return nil
  end

  -- Get the current frame ID for evaluation context.
  local frame_id = nil
  local frame_done = false
  session:request("stackTrace", {
    threadId = session.stopped_thread_id,
    startFrame = 0,
    levels = 1,
  }, function(err, response)
    if not err and response and response.stackFrames and #response.stackFrames > 0 then
      frame_id = response.stackFrames[1].id
    end
    frame_done = true
  end)
  vim.wait(timeout_ms, function()
    return frame_done
  end)

  local result_str = nil
  local done = false
  session:request("evaluate", {
    expression = expression,
    frameId = frame_id,
    context = "repl",
  }, function(err, response)
    if not err and response then
      result_str = response.result
    end
    done = true
  end)
  vim.wait(timeout_ms, function()
    return done
  end)

  return result_str
end

--- Get the current stack trace.
---@param timeout_ms? integer
---@return table[] frames List of stack frame objects.
function M.get_stack_trace(timeout_ms)
  timeout_ms = timeout_ms or 5000
  local dap = require("dap")
  local session = dap.session()
  if not session then
    return {}
  end

  local frames = {}
  local done = false
  session:request("stackTrace", {
    threadId = session.stopped_thread_id,
  }, function(err, response)
    if not err and response and response.stackFrames then
      frames = response.stackFrames
    end
    done = true
  end)
  vim.wait(timeout_ms, function()
    return done
  end)

  return frames
end

--- Clean up any active DAP session.
function M.cleanup_session()
  local dap_ok, dap_mod = pcall(require, "dap")
  if not dap_ok then
    return
  end

  local session = dap_mod.session()
  if not session then
    return
  end

  -- Send disconnect directly to avoid interactive prompts in headless mode.
  pcall(function()
    session:disconnect({ terminateDebuggee = true })
  end)
  M.wait_for_terminated(5000)

  -- Force-close if disconnect didn't work.
  if dap_mod.session() then
    pcall(function()
      session:close()
    end)
    vim.wait(1000)
  end
end

return M
