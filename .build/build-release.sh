#!/bin/bash
set -e

DIR=name-needed/name-needed
mkdir -p $DIR

# ensure suffix is passed if needed e.g. ".exe"
#cargo build --release --bin main
#mv -v "target/release/main$1" $DIR/name-needed$1

touch $DIR/name-needed$1

mv -v README.md LICENSE resources $DIR

# unwanted files
rm -f $DIR/resources/ci_test.ron