#!/usr/bin/env bash

set -e
cd $(dirname $0)

CMD=$1
shift

case "$CMD" in
    download-all)
        rm -rf ./sources/
        git clone https://github.com/embassy-rs/stm32-data-sources.git ./sources/
        # The following is a temporary workaround until https://github.com/embassy-rs/stm32-data/pull/175 is merged.
        cd ./sources/
        git checkout 3d60b46
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
    ci)
        [ -d sources ] || ./d download-all
        rm -rf build
        cargo run --release --bin stm32-data-gen
        cargo run --release --bin stm32-metapac-gen
        (cd build/stm32-metapac && cargo check --features stm32h755zi-cm7,pac,metadata)
    ;;
    *)
        echo "unknown command"
    ;;
esac

