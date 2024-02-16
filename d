#!/usr/bin/env bash

set -e
cd $(dirname $0)

CMD=$1
REV=796a8322b8f6e4329002a1fb49438aec95dae812
shift

case "$CMD" in
    download-all)
        rm -rf ./sources/
        git clone https://github.com/embassy-rs/stm32-data-sources.git ./sources/ -q
        cd ./sources/
        git checkout $REV
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
                if grep -q 'peripheral not found' tmp/$peri/$f.err; then
                    echo No Peripheral
                else
                    echo OTHER FAILURE
                fi
                rm tmp/$peri/$f.yaml
            fi
        done
    ;;
    gen)
        rm -rf build/data
        cargo run --release --bin stm32-data-gen
    ;;
    ci)
        [ -d sources ] || ./d download-all
        cd ./sources/
        git fetch origin $REV
        git checkout $REV
        cd ..
        rm -rf build/{data,stm32-metapac}
        cargo run --release --bin stm32-data-gen
        cargo run --release --bin stm32-metapac-gen
        cd build/stm32-metapac
        find . -name '*.rs' -not -path '*target*' | xargs rustfmt --skip-children --unstable-features --edition 2021
        cargo check --features stm32h755zi-cm7,pac,metadata
        cargo check --features stm32f777zi,pac
        cargo check --features stm32u585zi,metadata
    ;;
    *)
        echo "unknown command"
    ;;
esac

