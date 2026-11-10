PYTHON ?= python3

.PHONY: check check-links render-assets screenshots epub release clean

check:
	$(PYTHON) -m unittest scripts.test_book_contract
	$(PYTHON) scripts/check_book.py
	$(PYTHON) scripts/check_links.py

check-links:
	$(PYTHON) scripts/check_links.py --external

render-assets:
	$(PYTHON) scripts/render_assets.py

screenshots:
	$(PYTHON) scripts/capture_editor_screenshots.py
	$(PYTHON) scripts/capture_adoption_screenshots.py

epub:
	$(PYTHON) scripts/build.py

release:
	$(PYTHON) scripts/build.py --release

clean:
	rm -rf dist/*
