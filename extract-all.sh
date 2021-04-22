#!/bin/bash

peri=$1
mkdir -p tmp/$peri

cargo build --release --manifest-path ../../svd2rust/Cargo.toml

for f in `ls sources/svd`; do
    f=${f#"stm32"}
    f=${f%".svd"}
    echo -n processing $f ...
    RUST_LOG=info ../../svd2rust/target/release/svd4rust extract-peripheral --svd sources/svd/stm32$f.svd --transform transform.yaml --peripheral $peri > tmp/$peri/$f.yaml 2> tmp/$peri/$f.yaml.out
    if [ $? -ne 0 ]; then 
        rm tmp/$peri/$f.yaml
        echo FAIL
    else
        rm tmp/$peri/$f.yaml.out
        echo OK
    fi
done
