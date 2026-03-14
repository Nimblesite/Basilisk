# Dev container matching CI (GitHub Actions ubuntu-latest + dtolnay/rust-toolchain@stable)
# Pin Rust to match CI's stable channel — this is the critical version.
FROM rust:1.94.0-bookworm

# System deps (pinned to Debian bookworm repository versions)
RUN apt-get update && apt-get install -y --no-install-recommends \
    software-properties-common \
    xvfb \
    curl \
    ca-certificates \
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

# Python packages (pinned — matches CI: pip install debugpy ruff)
RUN pip install --break-system-packages debugpy==1.8.14 ruff==0.11.2

# Match CI environment variables
ENV CARGO_TERM_COLOR=always
ENV RUSTFLAGS="-D warnings"

WORKDIR /workspace
