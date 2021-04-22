SHELL=/bin/bash

clean:
	rm -rf sources
	rm -rf tmp

metadata: sources/metadata/files.json sources/metadata/mcus.json

sources/metadata/files.json:
	mkdir -p sources/metadata
	wget http://stmcufinder.com/API/getFiles.php -O sources/metadata/files.json

sources/metadata/mcus.json:
	mkdir -p sources/metadata
	wget http://stmcufinder.com/API/getMCUsForMCUFinderPC.php -O sources/metadata/mcus.json	

sources/files: metadata
	mkdir -p sources/files
	jq -r .Files[].URL < sources/metadata/files.json  | wget -P sources/files/ -N -i -

svd: sources/.tmp/stm32-rs
	mkdir -p sources/svd
	ls -1 ./sources/.tmp/stm32-rs/svd/*.formatted | xargs basename | cut -f 1 -d . \
	| awk '{print "sources/.tmp/stm32-rs/svd/" $$0 ".svd.formatted" " " "sources/svd/" $$0 ".svd"}' \
	| xargs -n2 cp

sources/.tmp/stm32-rs:
	rm -rf ./sources/.tmp/stm32-rs
	git clone https://github.com/stm32-rs/stm32-rs.git ./sources/.tmp/stm32-rs
	cd ./sources/.tmp/stm32-rs && make svdformat

mcu_dirs: svd metadata

	ls -1 ./sources/.tmp/stm32-rs/svd/*.formatted | xargs basename | cut -f 1 -d . \
	| awk '{print "./sources/.tmp/stm32-rs/svd/" $$0 ".svd.formatted" " sources/mcu/" $$0 "/" $$0 ".svd" }' \
	| tr ' ' '\n' \
	| xargs -n2 cp
	
	
