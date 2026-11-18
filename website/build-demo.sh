#!/usr/bin/env sh
# Builds the web demo into static/demo for `zola serve`.
set -e
# Git Bash on Windows would rewrite /demo/ into a Windows path.
export MSYS_NO_PATHCONV=1
cd "$(dirname "$0")/../examples/web"
trunk build --release --public-url /demo/ --dist ../../website/static/demo
