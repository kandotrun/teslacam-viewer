# TeslaCam Viewer monorepo tasks.
#
#   apps/web  React + Vite web app, deployed to Cloudflare Workers
#   apps/mac  macOS-native app (Rust + GPUI)

set shell := ["bash", "-euo", "pipefail", "-c"]

# List available recipes
default:
    @just --list --unsorted

# ---------------------------------------------------------------------------
# Web (apps/web)
# ---------------------------------------------------------------------------

# Install web dependencies from the lockfile
[group('web')]
[working-directory('apps/web')]
web-install:
    npm ci

# Start the Vite dev server
[group('web')]
[working-directory('apps/web')]
web-dev:
    npm run dev

# Typecheck, lint and run unit tests once
[group('web')]
[working-directory('apps/web')]
web-check:
    npm run typecheck
    npm run lint
    npm test -- --run

# Build the production bundle into apps/web/dist
[group('web')]
[working-directory('apps/web')]
web-build:
    npm run build

# Run Playwright end-to-end tests (builds and serves the app itself)
[group('web')]
[working-directory('apps/web')]
web-e2e:
    npm run test:e2e

# Validate the Cloudflare Workers deployment without uploading
[group('web')]
[working-directory('apps/web')]
web-deploy-dry-run: web-build
    npx wrangler deploy --dry-run

# ---------------------------------------------------------------------------
# Mac (apps/mac)
# ---------------------------------------------------------------------------

# Run the Mac domain, media and app tests
[group('mac')]
[working-directory('apps/mac')]
mac-test:
    cargo test --workspace

# Lint the Mac workspace
[group('mac')]
[working-directory('apps/mac')]
mac-check:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings

# Run the Mac app in release mode
[group('mac')]
[working-directory('apps/mac')]
mac-run:
    cargo run --release -p teslacam-app

# Generate TeslaCam-like media fixtures for local playback tests
[group('mac')]
[working-directory('apps/mac')]
mac-fixtures:
    ./scripts/gen-fixtures.sh .fixtures

# ---------------------------------------------------------------------------
# Shared contract (packages/contract)
# ---------------------------------------------------------------------------

# Regenerate the golden fixtures from the web implementation
[group('contract')]
[working-directory('apps/web')]
contract-generate:
    npm run contract:generate
