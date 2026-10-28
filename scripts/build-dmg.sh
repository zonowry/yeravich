#!/usr/bin/env bash
set -euo pipefail

if [[ $(uname -s) != Darwin ]]; then
    echo 'DMG packaging requires macOS and Xcode Command Line Tools.' >&2
    exit 1
fi
if [[ $# -gt 1 ]]; then
    echo "Usage: $0 [aarch64-apple-darwin|x86_64-apple-darwin]" >&2
    exit 1
fi

if [[ -z ${MACOS_SIGNING_IDENTITY:-} || $MACOS_SIGNING_IDENTITY == - ]]; then
    echo 'Set MACOS_SIGNING_IDENTITY to your fixed code-signing certificate name or SHA-1 fingerprint. See README.md.' >&2
    exit 1
fi

target=${1:-}
if [[ -z "$target" ]]; then
    case $(uname -m) in
        arm64) target=aarch64-apple-darwin ;;
        x86_64) target=x86_64-apple-darwin ;;
        *) echo 'Unsupported Mac architecture.' >&2; exit 1 ;;
    esac
fi
case "$target" in
    aarch64-apple-darwin) arch=arm64 ;;
    x86_64-apple-darwin) arch=x86_64 ;;
    *) echo "Unsupported Rust target: $target" >&2; exit 1 ;;
esac

for tool in cargo python3 rsvg-convert iconutil plutil codesign hdiutil lipo shasum; do
    command -v "$tool" > /dev/null || {
        echo "Missing tool: $tool. See the macOS DMG build instructions in README.md." >&2
        exit 1
    }
done

repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_dir"
build_dir="$repo_dir/target/dmg/$arch"
mkdir -p "$build_dir" "$repo_dir/dist"
export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-11.0}"

cargo build --release --locked --target "$target"
cargo metadata --locked --no-deps --format-version 1 > "$build_dir/metadata.json"
version=$(python3 -c 'import json,sys; print(next(p["version"] for p in json.load(open(sys.argv[1]))["packages"] if p["name"] == "yeravich"))' "$build_dir/metadata.json")
target_dir=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["target_directory"])' "$build_dir/metadata.json")

work_dir=$(mktemp -d "$build_dir/staging.XXXXXX")
trap 'rm -rf -- "$work_dir"' EXIT
image_root="$work_dir/image"
app_dir="$image_root/Yeravich.app"
mkdir -p "$app_dir/Contents/MacOS" "$app_dir/Contents/Resources"
install -m755 "$target_dir/$target/release/yeravich" "$app_dir/Contents/MacOS/yeravich"
lipo "$app_dir/Contents/MacOS/yeravich" -verify_arch "$arch"

# Cargo 使用 SemVer；bundle 版本只保留数字段，DMG 文件名保留完整版本。
python3 - data/macos/Info.plist "$app_dir/Contents/Info.plist" "$version" "$MACOSX_DEPLOYMENT_TARGET" <<'PY'
import plistlib
import re
import sys

source, destination, version, minimum = sys.argv[1:]
bundle_version = re.fullmatch(r"(\d+\.\d+\.\d+)(?:[-+].*)?", version)
if bundle_version is None:
    raise SystemExit(f"Invalid application version: {version}")
with open(source, "rb") as file:
    info = plistlib.load(file)
info["CFBundleShortVersionString"] = bundle_version[1]
info["CFBundleVersion"] = bundle_version[1]
info["LSMinimumSystemVersion"] = minimum
with open(destination, "wb") as file:
    plistlib.dump(info, file, sort_keys=False)
PY
plutil -lint "$app_dir/Contents/Info.plist"

# 从同一个 SVG 渲染所有尺寸，保持 Linux 与 macOS 的图标一致。
iconset="$work_dir/Yeravich.iconset"
mkdir -p "$iconset"
for size in 16 32 128 256 512; do
    for scale in 1 2; do
        pixels=$((size * scale))
        suffix=''
        if [[ $scale == 2 ]]; then suffix='@2x'; fi
        rsvg-convert --width "$pixels" --height "$pixels" \
            data/com.zonowry.yeravich.svg \
            --output "$iconset/icon_${size}x${size}${suffix}.png"
    done
done
iconutil --convert icns --output "$app_dir/Contents/Resources/Yeravich.icns" "$iconset"

# 每个版本使用同一证书；缺少证书时失败，不回退到 ad-hoc。
bash scripts/sign-macos-app.sh "$app_dir"
ln -s /Applications "$image_root/Applications"

image_name="Yeravich-$version-macos-$arch.dmg"
hdiutil create -volname Yeravich -srcfolder "$image_root" \
    -format UDZO -fs HFS+ -ov "$repo_dir/dist/$image_name"
hdiutil verify "$repo_dir/dist/$image_name"
cd "$repo_dir/dist"
shasum -a 256 "$image_name" > "$image_name.sha256"
echo "DMG: $repo_dir/dist/$image_name"
