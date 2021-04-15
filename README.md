wget http://stmcufinder.com/API/getFiles.php -O sources/files.json
wget http://stmcufinder.com/API/getMCUsForMCUFinderPC.php -O sources/mcus.json

jq -r .Files[].URL < sources/files.json  | wget -N -i - 