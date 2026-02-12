#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

"${SCRIPT_DIR}/starbott-dev.sh" whoami
echo
"${SCRIPT_DIR}/starbott-dev.sh" chat "hi"

