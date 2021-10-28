#!/usr/bin/env bash

set -e
cd $(dirname $0)

die() { echo "$*" 1>&2; exit 1; }

for i in jq wget svd git; do
    command -v "$i" &>/dev/null || die "Missing the command line tool '$i'"
done

rm -rf ./sources/
git clone https://github.com/embassy-rs/stm32-data-sources.git ./sources/

