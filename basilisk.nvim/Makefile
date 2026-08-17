.PHONY: test help

NVIM ?= nvim

## Run the plugin specs
test:
	$(NVIM) --headless -u tests/minimal_init.lua \
		-c "PlenaryBustedDirectory tests/basilisk {minimal_init = 'tests/minimal_init.lua'}"

## Show help
help:
	@echo "Available targets:"
	@echo "  test  Run the plugin specs"
