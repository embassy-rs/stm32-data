#!/usr/bin/env bash

set -e
cd $(dirname $0)

CMD=$1
REV=152220bbbf8eccbced29252c56f901e8a0a169ac
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
        find . -name '*.rs' -not -path '*target*' | xargs rustfmt --skip-children --unstable-features --edition 2024
    ;;
    ci)
        ./d update-all
        ./d gen-all
    ;;
    check)
        cargo batch \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32c031c6 \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f030c6 \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f030r8 \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f030rc \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f031k6 \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f038f6 \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f042g4 \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f058t8 \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f070f6 \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f072c8 \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f078vb \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f091rc \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f100c4 \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f103c8 \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f103re \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f107vc \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f207zg \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f217zg \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f303c8 \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f303ze \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f378cc \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f398ve \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f401ve \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f405zg \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f407zg \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f410tb \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f411ce \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f412zg \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f413vh \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f415zg \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f417zg \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f423zh \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f427zi \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f429zi \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f437zi \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f439zi \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f446re \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f446ze \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f469zi \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f479zi \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f730i8 \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32f767zi \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32g071rb \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32g0c1ve \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32g474pe \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32g491re \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32h503rb \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32h523cc \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32h562ag \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32h563zi \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32h725re \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32h735zg \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32h753zi \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32h755zi-cm7 \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32h755zi-cm4 \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32h7a3zi \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32h7b3ai \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32h7r3z8 \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32h7r7a8 \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32h7s3a8 \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32h7s3l8 \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32h7s7z8 \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32l041f6 \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32l051k8 \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32l072cz \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32l073cz \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32l073rz \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32l151cb \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32l152re \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32l422cb \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32l431cb \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32l476vg \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32l496zg \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32l4a6zg \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32l4r5zi \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32l552ze \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv8m.main-none-eabihf --features pac,metadata,stm32n657x0 \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv8m.main-none-eabihf --features pac,metadata,stm32n647x0 \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv8m.main-none-eabihf --features pac,metadata,stm32n655x0 \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv8m.main-none-eabihf --features pac,metadata,stm32n645x0 \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32u031r8 \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32u073mb \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32u083rc \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32u585ai \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32u5a5zj \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32u5f9zj \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32u5g9nj \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32wb15cc \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32wb35ce \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32wb55rg \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv8m.main-none-eabihf --features pac,metadata,stm32wba50ke \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv8m.main-none-eabihf --features pac,metadata,stm32wba52cg \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv8m.main-none-eabihf --features pac,metadata,stm32wba55ug \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv8m.main-none-eabihf --features pac,metadata,stm32wba62mg \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv8m.main-none-eabihf --features pac,metadata,stm32wba63cg \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv8m.main-none-eabihf --features pac,metadata,stm32wba64ci \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv8m.main-none-eabihf --features pac,metadata,stm32wba65ri \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32wl54jc-cm4 \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32wl54jc-cm0p \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32wl55jc-cm4 \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32wl55jc-cm0p \
            --- check --manifest-path build/stm32-metapac/Cargo.toml --target thumbv7em-none-eabi --features pac,metadata,stm32wle5jb \

    ;;
    *)
        echo "unknown command"
    ;;
esac
