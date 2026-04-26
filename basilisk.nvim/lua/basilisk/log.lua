--- Logger for basilisk.nvim.
---
--- Wraps vim.notify with configurable log levels and optional file logging.

local M = {}

local default_notify = vim.notify

--- Log level names to vim.log.levels mapping.
---@type table<string, integer>
local LEVELS = {
  trace = vim.log.levels.TRACE,
  debug = vim.log.levels.DEBUG,
  info = vim.log.levels.INFO,
  warn = vim.log.levels.WARN,
  error = vim.log.levels.ERROR,
}

--- Current minimum log level.
---@type integer
local min_level = vim.log.levels.INFO

--- Optional file handle for file logging.
---@type file*?
local log_file = nil

--- Resolve the notification level for the current Neovim context.
---@param level integer vim.log.levels.*
---@return integer
local function notify_level(level)
  if
    level == vim.log.levels.ERROR
    and vim.notify == default_notify
    and #vim.api.nvim_list_uis() == 0
  then
    return vim.log.levels.WARN
  end
  return level
end

--- Set the minimum log level.
---@param level string One of "trace", "debug", "info", "warn", "error".
function M.set_level(level)
  local resolved = LEVELS[level]
  if resolved then
    min_level = resolved
  end
end

--- Enable file logging to the given path.
---@param path string
function M.enable_file(path)
  if log_file then
    log_file:close()
  end
  log_file = io.open(path, "a")
end

--- Close the log file if open.
function M.close_file()
  if log_file then
    log_file:close()
    log_file = nil
  end
end

--- Log a message at the given level.
---@param level integer vim.log.levels.*
---@param fmt string Format string.
---@param ... any Format arguments.
local function log(level, fmt, ...)
  if level < min_level then
    return
  end
  local msg = string.format(fmt, ...)
  vim.notify("[basilisk] " .. msg, notify_level(level))
  if log_file then
    log_file:write(string.format("%s [%s] %s\n", os.date("%Y-%m-%d %H:%M:%S"), level, msg))
    log_file:flush()
  end
end

function M.trace(fmt, ...)
  log(vim.log.levels.TRACE, fmt, ...)
end

function M.debug(fmt, ...)
  log(vim.log.levels.DEBUG, fmt, ...)
end

function M.info(fmt, ...)
  log(vim.log.levels.INFO, fmt, ...)
end

function M.warn(fmt, ...)
  log(vim.log.levels.WARN, fmt, ...)
end

function M.error(fmt, ...)
  log(vim.log.levels.ERROR, fmt, ...)
end

return M
