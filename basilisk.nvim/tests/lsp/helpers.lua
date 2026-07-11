--- Real LSP integration test helpers.
---
--- Starts the REAL basilisk LSP server, creates temp Python files,
--- and provides polling utilities for diagnostics, hover, etc.
--- NO MOCKING. Uses the actual basilisk binary.

local M = {}

--- Timeout constants (matching VS Code test-helpers.ts).
M.DIAGNOSTIC_TIMEOUT_MS = 15000
M.NO_DIAGNOSTIC_WAIT_MS = 5000
M.SERVER_START_WAIT_MS = 10000

--- Resolve the basilisk binary path.
---@return string? path
function M.find_binary()
  -- 1. BASILISK_EXECUTABLE_PATH env var (for CI).
  local env = vim.env.BASILISK_EXECUTABLE_PATH
  if env and env ~= "" and vim.fn.executable(env) == 1 then
    return env
  end

  -- 2. target/debug/basilisk (repo build).
  local repo_root = vim.fn.fnamemodify(
    debug.getinfo(1, "S").source:sub(2),
    ":h:h:h"
  )
  local candidates = {
    repo_root .. "/target/debug/basilisk",
    repo_root .. "/target/release/basilisk",
    vim.fn.expand("~/.cargo/bin/basilisk"),
  }
  for _, path in ipairs(candidates) do
    if vim.fn.executable(path) == 1 then
      return path
    end
  end

  -- 3. PATH.
  local on_path = vim.fn.exepath("basilisk")
  if on_path ~= "" then
    return on_path
  end

  return nil
end

--- Create a temporary directory for test fixtures.
---@return string path
function M.create_tmpdir()
  local tmpdir = vim.fn.tempname() .. "-basilisk-test"
  vim.fn.mkdir(tmpdir, "p")
  return tmpdir
end

--- Write a Python file to the temp directory and open it in a buffer.
---@param tmpdir string
---@param filename string
---@param content string
---@return integer buf, string uri, string filepath
function M.open_python_file(tmpdir, filename, content)
  local filepath = tmpdir .. "/" .. filename
  local fh = io.open(filepath, "w")
  assert(fh, "failed to create test file: " .. filepath)
  fh:write(content)
  fh:close()

  vim.cmd("edit " .. vim.fn.fnameescape(filepath))
  local buf = vim.api.nvim_get_current_buf()
  -- Use uri_from_bufnr to get the canonical URI (handles macOS /var → /private/var symlinks).
  local uri = vim.uri_from_bufnr(buf)
  return buf, uri, filepath
end

--- Replace the content of a buffer.
---@param buf integer
---@param content string
function M.replace_content(buf, content)
  local lines = vim.split(content, "\n", { plain = true })
  vim.api.nvim_buf_set_lines(buf, 0, -1, false, lines)
  -- Trigger didChange.
  vim.cmd("doautocmd TextChanged")
end

--- Poll until a condition is met, or timeout.
---@param condition fun(remaining_ms: integer): boolean
---@param timeout_ms integer
---@param desc? string Description for error message.
---@return boolean success
function M.poll_until(condition, timeout_ms, desc)
  local interval = 100
  local deadline = vim.uv.hrtime() + timeout_ms * 1000000

  while vim.uv.hrtime() < deadline do
    local remaining_ms = math.max(1, math.ceil((deadline - vim.uv.hrtime()) / 1000000))
    if condition(remaining_ms) then
      return true
    end

    local remaining_ns = deadline - vim.uv.hrtime()
    if remaining_ns <= 0 then
      break
    end
    vim.wait(math.min(interval, math.ceil(remaining_ns / 1000000)))
  end
  return false
end

--- Wait for diagnostics to appear on a buffer.
---@param buf integer
---@param timeout_ms? integer Default DIAGNOSTIC_TIMEOUT_MS.
---@return vim.Diagnostic[]
function M.wait_for_diagnostics(buf, timeout_ms)
  timeout_ms = timeout_ms or M.DIAGNOSTIC_TIMEOUT_MS
  local diags = {}
  M.poll_until(function()
    diags = vim.diagnostic.get(buf)
    return #diags > 0
  end, timeout_ms, "diagnostics")
  return diags
end

--- Wait for diagnostics to clear on a buffer.
---@param buf integer
---@param timeout_ms? integer Default NO_DIAGNOSTIC_WAIT_MS.
---@return boolean cleared
function M.wait_for_diagnostics_cleared(buf, timeout_ms)
  timeout_ms = timeout_ms or M.NO_DIAGNOSTIC_WAIT_MS
  return M.poll_until(function()
    return #vim.diagnostic.get(buf) == 0
  end, timeout_ms, "diagnostics cleared")
end

--- Wait for the basilisk LSP client to attach to a buffer.
---@param buf integer
---@param timeout_ms? integer Default SERVER_START_WAIT_MS.
---@return vim.lsp.Client?
function M.wait_for_client(buf, timeout_ms)
  timeout_ms = timeout_ms or M.SERVER_START_WAIT_MS
  local client = nil
  M.poll_until(function()
    local clients = vim.lsp.get_clients({ name = "basilisk", bufnr = buf })
    if #clients > 0 then
      client = clients[1]
      return true
    end
    return false
  end, timeout_ms, "LSP client attach")
  return client
end

--- Wait for the LSP server to be fully ready (responds to documentSymbol).
---@param buf integer
---@param timeout_ms? integer
---@return boolean ready
function M.wait_for_server_ready(buf, timeout_ms)
  timeout_ms = timeout_ms or M.SERVER_START_WAIT_MS
  local client = M.wait_for_client(buf, timeout_ms)
  if not client then
    return false
  end

  local ready = false
  M.poll_until(function(remaining_ms)
    local result = nil
    local done = false
    client:request("textDocument/documentSymbol", {
      textDocument = { uri = vim.uri_from_bufnr(buf) },
    }, function(err, res)
      result = res
      done = true
    end, buf)
    -- Wait for the response.
    vim.wait(math.min(3000, remaining_ms), function()
      return done
    end)
    if done then
      ready = true
      return true
    end
    return false
  end, timeout_ms, "server ready")
  return ready
end

--- Send an LSP request and wait for the response.
---@param client vim.lsp.Client
---@param method string
---@param params table
---@param buf integer
---@param timeout_ms? integer
---@return any? err, any? result
function M.lsp_request(client, method, params, buf, timeout_ms)
  timeout_ms = timeout_ms or 5000
  local err_result = nil
  local ok_result = nil
  local done = false

  client:request(method, params, function(err, result)
    err_result = err
    ok_result = result
    done = true
  end, buf)

  vim.wait(timeout_ms, function()
    return done
  end)

  if not done then
    return { message = "request timed out: " .. method }, nil
  end
  return err_result, ok_result
end

--- Close all open buffers (cleanup between tests).
function M.close_all_buffers()
  for _, win in ipairs(vim.api.nvim_list_wins()) do
    local ok, config = pcall(vim.api.nvim_win_get_config, win)
    if ok and config.relative ~= "" then
      pcall(vim.api.nvim_win_close, win, true)
    end
  end

  for _, buf in ipairs(vim.api.nvim_list_bufs()) do
    if vim.api.nvim_buf_is_valid(buf) and vim.api.nvim_buf_is_loaded(buf) then
      pcall(vim.api.nvim_buf_delete, buf, { force = true })
    end
  end
end

--- Stop all basilisk LSP clients.
function M.stop_clients()
  for _, client in ipairs(vim.lsp.get_clients({ name = "basilisk" })) do
    client:stop(true)
  end
  -- Wait for clients to stop.
  vim.wait(2000, function()
    return #vim.lsp.get_clients({ name = "basilisk" }) == 0
  end)
end

--- Clean up a temp directory.
---@param tmpdir string
function M.cleanup_tmpdir(tmpdir)
  if tmpdir and vim.fn.isdirectory(tmpdir) == 1 then
    vim.fn.delete(tmpdir, "rf")
  end
end

return M
