local helpers = require("tests.lsp.helpers")

describe("LSP test helpers", function()
  it("poll_until charges condition execution time against its deadline", function()
    local started = vim.uv.hrtime()

    local result = helpers.poll_until(function()
      -- Simulate a request predicate that itself blocks. The old elapsed-time
      -- counter ignored this work and then slept for another full interval,
      -- allowing a nominal 30 ms deadline to take 140+ ms (and real LSP waits
      -- to overrun the per-spec timeout by minutes).
      vim.wait(40)
      return false
    end, 30, "blocking predicate")

    local elapsed_ms = (vim.uv.hrtime() - started) / 1e6
    assert.is_false(result)
    assert.is_true(elapsed_ms < 100, ("deadline overran: %.1f ms"):format(elapsed_ms))
  end)
end)
