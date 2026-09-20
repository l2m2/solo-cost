#!/usr/bin/env bash

set -euo pipefail

TUNNEL_CLIENT="${TUNNEL_CLIENT:-$(command -v tunnel-client || true)}"
PROFILE="solo-cost"

if [[ -z "$TUNNEL_CLIENT" ]]; then
  TUNNEL_CLIENT="/Users/l2m2/.cache/solo-cost/tunnel-client-v0.0.14/tunnel-client"
fi

if [[ ! -x "$TUNNEL_CLIENT" ]]; then
  echo "找不到 tunnel-client：$TUNNEL_CLIENT" >&2
  exit 1
fi

if [[ -z "${CONTROL_PLANE_API_KEY:-}" ]]; then
  read -r -s -p "请输入 OpenAI Runtime API Key: " CONTROL_PLANE_API_KEY
  printf '\n'
  export CONTROL_PLANE_API_KEY
fi

"$TUNNEL_CLIENT" doctor --profile "$PROFILE" --explain
exec "$TUNNEL_CLIENT" run --profile "$PROFILE"
