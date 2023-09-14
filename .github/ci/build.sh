#!/bin/bash
## on push branch~=gh-readonly-queue/main/.*
## on pull_request

set -euo pipefail

export RUSTUP_HOME=/ci/cache/rustup
export CARGO_HOME=/ci/cache/cargo
export CARGO_TARGET_DIR=/ci/cache/target

hashtime restore /ci/cache/filetime.json || true
hashtime save /ci/cache/filetime.json

cargo fmt -- --check

# clone stm32-data-generated at the merge base
# so the diff will show this PR's effect
git remote add upstream https://github.com/embassy-rs/stm32-data
git fetch --depth 1 upstream main
git clone --depth 1 --branch stm32-data-$(git merge-base HEAD upstream/main) https://github.com/embassy-rs/stm32-data-generated/ build

./d ci

# upload diff
(
    cd build
    git diff --color data | aha --black > /ci/artifacts/diff.html
)

# upload generated data to a fake git repo at
# https://ci.embassy.dev/jobs/$ID/artifacts/generated.git
# this allows testing the corresponding embassy-stm32 PR before merging the stm32-data one.
(
    cd build
    rm -rf .git
    git init
    git add .
    git commit -m 'generated'
    git gc  # makes cloning faster
    git update-server-info  # generate .git/info/refs
    mv .git /ci/artifacts/generated.git
)

cat > /ci/comment.md <<EOF
diff: https://ci.embassy.dev/jobs/$(jq -r .id < /ci/job.json)/artifacts/diff.html
EOF
