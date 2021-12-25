#!/bin/bash
set -ex

SUFFIX="$1"
PLATFORM="$2"

# TODO declare sdl version somewhere else
SDL_VERSION="2.0.14"

# declare output first via stdout to actions
if [[ "$PLATFORM" == "linux" ]]; then
	TARGET=nn-linux.tar.gz
elif [[ "$PLATFORM" == "windows" ]]; then
	TARGET=nn-windows.zip
else
	exit 1
fi
echo $TARGET

if [[ "$PLATFORM" == "linux" ]]; then
	SDL_FILE="SDL2-$SDL_VERSION"
	wget "https://www.libsdl.org/release/$SDL_FILE.zip"
	unzip $SDL_FILE.zip

	pushd $SDL_FILE
	./configure
	make -j$(nproc)
	sudo make install
	popd
elif [[ "$PLATFORM" == "windows" ]]; then
	SDL_ZIP="SDL2-devel-$SDL_VERSION-VC.zip"
	python .build/download-and-unzip.py "https://www.libsdl.org/release/$SDL_ZIP"

	SDL_LIBS="SDL2-$SDL_VERSION/lib/x64"
	find $SDL_LIBS -iname '*.lib' -exec cp -v {} "/c/Users/$USERNAME/.rustup/toolchains/stable-x86_64-pc-windows-msvc/lib/rustlib/x86_64-pc-windows-msvc/lib" \;
	find $SDL_LIBS -iname '*.dll' -exec cp -v {} ./renderer/engine/ \;
fi

DIR=name-needed
mkdir -p $DIR

cargo build --release --bin main
mv -v "target/release/main$SUFFIX" $DIR/name-needed$SUFFIX

mv -v README.md LICENSE resources $DIR
rm -f $DIR/resources/ci_test.ron

# package up
if [[ "$PLATFORM" == "linux" ]]; then
	tar czf $TARGET name-needed
elif [[ "$PLATFORM" == "windows" ]]; then
	find ./renderer/engine -iname '*.dll' -exec cp -v {} name-needed \;

	7z a $TARGET name-needed
fi
