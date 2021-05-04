#!/bin/bash

jq -r ".MCUs[] | select(.RPN == \"$1\") | .files[].file_id" sources/mcufinder/mcus.json
