#!/bin/bash
set -e

cargo build --release --bin main

if [ -f target/release/main ]; then
	NAME=target/release/main
elif [ -f target/release/main.exe ]; then
	NAME=target/release/main.exe
else
	echo "can't find release artefact"
	exit 1
fi

mv $NAME name-needed
