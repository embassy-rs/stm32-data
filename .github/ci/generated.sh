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

git clone --depth 1 https://github.com/embassy-rs/stm32-data-generated/ build -q
./d ci

# Generate peripheral summary and update README.md
cat > build/README.md << 'EOF'
# stm32-data generated output

This repo contains generated output for [`stm32-data`](https://github.com/embassy-rs/stm32-data). It is updated for every push to the `main` branch. See the `stm32-data` README for more details.

## STM32 Peripheral Support Matrix

The following table shows which STM32 peripheral versions are supported across different families:

EOF
cargo run --release --bin summary >> build/README.md

COMMIT=$(git rev-parse HEAD)
cd build
git add data stm32-metapac README.md
git commit -m "Generated from stm32-data $COMMIT" --allow-empty
git tag -a stm32-data-$COMMIT -m "Generated from stm32-data $COMMIT"
git push --follow-tags
