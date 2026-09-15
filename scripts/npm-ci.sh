#!/usr/bin/env bash
# npm postinstall downloads can fail transiently outside npm's own fetch retries.
set -euo pipefail
for attempt in 1 2 3; do
  # Den uses browser WASM/WebGPU, never Node's Linux CUDA providers. Keep every
  # package's install scripts enabled; only opt out of ONNX's extra download.
  if ONNXRUNTIME_NODE_INSTALL=skip npm ci "$@"; then
    exit 0
  else
    status=$?
  fi
  if [[ $attempt -eq 3 ]]; then
    echo "npm ci failed after 3 attempts (exit $status)" >&2
    exit "$status"
  fi
  delay=$((attempt * 5))
  echo "npm ci attempt $attempt failed (exit $status); retrying in ${delay}s" >&2
  sleep "$delay"
done
