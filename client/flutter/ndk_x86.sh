#!/usr/bin/env bash


export CFLAGS="-DBROKEN_CLANG_ATOMICS"
export CXXFLAGS="-DBROKEN_CLANG_ATOMICS"

cargo ndk --platform 21 --target i686-linux-android build --locked --release --features flutter
