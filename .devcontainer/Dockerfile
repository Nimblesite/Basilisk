# Dev container matching CI (GitHub Actions ubuntu-latest + dtolnay/rust-toolchain@stable)
# Pin Rust to match CI's stable channel — this is the critical version.
FROM rust:1.94.0-bookworm

# System deps (pinned to Debian bookworm repository versions)
# VS Code test runner requires: libnspr4, libnss3, libgbm1, libxkbfile1, libsecret-1-0,
# libgtk-3-0, libxss1, libxtst6, libasound2, libatk-bridge2.0-0 (Electron/Chromium stack)
RUN apt-get update && apt-get install -y --no-install-recommends \
    software-properties-common \
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

# Python 3.12 (matches CI: actions/setup-python@v5 with python-version: "3.12")
RUN apt-get update && apt-get install -y --no-install-recommends \
    python3 \
    python3-pip \
    python3-venv \
    && rm -rf /var/lib/apt/lists/*

# Node 20 (matches CI: actions/setup-node@v4 with node-version: 20)
RUN curl -fsSL https://deb.nodesource.com/setup_20.x | bash - \
    && apt-get install -y --no-install-recommends nodejs=20.* \
    && rm -rf /var/lib/apt/lists/*

# Rust components (matches CI: dtolnay/rust-toolchain@stable with components)
RUN rustup component add rustfmt clippy

# Rust cargo tools (pinned)
RUN cargo install cargo-llvm-cov --version 0.8.4 --locked

# Python packages (pinned — matches CI: pip install debugpy ruff)
RUN pip install --break-system-packages debugpy==1.8.14 ruff==0.11.2

# Match CI environment variables
ENV CARGO_TERM_COLOR=always
ENV RUSTFLAGS="-D warnings"

# Create a non-root user so Claude Code can use --dangerously-skip-permissions
RUN useradd -m -s /bin/bash -u 1000 dev \
    && mkdir -p /workspace \
    && chown -R dev:dev /workspace /usr/local/cargo /usr/local/rustup

USER dev

WORKDIR /workspace
