#!/bin/bash
downloadFileIfItDoesntExist() {
  local DOWNLOADFILE=$1
  local URLDOWNLOAD=$2
  [[ -e "$DOWNLOADFILE" ]] || curl -L -o "$DOWNLOADFILE" "$URLDOWNLOAD"
}

cross build --target x86_64-unknown-linux-gnu --release
cross build --target x86_64-pc-windows-gnu --release
mkdir -p release/assets/shared
cargo about generate -o release/assets/shared/third-party-licenses.html about.hbs
mkdir -p release/downloads

# --- download linux assets and create tarball ---

# ruffle
downloadFileIfItDoesntExist 'release/downloads/ruffle-linux-x86_64.tar.gz' 'https://github.com/ruffle-rs/ruffle/releases/download/nightly-2024-10-12/ruffle-nightly-2024_10_12-linux-x86_64.tar.gz'
mkdir -p release/downloads/x86_64-unknown-linux-gnu/ruffle
tar -xzvf release/downloads/ruffle-linux-x86_64.tar.gz -C release/downloads/x86_64-unknown-linux-gnu/ruffle
mkdir -p release/assets/x86_64-unknown-linux-gnu/dependencies
mv release/downloads/x86_64-unknown-linux-gnu/ruffle/LICENSE.md release/assets/x86_64-unknown-linux-gnu/LICENSE-ruffle.md
mv release/downloads/x86_64-unknown-linux-gnu/ruffle/ruffle release/assets/x86_64-unknown-linux-gnu/dependencies/ruffle

# mtasc
mkdir -p release/downloads/x86_64-unknown-linux-gnu/mtasc
# this is where i would download mtasc from the internet archive if the internet archive wasn't being DDOSed right now
# instead i manually copied my earlier downloaded files over
# in the future, maybe we should get these files from previous releases
mv release/downloads/x86_64-unknown-linux-gnu/mtasc/Readme.txt release/assets/x86_64-unknown-linux-gnu/LICENSE-mtasc.txt
mv release/downloads/x86_64-unknown-linux-gnu/mtasc/mtasc release/assets/x86_64-unknown-linux-gnu/dependencies/mtasc
mv release/downloads/x86_64-unknown-linux-gnu/mtasc/std release/assets/x86_64-unknown-linux-gnu/dependencies/std
mv release/downloads/x86_64-unknown-linux-gnu/mtasc/std8 release/assets/x86_64-unknown-linux-gnu/dependencies/std8

# create tarball
tar -czvf release/flits-editor-x86_64-unknown-linux-gnu.tar.gz --owner=1000 --group=1000 LICENSE -C target/x86_64-unknown-linux-gnu/release/ flits-editor -C ../../../release/assets/shared $(ls release/assets/shared) -C ../x86_64-unknown-linux-gnu $(ls release/assets/x86_64-unknown-linux-gnu)

# --- download windows assets and create zip ---

# ruffle
downloadFileIfItDoesntExist 'release/downloads/ruffle-windows-x86_64.zip' 'https://github.com/ruffle-rs/ruffle/releases/download/nightly-2024-10-12/ruffle-nightly-2024_10_12-windows-x86_64.zip'
mkdir -p release/downloads/x86_64-pc-windows-gnu/ruffle
unzip -u release/downloads/ruffle-windows-x86_64.zip -d release/downloads/x86_64-pc-windows-gnu/ruffle
mkdir -p release/assets/x86_64-pc-windows-gnu/dependencies
mv release/downloads/x86_64-pc-windows-gnu/ruffle/LICENSE.md release/assets/x86_64-pc-windows-gnu/LICENSE-ruffle.md
mv release/downloads/x86_64-pc-windows-gnu/ruffle/ruffle.exe release/assets/x86_64-pc-windows-gnu/dependencies/ruffle.exe

# mtasc
mkdir -p release/downloads/x86_64-pc-windows-gnu/mtasc
# same comment as above about missing files because the internet archive is down
mv release/downloads/x86_64-pc-windows-gnu/mtasc/Readme.txt release/assets/x86_64-pc-windows-gnu/LICENSE-mtasc.txt
mv release/downloads/x86_64-pc-windows-gnu/mtasc/mtasc.exe release/assets/x86_64-pc-windows-gnu/dependencies/mtasc.exe
mv release/downloads/x86_64-pc-windows-gnu/mtasc/std release/assets/x86_64-pc-windows-gnu/dependencies/std
mv release/downloads/x86_64-pc-windows-gnu/mtasc/std8 release/assets/x86_64-pc-windows-gnu/dependencies/std8

# create zipfile
# remove old one otherwise zip will add to it
rm release/flits-editor-x86_64-pc-windows-gnu.zip
zip release/flits-editor-x86_64-pc-windows-gnu.zip LICENSE -j target/x86_64-pc-windows-gnu/release/flits-editor.exe -rj release/assets/shared
cd release/assets/x86_64-pc-windows-gnu
zip ../../flits-editor-x86_64-pc-windows-gnu.zip -r .
cd ../../../