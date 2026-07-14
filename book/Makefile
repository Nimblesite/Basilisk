PYTHON ?= python3

.PHONY: check check-links render-assets screenshots epub release clean

check:
	$(PYTHON) scripts/check_book.py
	$(PYTHON) scripts/check_links.py

check-links:
	$(PYTHON) scripts/check_links.py --external

render-assets:
	$(PYTHON) scripts/render_assets.py

screenshots:
	$(PYTHON) scripts/capture_editor_screenshots.py

epub:
	$(PYTHON) scripts/build.py

release:
	$(PYTHON) scripts/build.py --release

clean:
	rm -rf dist/*
