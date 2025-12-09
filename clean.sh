#!/bin/sh

./yamlfmt -v data/registers/hrtim_v2.yaml
sed -i '' 's/"/'"'"'/g' data/registers/hrtim_v2.yaml


