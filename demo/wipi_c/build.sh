#!/bin/sh
# WIPI C(Clet) 데모 빌드 — dlunch/wipi SDK(Rust→thumbv4t ARM)로 컴파일해 KTF zip 포장.
# 요구: rustup nightly + rust-src (rustup toolchain install nightly --component rust-src)
# 산출: ../demo_wipi.zip (__adf__ + 00000000.jar(client.bin) + big.icon)
set -e
cd "$(dirname "$0")"

SDK=${WIPI_SDK:-/tmp/wipi-sdk-build}
[ -d "$SDK" ] || git clone --depth 1 https://github.com/dlunch/wipi.git "$SDK"

cp heart_catch.rs "$SDK/examples/src/heart_catch.rs"
grep -q 'name = "heart_catch"' "$SDK/examples/Cargo.toml" || cat >> "$SDK/examples/Cargo.toml" << 'TOML'

[[bin]]
name = "heart_catch"
path = "src/heart_catch.rs"
TOML
mkdir -p "$SDK/examples/resources/heart_catch"

cd "$SDK"
cargo -Zbuild-std=core,alloc build -p examples --target thumbv4t-none-eabi --features ktf --profile examples --no-default-features --bin heart_catch
cargo run -p wipi-archiver -- ktf target/thumbv4t-none-eabi/examples/heart_catch Clet 00000000 PD000000 ./examples/resources/heart_catch > /tmp/heart_catch_raw.zip

cd - > /dev/null
python3 - << 'PYEOF'
import zipfile
src = zipfile.ZipFile("/tmp/heart_catch_raw.zip")
dst = zipfile.ZipFile("../demo_wipi.zip", "w", zipfile.ZIP_DEFLATED)
for item in src.namelist():
    data = src.read(item)
    if item == "__adf__":
        data = data + "Name:데모: 하트 캐치\n".encode("euc-kr")
    dst.writestr(item, data)
dst.write("../assets/big.icon", "big.icon")
dst.close()
print("OK: demo_wipi.zip")
PYEOF
