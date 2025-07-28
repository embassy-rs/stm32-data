#!/bin/bash
## on push branch=main
## priority -100
## dedup dequeue
## cooldown 15m

set -euo pipefail

export RUSTUP_HOME=/ci/cache/rustup
export CARGO_HOME=/ci/cache/cargo
export CARGO_TARGET_DIR=/ci/cache/target
export BUILDER_THREADS=6
export BUILDER_COMPRESS=true

hashtime restore /ci/cache/filetime.json || true
hashtime save /ci/cache/filetime.json

./d ci

docserver-builder -i ./build/stm32-metapac -o crates/stm32-metapac/git.zup

export KUBECONFIG=/ci/secrets/kubeconfig.yml
POD=$(kubectl -n embassy get po -l app=docserver -o jsonpath={.items[0].metadata.name})
kubectl cp crates $POD:/data
