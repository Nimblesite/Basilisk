--- User commands for Basilisk.
---
--- All profiling/memory/test/uv LSP commands (defined in LSP-ARCHITECTURE-SPEC.md)
--- surface as :Basilisk* user commands.

local log = require("basilisk.log")
local ui = require("basilisk.ui")

local M = {}

--- Map LSP message types to Neovim notification levels.
---@param message_type integer?
---@return integer
local function lsp_message_level(message_type)
  local message_types = vim.lsp.protocol.MessageType
  if message_type == message_types.Error then
    return vim.log.levels.ERROR
  end
  if message_type == message_types.Warning then
    return vim.log.levels.WARN
  end
  return vim.log.levels.INFO
end

--- Send an LSP server message through the plugin logger.
---@param result table?
---@param show_info boolean
local function log_lsp_message(result, show_info)
  if not result or type(result.message) ~= "string" or result.message == "" then
    return
  end
  local message = result.message:gsub("^Basilisk:%s*", "")
  local level = lsp_message_level(result.type)
  if level == vim.log.levels.ERROR then
    log.error("%s", message)
  elseif level == vim.log.levels.WARN then
    log.warn("%s", message)
  elseif show_info then
    log.info("%s", message)
  else
    log.debug("%s", message)
  end
end

--- Install Basilisk message handlers on already-running clients.
local function install_message_handlers()
  for _, client in ipairs(vim.lsp.get_clients({ name = "basilisk" })) do
    client.handlers = client.handlers or {}
    client.handlers["window/logMessage"] = function(_err, result)
      log_lsp_message(result, false)
    end
    client.handlers["window/showMessage"] = function(_err, result)
      log_lsp_message(result, true)
    end
  end
end

--- Send an LSP executeCommand request.
---@param command string
---@param args? table
---@param callback? fun(err: any, result: any)
local function execute_command(command, args, callback)
  local client = ui.get_client()
  if not client then
    log.warn("no active LSP client")
    return
  end
  install_message_handlers()
  client:request("workspace/executeCommand", {
    command = command,
    arguments = args or {},
  }, callback, 0)
end


--- Register all :Basilisk* commands.
---@param config BasiliskConfig
function M.register(config)
  local lsp_mod = require("basilisk.lsp")
  local profiling = require("basilisk.profiling")
  local memory = require("basilisk.memory")
  local testing = require("basilisk.testing")
  install_message_handlers()
  if lsp_mod.install_handlers then
    lsp_mod.install_handlers()
  end

  -- Core commands.

  vim.api.nvim_create_user_command("BasiliskRestart", function()
    lsp_mod.reset_restart_count()
    lsp_mod.restart(config, true)
    log.info("restarting server...")
  end, { desc = "Restart the Basilisk LSP server" })

  local info_panel = require("basilisk.info")

  vim.api.nvim_create_user_command("BasiliskInfo", function()
    info_panel.show(config)
  end, { desc = "Show Basilisk LSP server info" })

  vim.api.nvim_create_user_command("BasiliskOrganizeImports", function()
    local uri = vim.uri_from_bufnr(vim.api.nvim_get_current_buf())
    execute_command("basilisk.organizeImports", { uri })
  end, { desc = "Organize imports via Basilisk" })

  vim.api.nvim_create_user_command("BasiliskFixFile", function()
    local uri = vim.uri_from_bufnr(vim.api.nvim_get_current_buf())
    execute_command("basilisk.fixFile", { uri }, function(err)
      if err then
        log.error("fix file failed: %s", err.message or tostring(err))
      else
        log.info("file fixed")
      end
    end)
  end, { desc = "Fix all diagnostics in current file" })

  vim.api.nvim_create_user_command("BasiliskFixWorkspace", function()
    execute_command("basilisk.fixWorkspace", {}, function(err)
      if err then
        log.error("fix workspace failed: %s", err.message or tostring(err))
      else
        log.info("workspace fixed")
      end
    end)
  end, { desc = "Fix all diagnostics in workspace" })

  vim.api.nvim_create_user_command("BasiliskAdoptFile", function()
    local uri = vim.uri_from_bufnr(vim.api.nvim_get_current_buf())
    execute_command("basilisk.adoptFile", { uri }, function(err)
      if err then
        log.error("adopt file failed: %s", err.message or tostring(err))
      else
        log.info("file adopted for type checking")
      end
    end)
  end, { desc = "Opt-in current file to type checking" })

  vim.api.nvim_create_user_command("BasiliskAdoptWorkspace", function()
    execute_command("basilisk.adoptWorkspace", {}, function(err)
      if err then
        log.error("adopt workspace failed: %s", err.message or tostring(err))
      else
        log.info("workspace adopted for type checking")
      end
    end)
  end, { desc = "Opt-in workspace to type checking" })

  vim.api.nvim_create_user_command("BasiliskUnadoptFile", function()
    local uri = vim.uri_from_bufnr(vim.api.nvim_get_current_buf())
    execute_command("basilisk.unadoptFile", { uri }, function(err)
      if err then
        log.error("unadopt file failed: %s", err.message or tostring(err))
      else
        log.info("file unadopted from type checking")
      end
    end)
  end, { desc = "Opt-out current file from type checking" })

  vim.api.nvim_create_user_command("BasiliskDisableRule", function(opts)
    local rule = opts.args
    if rule == "" then
      vim.ui.input({ prompt = "Diagnostic code to disable (e.g. BSK-E0001): " }, function(input)
        if input and input ~= "" then
          execute_command("basilisk.disableRule", { { rule = input, severity = "off" } })
        end
      end)
    else
      execute_command("basilisk.disableRule", { { rule = rule, severity = "off" } })
    end
  end, { nargs = "?", desc = "Disable a diagnostic rule in pyproject.toml" })

  vim.api.nvim_create_user_command("BasiliskShowOutput", function()
    -- In Neovim, open the LSP log file.
    local logpath = vim.lsp.get_log_path()
    if logpath then
      vim.cmd("edit " .. vim.fn.fnameescape(logpath))
    else
      log.info("no LSP log file found")
    end
  end, { desc = "Show LSP output log" })

  -- Refactoring commands.

  vim.api.nvim_create_user_command("BasiliskExtractVariable", function()
    vim.lsp.buf.code_action({
      filter = function(action)
        return action.kind and action.kind:find("refactor.extract.variable") ~= nil
      end,
      apply = true,
    })
  end, { desc = "Extract selection to a variable" })

  vim.api.nvim_create_user_command("BasiliskExtractConstant", function()
    vim.lsp.buf.code_action({
      filter = function(action)
        return action.kind and action.kind:find("refactor.extract.constant") ~= nil
      end,
      apply = true,
    })
  end, { desc = "Extract selection to a module-level constant" })

  vim.api.nvim_create_user_command("BasiliskConvertUnion", function()
    vim.lsp.buf.code_action({
      filter = function(action)
        return action.kind and action.kind:find("refactor.rewrite") ~= nil
          and (action.title:find("Union") ~= nil or action.title:find("Optional") ~= nil)
      end,
      apply = false,
    })
  end, { desc = "Convert between Union/Optional syntax styles" })

  vim.api.nvim_create_user_command("BasiliskImplementMethods", function()
    vim.lsp.buf.code_action({
      filter = function(action)
        return action.kind and action.kind:find("refactor.rewrite.implement") ~= nil
      end,
      apply = true,
    })
  end, { desc = "Implement all abstract methods" })

  -- Profiling commands.

  vim.api.nvim_create_user_command("BasiliskProfile", function(opts)
    local pid = opts.args ~= "" and tonumber(opts.args) or nil
    profiling.start(pid)
  end, { nargs = "?", desc = "Start profiling" })

  vim.api.nvim_create_user_command("BasiliskProfileStop", function()
    profiling.stop()
  end, { desc = "Stop profiling and show results" })

  vim.api.nvim_create_user_command("BasiliskProfileSnapshot", function()
    profiling.snapshot()
  end, { desc = "Take profiling snapshot" })

  -- Memory commands.

  vim.api.nvim_create_user_command("BasiliskMemLeak", function()
    memory.start()
  end, { desc = "Start memory leak tracking" })

  vim.api.nvim_create_user_command("BasiliskMemStop", function()
    memory.stop()
  end, { desc = "Stop memory tracking and show report" })

  vim.api.nvim_create_user_command("BasiliskMemRefs", function(opts)
    memory.refs(opts.args)
  end, {
    nargs = 1,
    desc = "Show memory references for a type",
    complete = function(lead)
      return memory.complete_refs(lead)
    end,
  })

  -- Debug commands.

  vim.api.nvim_create_user_command("BasiliskDebugFile", function()
    local dap_ok, dap = pcall(require, "dap")
    if not dap_ok then
      log.error("nvim-dap required for debugging")
      return
    end
    if not dap.adapters or not dap.adapters.basilisk then
      log.warn("basilisk DAP adapter not configured")
      return
    end
    local program = vim.api.nvim_buf_get_name(0)
    if program == "" or vim.fn.filereadable(program) ~= 1 then
      log.warn("no readable file to debug")
      return
    end
    dap.run({
      type = "basilisk",
      request = "launch",
      name = "Debug: Current File",
      program = program,
      justMyCode = true,
      cwd = vim.fn.getcwd(),
    })
  end, { desc = "Start debugging current file" })

  -- Test commands.

  vim.api.nvim_create_user_command("BasiliskTestDiscover", function()
    testing.discover(config)
  end, { desc = "Discover tests" })

  vim.api.nvim_create_user_command("BasiliskTestRun", function(opts)
    local test_id = opts.args ~= "" and opts.args or nil
    testing.run(config, test_id)
  end, { nargs = "?", desc = "Run test(s)" })

  vim.api.nvim_create_user_command("BasiliskTestDebug", function(opts)
    if opts.args == "" then
      log.warn("test ID required for debug")
      return
    end
    testing.debug(config, opts.args)
  end, { nargs = 1, desc = "Debug a test" })

  vim.api.nvim_create_user_command("BasiliskTestToggle", function()
    testing.toggle(config)
  end, { desc = "Toggle test explorer panel" })

  -- Activity panel commands.

  local modules_panel = require("basilisk.modules")
  local type_health_panel = require("basilisk.type_health")

  vim.api.nvim_create_user_command("BasiliskModules", function()
    modules_panel.toggle()
  end, { desc = "Toggle module explorer panel" })

  vim.api.nvim_create_user_command("BasiliskHealth", function()
    type_health_panel.toggle()
  end, { desc = "Toggle type health panel" })

  -- uv commands.

  vim.api.nvim_create_user_command("BasiliskUvSync", function()
    execute_command("basilisk.uv.sync", {}, function(err)
      if err then
        log.error("uv sync failed: %s", err.message or tostring(err))
      else
        log.info("uv sync complete")
      end
    end)
  end, { desc = "Run uv sync" })

  vim.api.nvim_create_user_command("BasiliskUvAdd", function(opts)
    execute_command("basilisk.uv.add", { { package = opts.args } }, function(err)
      if err then
        log.error("uv add failed: %s", err.message or tostring(err))
      else
        log.info("added package: %s", opts.args)
      end
    end)
  end, { nargs = 1, desc = "Add a package via uv" })

  vim.api.nvim_create_user_command("BasiliskUvAddDev", function(opts)
    execute_command("basilisk.uv.addDev", { { package = opts.args } }, function(err)
      if err then
        log.error("uv add --dev failed: %s", err.message or tostring(err))
      else
        log.info("added dev package: %s", opts.args)
      end
    end)
  end, { nargs = 1, desc = "Add a dev package via uv" })

  vim.api.nvim_create_user_command("BasiliskUvRemove", function(opts)
    execute_command("basilisk.uv.remove", { { package = opts.args } }, function(err)
      if err then
        log.error("uv remove failed: %s", err.message or tostring(err))
      else
        log.info("removed package: %s", opts.args)
      end
    end)
  end, { nargs = 1, desc = "Remove a package via uv" })

  vim.api.nvim_create_user_command("BasiliskUvLock", function()
    execute_command("basilisk.uv.lock", {}, function(err)
      if err then
        log.error("uv lock failed: %s", err.message or tostring(err))
      else
        log.info("uv lock complete")
      end
    end)
  end, { desc = "Run uv lock" })

  vim.api.nvim_create_user_command("BasiliskUvCreateEnv", function(opts)
    local args = {}
    if opts.args ~= "" then
      args = { { pythonVersion = opts.args } }
    end
    execute_command("basilisk.uv.createEnv", args, function(err)
      if err then
        log.error("uv venv failed: %s", err.message or tostring(err))
      else
        log.info("virtual environment created")
      end
    end)
  end, { nargs = "?", desc = "Create virtual environment via uv" })

  -- Set up test auto-discover.
  testing.setup_auto_discover(config)
end

return M
