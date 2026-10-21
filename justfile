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

# TODO(mac): add mac-* recipes here (build, test, run, bundle).
