#!/bin/bash -e

shopt -s expand_aliases
alias run="docker run --rm -it -v "$(pwd -W)":/home/rust/src messense/rust-musl-cross:armv7-musleabihf"

run cargo build --release
run musl-strip target/armv7-unknown-linux-musleabihf/release/mastodon-twitter-sync
