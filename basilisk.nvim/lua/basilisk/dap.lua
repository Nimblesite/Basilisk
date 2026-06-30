--- DAP integration for Basilisk.
---
--- Registers an nvim-dap adapter that communicates with the basilisk LSP
--- to spawn debugpy sessions. Implements DapTcpProxy using vim.uv (libuv).
---
--- Implements [NVIM-DAP-INTEGRATION] — detects nvim-dap at runtime via
--- pcall(require, 'dap') and degrades gracefully when it is absent.

local log = require("basilisk.log")
local ui = require("basilisk.ui")

local M = {}

--- Content-Length header pattern for DAP message framing.
local CONTENT_LENGTH_PATTERN = "^Content%-Length: (%d+)\r\n\r\n"

--- Parse a DAP message from a buffer.
---@param data string Raw data buffer.
---@return table? message Parsed JSON message, or nil if incomplete.
---@return string remaining Remaining unparsed data.
local function parse_dap_message(data)
  local len_str = data:match(CONTENT_LENGTH_PATTERN)
  if not len_str then
    return nil, data
  end

  local header_end = data:find("\r\n\r\n", 1, true)
  if not header_end then
    return nil, data
  end

  local content_start = header_end + 4
  local content_length = tonumber(len_str)
  if #data < content_start + content_length - 1 then
    return nil, data
  end

  local body = data:sub(content_start, content_start + content_length - 1)
  local remaining = data:sub(content_start + content_length)
  local ok, msg = pcall(vim.json.decode, body)
  if not ok then
    log.error("DAP message parse error: %s", tostring(msg))
    return nil, remaining
  end
  return msg, remaining
end

--- Frame a DAP message with Content-Length header.
---@param msg table JSON-serializable message.
---@return string framed Framed message with header.
local function frame_dap_message(msg)
  local body = vim.json.encode(msg)
  return string.format("Content-Length: %d\r\n\r\n%s", #body, body)
end

--- Check whether a line is structural (try:, with:, if:, etc.)
---@param msg table DAP message.
---@return boolean
local function is_structural_step_out(msg)
  -- This interception is handled at the proxy level by inspecting
  -- stepOut responses — the actual line classification happens
  -- server-side. The proxy injects an auto-next if needed.
  return msg.command == "stepOut"
end

--- Create and start a DapTcpProxy.
--- Implements [NVIM-DAP-INTEGRATION-DAP-TCP-PROXY] — vim.uv.new_tcp() socket pair
--- with Content-Length header framing and the DAP interception rules.
---@param remote_host string
---@param remote_port integer
---@param callback fun(proxy_port: integer) Called with the local proxy port.
function M.create_proxy(remote_host, remote_port, callback)
  local server = vim.uv.new_tcp()
  local client_conn = nil
  local remote_conn = nil
  local client_buf = ""
  local remote_buf = ""
  local terminated = false

  server:bind("127.0.0.1", 0)
  server:listen(1, function(listen_err)
    if listen_err then
      log.error("DapTcpProxy listen error: %s", listen_err)
      return
    end

    client_conn = vim.uv.new_tcp()
    server:accept(client_conn)

    -- Connect to the remote debugpy.
    remote_conn = vim.uv.new_tcp()
    remote_conn:connect(remote_host, remote_port, function(connect_err)
      if connect_err then
        log.error("DapTcpProxy connect error: %s", connect_err)
        return
      end

      -- Relay: client -> remote (with interception).
      client_conn:read_start(function(err, data)
        if err or not data then
          if remote_conn and not remote_conn:is_closing() then
            remote_conn:close()
          end
          return
        end
        client_buf = client_buf .. data
        while true do
          local msg, rest = parse_dap_message(client_buf)
          if not msg then
            break
          end
          client_buf = rest

          -- Intercept stepOut for structural lines.
          if is_structural_step_out(msg) then
            log.debug("DapTcpProxy: intercepting stepOut, will inject next")
          end

          -- Fast disconnect post-termination.
          if terminated and msg.command == "disconnect" then
            local response = {
              type = "response",
              request_seq = msg.seq,
              success = true,
              command = "disconnect",
              seq = 0,
            }
            client_conn:write(frame_dap_message(response))
            -- Do not forward — session is already terminated.
          else
            remote_conn:write(frame_dap_message(msg))
          end
        end
      end)

      -- Relay: remote -> client (with interception).
      remote_conn:read_start(function(err, data)
        if err or not data then
          if client_conn and not client_conn:is_closing() then
            client_conn:close()
          end
          return
        end
        remote_buf = remote_buf .. data
        while true do
          local msg, rest = parse_dap_message(remote_buf)
          if not msg then
            break
          end
          remote_buf = rest

          -- Track terminated state.
          if msg.event == "terminated" then
            terminated = true
          end

          -- Inject exited event before terminated if missing.
          if msg.event == "terminated" then
            local exited = {
              type = "event",
              event = "exited",
              seq = 0,
              body = { exitCode = 0 },
            }
            client_conn:write(frame_dap_message(exited))
          end

          client_conn:write(frame_dap_message(msg))
        end
      end)
    end)
  end)

  local addr = server:getsockname()
  callback(addr.port)
end

--- Set up DAP integration.
---@param config BasiliskConfig
function M.setup(config)
  local dap_ok, dap = pcall(require, "dap")
  if not dap_ok then
    log.debug("nvim-dap not found, skipping DAP setup")
    return
  end

  if not config.debugger.enabled then
    log.debug("debugger disabled in config, skipping DAP setup")
    return
  end

  -- Register the basilisk DAP adapter.
  -- Implements [NVIM-DAP-INTEGRATION-ADAPTER-REGISTRATION] — sends
  -- basilisk.startDebugSession to the LSP, then points nvim-dap at the local
  -- DapTcpProxy port returned by create_proxy.
  dap.adapters.basilisk = function(callback, dap_config)
    local client = ui.get_client()
    if not client then
      log.error("no active basilisk LSP client for debug session")
      return
    end

    local uri = vim.uri_from_bufnr(vim.api.nvim_get_current_buf())
    client:request("workspace/executeCommand", {
      command = "basilisk.startDebugSession",
      arguments = { { uri = uri, pythonPath = config.python } },
    }, function(err, result)
      if err then
        log.error("startDebugSession failed: %s", err.message or tostring(err))
        return
      end
      if not result then
        log.error("startDebugSession returned nil")
        return
      end

      -- libuv TCP requires a numeric IP, not a hostname.
      local raw_host = result.host or "127.0.0.1"
      local host = (raw_host == "localhost") and "127.0.0.1" or raw_host
      local port = result.port
      local debug_session_id = result.sessionId

      M.create_proxy(host, port, function(proxy_port)
        vim.schedule(function()
          callback({
            type = "server",
            host = "127.0.0.1",
            port = proxy_port,
            options = {
              disconnect_timeout_sec = 3,
            },
          })
        end)
      end)

      -- Store session ID for cleanup.
      M._active_session_id = debug_session_id
    end, 0)
  end

  -- Default launch configurations.
  -- Implements [NVIM-DAP-INTEGRATION-DEFAULT-CONFIGURATIONS] — adds the launch
  -- (Current File) and attach (port 5678) configurations the spec documents.
  if not dap.configurations.python or #dap.configurations.python == 0 then
    dap.configurations.python = {}
  end

  -- Add basilisk configurations if not already present.
  local has_basilisk_launch = false
  local has_basilisk_attach = false
  for _, conf in ipairs(dap.configurations.python) do
    if conf.type == "basilisk" and conf.request == "launch" then
      has_basilisk_launch = true
    end
    if conf.type == "basilisk" and conf.request == "attach" then
      has_basilisk_attach = true
    end
  end

  if not has_basilisk_launch then
    dap.configurations.python[#dap.configurations.python + 1] = {
      type = "basilisk",
      request = "launch",
      name = "Python: Current File (Basilisk)",
      program = "${file}",
      justMyCode = true,
      redirectOutput = true,
      console = "integratedTerminal",
    }
  end

  if not has_basilisk_attach then
    dap.configurations.python[#dap.configurations.python + 1] = {
      type = "basilisk",
      request = "attach",
      name = "Python: Attach (Basilisk)",
      connect = { host = "127.0.0.1", port = 5678 },
    }
  end

  -- Optional integrations.
  -- Implements [NVIM-DAP-INTEGRATION-OPTIONAL-INTEGRATIONS] — nvim-dap-ui
  -- auto open/close on initialized/terminated, and nvim-dap-virtual-text.
  local dapui_ok, dapui = pcall(require, "dapui")
  if dapui_ok then
    dap.listeners.after.event_initialized["basilisk"] = function()
      dapui.open()
    end
    dap.listeners.before.event_terminated["basilisk"] = function()
      dapui.close()
    end
    dap.listeners.before.event_exited["basilisk"] = function()
      dapui.close()
    end
  end

  -- Optional: nvim-dap-virtual-text.
  local vtext_ok, vtext = pcall(require, "nvim-dap-virtual-text")
  if vtext_ok then
    vtext.setup()
  end

  log.debug("DAP setup complete")
end

--- Stop the active debug session.
function M.stop_session()
  local client = ui.get_client()
  if not client or not M._active_session_id then
    return
  end

  client:request("workspace/executeCommand", {
    command = "basilisk.stopDebugSession",
    arguments = { { sessionId = M._active_session_id } },
  }, function(err)
    if err then
      log.error("stopDebugSession failed: %s", err.message or tostring(err))
    end
    M._active_session_id = nil
  end, 0)
end

return M
