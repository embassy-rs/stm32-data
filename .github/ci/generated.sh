#!/bin/bash
## on push branch=main
## permission contents write
## permission_repo stm32-data-generated

set -euxo pipefail

export RUSTUP_HOME=/ci/cache/rustup
export CARGO_HOME=/ci/cache/cargo
export CARGO_TARGET_DIR=/ci/cache/target

hashtime restore /ci/cache/filetime.json || true
hashtime save /ci/cache/filetime.json

git clone --depth 1 https://github.com/embassy-rs/stm32-data-generated/ build
./d ci

COMMIT=$(git rev-parse HEAD)
cd build
git add data stm32-metapac
git commit -m "Generated from stm32-data $COMMIT" --allow-empty
git tag -a stm32-data-$COMMIT -m "Generated from stm32-data $COMMIT"
git push --follow-tags
