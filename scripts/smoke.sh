#!/usr/bin/env bash
# Scripted check: the example server binary starts and answers an MCP
# initialize / tools/list round-trip over stateless Streamable HTTP.
set -euo pipefail

BIN=./target/debug/examples/minimal_server
PORT="${EMPEROR_PORT:-8017}"
URL="http://127.0.0.1:${PORT}/"

[ -x "$BIN" ] || { echo "example binary missing — run: cargo build --examples"; exit 1; }

"$BIN" &
PID=$!
trap 'kill "$PID" 2>/dev/null || true' EXIT

for _ in $(seq 1 50); do
  curl -s -o /dev/null -X POST "$URL" -d '{}' && break
  sleep 0.1
done

init=$(curl -sf -X POST "$URL" \
  -H 'Content-Type: application/json' -H 'Accept: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}')
echo "$init" | grep -q '"protocolVersion"' || { echo "initialize failed: $init"; exit 1; }

tools=$(curl -sf -X POST "$URL" \
  -H 'Content-Type: application/json' -H 'Accept: application/json' \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/list"}')
echo "$tools" | grep -q '"echo"' || { echo "tools/list failed: $tools"; exit 1; }

echo "smoke OK: initialize + tools/list answered on ${URL}"
