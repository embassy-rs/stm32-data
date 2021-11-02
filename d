#!/usr/bin/env bash

set -e
cd $(dirname $0)

die() { echo "$*" 1>&2; exit 1; }

for i in jq wget svd git; do
    command -v "$i" &>/dev/null || die "Missing the command line tool '$i'"
done

CMD=$1
shift

case "$CMD" in
    download-all)
        rm -rf ./sources/
        git clone https://github.com/embassy-rs/stm32-data-sources.git ./sources/
    ;;
    install-chiptool)
        cargo install --git https://github.com/embassy-rs/chiptool
    ;;
    extract-all)
        peri=$1
        shift
        echo $@

        rm -rf tmp/$peri
        mkdir -p tmp/$peri

        for f in `ls sources/svd`; do
            f=${f#"stm32"}
            f=${f%".svd"}
            echo -n processing $f ...
            if chiptool extract-peripheral --svd sources/svd/stm32$f.svd --peripheral $peri $@ > tmp/$peri/$f.yaml 2> tmp/$peri/$f.err; then
                rm tmp/$peri/$f.err
                echo OK
            else
                rm tmp/$peri/$f.yaml
                echo FAIL
            fi
        done
    ;;
    *)
        echo "unknown command"
    ;;
esac

