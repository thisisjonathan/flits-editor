#!/bin/bash
cross build --target x86_64-unknown-linux-gnu --release
cross build --target x86_64-pc-windows-gnu --release
cargo about generate -o release/assets/shared/third-party-licenses.html about.hbs