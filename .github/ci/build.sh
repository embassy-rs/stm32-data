#!/bin/bash
## on push branch~=gh-readonly-queue/main/.*
## on pull_request

set -euxo pipefail

# prevent "fatal: gc is already running" errors
git config --global gc.autoDetach false

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

attempt=0
while [ -z "$(git merge-base HEAD upstream/main)" ] && [ $attempt -lt 10 ]; do
  git fetch --deepen=100 upstream main
  attempt=$((attempt + 1))
done

git merge upstream/main
MERGE_BASE=$(git merge-base HEAD upstream/main)

set +e
git clone --depth 1 --branch stm32-data-$MERGE_BASE https://github.com/embassy-rs/stm32-data-generated/ build -q
DIFF_OK=$?
set -e

# in the event that the CI has not yet generated this, we need to build it ourselves
if [ $DIFF_OK -ne 0 ]; then
    echo "rebuilding merge base"
    # rm build dir
    rm -rf build
    # save the current commit
    COMMIT=$(git rev-parse HEAD)
    # checkout the merge base
    git checkout $MERGE_BASE
    # build
    ./d ci
    # add build dir to git
    cd build
    rm -rf .git
    git init -q
    git add .
    git commit -m 'generated'
    cd ..
    # checkout saved commit
    git checkout $COMMIT
fi

# move the sources directory out of the cache if it exists
mv /ci/cache/sources ./sources || true

./d ci
./d check

# move the sources directory into the cache
mv ./sources /ci/cache/sources

# upload diff
(
    cd build
    git add .
    git diff --staged --color data | aha --black > /ci/artifacts/diff.html
)

cat > /ci/comment.md <<EOF
diff: https://ci.embassy.dev/jobs/$(jq -r .id < /ci/job.json)/artifacts/diff.html
git: https://ci.embassy.dev/jobs/$(jq -r .id < /ci/job.json)/artifacts/generated.git
EOF

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