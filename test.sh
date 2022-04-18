#!/usr/bin/bash

rm -r test/test
rm test/*cbz

set -e

cp -r test/orig test/test
cargo run -- test/test -v -d
