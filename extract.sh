#!/bin/bash

echo "Usage: ./extract.sh all|<board_name> <peripheral>"

board=$1
peri=$2
mkdir -p regs/$peri

cargo build --release --manifest-path ../../../svd2rust/Cargo.toml 

transform="transform.yaml"

if [ -f "transform-$peri.yaml" ]; 
then
    transform="transform-$peri.yaml"
fi

query="sources/svd";

if [ $board != "all" ];
then
    query="sources/svd/stm32$board*.svd"
fi


for f in `ls $query`; do
    f=${f##*/}
    f=${f#"stm32"}
    f=${f%".svd"}
    echo -n processing $f ...
    RUST_LOG=info ../../../svd2rust/target/release/svd4rust extract-peripheral --svd sources/svd/stm32$f.svd --transform $transform --peripheral $peri > regs/$peri/$f.yaml 2> regs/$peri/$f.yaml.out
    if [ $? -ne 0 ]; then 
        mv regs/$peri/$f.yaml.out regs/$peri/$f.err
        rm regs/$peri/$f.yaml
        echo FAIL
    else
        rm regs/$peri/$f.yaml.out
        echo OK
    fi
done
