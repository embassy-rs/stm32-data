#!/bin/bash

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
        ./d download-mcufinder
        ./d download-svd
        ./d download-headers
        ./d download-cubedb
        ./d download-cubeprogdb
    ;;
    download-mcufinder)
        mkdir -p sources/mcufinder
        wget http://stmcufinder.com/API/getFiles.php -O sources/mcufinder/files.json
        wget http://stmcufinder.com/API/getMCUsForMCUFinderPC.php -O sources/mcufinder/mcus.json	
    ;;
    download-pdf)
	    jq -r .Files[].URL < sources/mcufinder/files.json  | wget -P sources/pdf/ -N -i -
    ;;
    download-svd)
    	rm -rf ./sources/git/stm32-rs
        git clone --depth 1 https://github.com/stm32-rs/stm32-rs.git ./sources/git/stm32-rs
        (cd ./sources/git/stm32-rs; make svdformat)
        mkdir -p sources/svd
        for f in ./sources/git/stm32-rs/svd/*.formatted; do
            base=$(basename $f | cut -f 1 -d .)
            cp $f sources/svd/$base.svd
        done
    ;;
    download-headers)
        for f in F0 F1 F2 F3 F4 F7 H7 L0 L1 L4 L5 G0 G4 WB WL; do
            rm -rf ./sources/git/STM32Cube$f
            git clone --depth 1 https://github.com/STMicroelectronics/STM32Cube$f sources/git/STM32Cube$f
        done
        rm -rf sources/headers
        mkdir -p sources/headers
        cp sources/git/STM32Cube*/Drivers/CMSIS/Device/ST/STM32*/Include/*.h sources/headers
        rm sources/headers/stm32??xx.h
        rm sources/headers/system_*.h
        rm sources/headers/partition_*.h
    ;;
    download-cubedb)
        rm -rf sources/cubedb
        git clone --depth 1 https://github.com/embassy-rs/stm32cube-database.git sources/cubedb
    ;;
    download-cubeprogdb)
        rm -rf sources/cubeprogdb
        git clone --depth 1 https://github.com/embassy-rs/stm32cubeprog-database.git sources/cubeprogdb
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
