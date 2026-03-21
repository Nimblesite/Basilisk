.PHONY: test test-ui test-lsp test-dap test-screenshots test-all lint help

NVIM ?= nvim

## Run all plenary tests
test:
	$(NVIM) --headless -u tests/minimal_init.lua \
		-c "PlenaryBustedDirectory tests/basilisk {minimal_init = 'tests/minimal_init.lua'}"

## Run UI tests
test-ui:
	$(NVIM) --headless -u tests/minimal_init.lua \
		-c "PlenaryBustedDirectory tests/ui {minimal_init = 'tests/minimal_init.lua'}"

## Run real LSP integration tests (requires basilisk binary)
test-lsp:
	$(NVIM) --headless -u tests/minimal_init.lua \
		-c "PlenaryBustedDirectory tests/lsp {minimal_init = 'tests/minimal_init.lua'}"

## Run DAP debug integration tests (requires basilisk binary + debugpy)
test-dap:
	$(NVIM) --headless -u tests/minimal_init.lua \
		-c "PlenaryBustedDirectory tests/dap {minimal_init = 'tests/minimal_init.lua'}"

## Run screenshot regression tests (requires mini.nvim + basilisk binary)
test-screenshots:
	$(NVIM) --headless -u tests/minimal_init.lua \
		-l tests/ui/run_screenshots.lua

## Run all tests
test-all: test test-ui test-lsp test-dap test-screenshots

## Show help
help:
	@echo "Available targets:"
	@echo "  test             Run core plenary tests"
	@echo "  test-ui          Run UI tests"
	@echo "  test-lsp         Run real LSP integration tests (rename, refactoring)"
	@echo "  test-dap         Run DAP debug integration tests (breakpoints, stepping)"
	@echo "  test-screenshots Run screenshot regression tests (mini.test)"
	@echo "  test-all         Run all tests"
