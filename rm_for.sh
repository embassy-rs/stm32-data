#!/bin/bash

#jq ".MCUs[] | select(.RPN == \"$1\") | .files[].file_id" sources/mcufinder/mcus.json

files=$(./files_for.sh $1)

#echo $files
for candidate in $files
do
  jq -e ".Files[] | select(.id_file == \"$candidate\") | select(.type == \"Reference manual\")" ./sources/mcufinder/files.json
done
