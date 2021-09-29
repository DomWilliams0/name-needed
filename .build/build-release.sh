#!/bin/bash
set -e

# nested so the next step can zip up name-needed/name-needed non-recursively and end up with a nice name-needed/$FILES structure
DIR=name-needed/name-needed
mkdir -p $DIR

# ensure suffix is passed if needed e.g. ".exe"
cargo build --release --bin main
mv -v "target/release/main$1" $DIR/name-needed$1

# make it executable
if [ -x "$(command -v chmod)" ]; then
  chmod +x $DIR/name-needed$1
fi

mv -v README.md LICENSE resources $DIR
rm -f $DIR/resources/ci_test.ron