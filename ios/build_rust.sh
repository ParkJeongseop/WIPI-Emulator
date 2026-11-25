#!/bin/sh
# Xcode Build Phase에서 자동 실행 — 현재 빌드 대상(SDK/아키텍처)에 맞는 wipi_ios 스태틱 라이브러리를 cargo로 빌드.
# Xcode ⌘R 한 번으로 Rust까지 최신화되므로 수동 cargo 단계가 필요 없다.
set -e

# cargo(brew rustup, keg-only)는 기본 PATH에 없으므로 명시적으로 추가
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"

# 시뮬레이터 vs 실기기 → Rust 타깃 선택
case "$PLATFORM_NAME" in
  iphonesimulator) RUST_TARGET="aarch64-apple-ios-sim" ;;
  *)               RUST_TARGET="aarch64-apple-ios" ;;
esac

cd "$SRCROOT/../rust"

# release-ios 프로필: fat LTO (rustc가 LTO를 끝낸 기계어 오브젝트만 아카이브에 담는다)
cargo build --profile release-ios --target "$RUST_TARGET" -p wipi_ios

echo "wipi_ios ($RUST_TARGET) 빌드 완료"
