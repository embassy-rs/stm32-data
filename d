#!/usr/bin/env bash

set -e
cd $(dirname $0)

CMD=$1
REV=823d4d95a20a62ce71f9a944c1f7374185a10fa1
shift

case "$CMD" in
    download-all)
        rm -rf ./sources/
        git clone https://github.com/embassy-rs/stm32-data-sources.git ./sources/ -q --depth 1
        cd ./sources/
        git fetch origin $REV
        git checkout $REV
    ;;
    update-all)
        [ -d sources ] || ./d download-all
        cd ./sources/
        git fetch origin $REV
        git checkout $REV
        cd ..
    ;;
    install-chiptool)
        cargo install --git https://github.com/embassy-rs/chiptool
    ;;
    extract-all)
        peri=$1
        shift
        echo $@

        cargo run --release --bin extract-all $peri
    ;;
    merge-regs)
        peri=$1
        shift
        echo $@

        cargo run --release --bin merge-regs tmp/$peri
    ;;
    transform)
        peri=$1
        shift
        echo $@

        chiptool transform --input regs_merged.yaml --output regs_merged.yaml --transform transforms/$peri.yaml
    ;;
    gen)
        peri=$1
        logLevel=$2
        shift
        echo $@

        case "$#" in
            2)
                cargo run --release --bin stm32-data-gen -- --filter "$peri" --log-level "$logLevel"
                ;;
            1)
                cargo run --release --bin stm32-data-gen -- --filter "$peri"
                ;;
            *)
                cargo run --release --bin stm32-data-gen
                ;;
        esac
    ;;
    gen-all)
        cargo run --release --bin stm32-data-gen
        cargo run --release --bin stm32-metapac-gen
        cd build/stm32-metapac
        find . -name '*.rs' -not -path '*target*' | xargs rustfmt --skip-children --unstable-features --edition 2021
    ;;
    ci)
        ./d update-all
        ./d gen-all
    ;;
    check)
        cargo batch \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32c031c6 \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f030c6 \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f030r8 \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f030rc \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f031k6 \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f038f6 \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f042g4 \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f058t8 \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f070f6 \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f072c8 \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f078vb \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f091rc \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f100c4 \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f103c8 \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f103re \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f107vc \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f207zg \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f217zg \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f303c8 \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f303ze \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f378cc \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f398ve \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f401ve \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f405zg \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f407zg \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f410tb \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f411ce \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f412zg \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f413vh \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f415zg \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f417zg \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f423zh \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f427zi \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f429zi \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f437zi \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f439zi \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f446re \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f446ze \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f469zi \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f479zi \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f730i8 \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f767zi \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32g071rb \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32g0c1ve \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32g474pe \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32g491re \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32h503rb \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32h523cc \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32h562ag \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32h563zi \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32h725re \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32h735zg \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32h753zi \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32h755zi-cm7 \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32h755zi-cm4 \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32h7a3zi \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32h7b3ai \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32h7r3z8 \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32h7r7a8 \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32h7s3a8 \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32h7s3l8 \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32h7s7z8 \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32l041f6 \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32l051k8 \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32l072cz \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32l073cz \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32l073rz \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32l151cb \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32l152re \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32l422cb \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32l431cb \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32l476vg \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32l496zg \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32l4a6zg \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32l4r5zi \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32l552ze \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv8m.main-none-eabihf --features pac,metadata,stm32n657x0 \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv8m.main-none-eabihf --features pac,metadata,stm32n647x0 \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv8m.main-none-eabihf --features pac,metadata,stm32n655x0 \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv8m.main-none-eabihf --features pac,metadata,stm32n645x0 \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32u031r8 \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32u073mb \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32u083rc \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32u585ai \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32u5a5zj \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32u5f9zj \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32u5g9nj \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32wb15cc \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32wb35ce \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32wb55rg \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv8m.main-none-eabihf --features pac,metadata,stm32wba50ke \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv8m.main-none-eabihf --features pac,metadata,stm32wba52cg \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv8m.main-none-eabihf --features pac,metadata,stm32wba55ug \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv8m.main-none-eabihf --features pac,metadata,stm32wba62mg \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv8m.main-none-eabihf --features pac,metadata,stm32wba63cg \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv8m.main-none-eabihf --features pac,metadata,stm32wba64ci \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv8m.main-none-eabihf --features pac,metadata,stm32wba65ri \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32wl54jc-cm4 \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32wl54jc-cm0p \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32wl55jc-cm4 \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32wl55jc-cm0p \
            --- build --release --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32wle5jb \

    ;;
    *)
        echo "unknown command"
    ;;
esac
