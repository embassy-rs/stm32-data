#!/bin/bash
## on push branch~=gh-readonly-queue/main/.*
## on pull_request

set -euxo pipefail

export RUSTUP_HOME=/ci/cache/rustup
export CARGO_HOME=/ci/cache/cargo
export CARGO_TARGET_DIR=/ci/cache/target

hashtime restore /ci/cache/filetime.json || true
hashtime save /ci/cache/filetime.json

cargo fmt -- --check

# clone stm32-data-generated at the merge base
# so the diff will show this PR's effect
git remote add upstream https://github.com/embassy-rs/stm32-data
git fetch --depth 15 upstream main
git clone --depth 1 --branch stm32-data-$(git merge-base HEAD upstream/main) https://github.com/embassy-rs/stm32-data-generated/ build -q

# move the sources directory out of the cache if it exists
mv /ci/cache/sources ./sources || true

./d ci

# move the sources directory into the cache
mv ./sources /ci/cache/sources

# upload diff
(
    cd build
    git add .
    git diff --staged --color data | aha --black > /ci/artifacts/diff.html
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
