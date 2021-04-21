SHELL=/bin/bash

clean:
	rm -rf sources
prepare:
	mkdir -p sources
	mkdir -p sources/metadata
	mkdir -p sources/files
	mkdir -p sources/mcu
	mkdir -p sources/svd
	mkdir -p sources/.tmp

metadata: prepare
	wget http://stmcufinder.com/API/getFiles.php -O sources/metadata/files.json
	wget http://stmcufinder.com/API/getMCUsForMCUFinderPC.php -O sources/metadata/mcus.json	

files: metadata
	jq -r .Files[].URL < sources/metadata/files.json  | wget -P sources/files/ -N -i -

#mcu_dirs: metadata
	#jq -r '.MCUs[] | select(.name|test("STM32.*")) | .name' < sources/metadata/mcus.json \
	#| sed 's/\(STM32[A-Z]*[0-9]*\)\(.*\)/\1/' \
	#| sort | uniq \
	#| awk '{print "sources/mcus/" tolower($0)}' \
	#| xargs mkdir -p

svd:
	git clone https://github.com/stm32-rs/stm32-rs.git ./sources/.tmp/stm32-rs
	cd ./sources/.tmp/stm32-rs && make svdformat

mcu_dirs: 
	ls -1 ./sources/.tmp/stm32-rs/svd/*.formatted | xargs basename | cut -f 1 -d . \
	| awk '{print "sources/mcu/" tolower($0)}' \
	| xargs mkdir -p 

	ls -1 ./sources/.tmp/stm32-rs/svd/*.formatted | xargs basename | cut -f 1 -d . \
	| awk '{print "./sources/.tmp/stm32-rs/svd/" tolower($0) ".svd.formatted" " sources/mcu/" tolower($0) "/" tolower($0) ".svd" }' \
	| tr ' ' '\n' \
	| xargs -n2 cp
	
	
