# Dev container matching CI (.github/workflows/ci.yml — every job runs on
# ubuntu-24.04, or vars.BIG_RUNNER, with dtolnay/rust-toolchain@stable).
# CI resolves `stable` at run time; this image pins the concrete release so a
# container rebuild is reproducible. 1.94 fails the workspace build
# (sysinfo 0.39.3 requires rustc >= 1.95).
FROM rust:1.96.0-bookworm

# System deps (pinned to Debian bookworm repository versions)
# VS Code test runner requires: libnspr4, libnss3, libgbm1, libxkbfile1, libsecret-1-0,
# libgtk-3-0, libxss1, libxtst6, libasound2, libatk-bridge2.0-0 (Electron/Chromium stack)
RUN apt-get update && apt-get install -y --no-install-recommends \
    software-properties-common \
    lld \
    xvfb \
    xauth \
    curl \
    ca-certificates \
    libnspr4 \
    libnss3 \
    libgbm1 \
    libxkbfile1 \
    libsecret-1-0 \
    libgtk-3-0 \
    libxss1 \
    libxtst6 \
    libasound2 \
    libatk1.0-0 \
    libatk-bridge2.0-0 \
    libgdk-pixbuf2.0-0 \
    libdbus-1-3 \
    && rm -rf /var/lib/apt/lists/*

# Python. CI pins 3.12 (actions/setup-python@v6 with python-version: "3.12");
# this base image is Debian bookworm, whose `python3` is 3.11. That is the one
# knowingly unsynced version here — the interpreters are NOT identical, so a
# green container run is not proof of the CI matrix.
RUN apt-get update && apt-get install -y --no-install-recommends \
    python3 \
    python3-pip \
    python3-venv \
    && rm -rf /var/lib/apt/lists/*

# Node 22 (matches CI: actions/setup-node@v6 with node-version 22.x in the
# `VS Code Extension` and `Shipwright Binary Gate` jobs — the ones this container
# reproduces through `make test` / `make build`. Only the website-build job,
# which this image does not run, pins Node 20.)
RUN curl -fsSL https://deb.nodesource.com/setup_22.x | bash - \
    && apt-get install -y --no-install-recommends nodejs=22.* \
    && rm -rf /var/lib/apt/lists/*

# Rust components + targets (matches CI: dtolnay/rust-toolchain@stable with
# `rustfmt, clippy` in Lint, `llvm-tools-preview` in Rust Tests & Coverage — the
# component cargo-llvm-cov needs — and the wasm32-wasip2 target in Zed Extension)
RUN rustup component add rustfmt clippy llvm-tools-preview \
    && rustup target add wasm32-wasip2

# Rust cargo tools (pinned)
RUN cargo install cargo-llvm-cov --version 0.8.4 --locked \
    && cargo install cargo-audit --version 0.22.2 --locked

# Python packages (pinned — ruff MUST match CI and the ruff_* crate tag in
# Cargo.toml). pytest drives the Neovim e2e harness (scripts/test-nvim.sh) and is
# pinned to CI's version; jinja2/markdown/tomlkit are the REAL python/typing
# conformance harness's own runtime deps, installed unpinned exactly as CI does.
RUN pip install --break-system-packages \
        debugpy==1.8.14 ruff==0.15.17 pytest==9.1.1 \
        jinja2 markdown tomlkit

# GitHub CLI (for PR creation, issue management)
RUN curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg \
    -o /usr/share/keyrings/githubcli-archive-keyring.gpg \
    && echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" \
    > /etc/apt/sources.list.d/github-cli.list \
    && apt-get update && apt-get install -y --no-install-recommends gh \
    && rm -rf /var/lib/apt/lists/*

# act (run GitHub Actions locally)
RUN curl -fsSL https://raw.githubusercontent.com/nektos/act/master/install.sh | bash -s -- -b /usr/local/bin

# Claude Code CLI
RUN npm install -g @anthropic-ai/claude-code

# Lua coverage for the Neovim suite. CI runs `luarocks install --local luacov`
# into the runner's HOME; installing it system-wide here puts it on the default
# LUA_PATH, so `make _test_nvim` finds it without an `eval "$(luarocks path)"`.
RUN apt-get update && apt-get install -y --no-install-recommends luarocks \
    && luarocks install luacov \
    && rm -rf /var/lib/apt/lists/*

# Match CI environment variables
ENV CARGO_TERM_COLOR=always
ENV RUSTFLAGS="-D warnings"

# Create a non-root user so Claude Code can use --dangerously-skip-permissions.
# It is created BEFORE the Homebrew bootstrap below because Homebrew refuses to
# install as root and its installer expects passwordless sudo.
RUN useradd -m -s /bin/bash -u 1000 dev \
    && mkdir -p /workspace \
    && chown -R dev:dev /workspace /usr/local/cargo /usr/local/rustup \
    && apt-get update \
    && apt-get install -y --no-install-recommends sudo git procps file build-essential \
    && echo 'dev ALL=(ALL) NOPASSWD:ALL' > /etc/sudoers.d/dev \
    && chmod 0440 /etc/sudoers.d/dev \
    && rm -rf /var/lib/apt/lists/*

USER dev

# Homebrew — the package manager scripts/install-deslop.sh drives, and the one
# the CI Lint job uses to install the deslop gate. That script relies on the
# runner already having brew; a Debian image has to bootstrap it.
RUN NONINTERACTIVE=1 /bin/bash -c \
    "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
ENV PATH="/home/linuxbrew/.linuxbrew/bin:/home/linuxbrew/.linuxbrew/sbin:${PATH}"

# Neovim — `make test` runs the basilisk.nvim e2e suite (`make _test_nvim`) and
# scripts/audit.sh gates on it. basilisk.nvim requires 0.11+ (its health.lua
# hard-errors below that, and lua/basilisk/lsp.lua uses the 0.11-only
# vim.lsp.config/vim.lsp.enable); CI covers that floor and nightly via
# rhysd/action-setup-vim. Debian bookworm's nvim predates 0.11, so it comes from
# brew here — audit.sh re-verifies the floor before the suite runs either way.
RUN brew install neovim

# deslop duplication gate — `make lint` runs it ([CI-DESLOP]). Installed with the
# repo's own script, byte-for-byte the step the CI Lint job runs, so the CLI here
# is the same latest release CI gates on.
COPY --chown=dev:dev scripts/install-deslop.sh /tmp/install-deslop.sh
RUN bash /tmp/install-deslop.sh && rm /tmp/install-deslop.sh

WORKDIR /workspace
