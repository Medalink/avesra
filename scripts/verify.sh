#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
case "${1:-static}" in
 static) cargo fmt --all -- --check; cargo clippy -p avesra-contracts -p avesra-core -p avesra-server --locked -- -D warnings ;;
 build) cargo build -p avesra-contracts -p avesra-core -p avesra-server --release --locked ;;
 *) echo 'Supported checks: static, build. Automated tests are excluded by owner instruction.' >&2; exit 2 ;;
esac
echo 'Core/server check completed. Audio serving, desktop, automated tests and live acceptance are not included.'
