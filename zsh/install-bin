#!/bin/sh

set -eux

root="$(dirname "$(realpath "$0")")"
install_path="$1"

echo 'Compiling and installing zsh...'

repo_path="$root/bin-submodules/zsh"
git submodule update --init "$repo_path"
cd "$repo_path"
sudo apt install --yes --no-install-recommends autoconf gcc make ncurses-dev yodl
./Util/preconfig
./configure --prefix="$install_path"
make
make install
