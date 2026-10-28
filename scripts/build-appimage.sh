#!/usr/bin/env bash
set -euo pipefail

if [[ $(uname -s) != Linux || $(uname -m) != x86_64 ]]; then
    echo 'AppImage packaging currently requires Linux x86_64.' >&2
    exit 1
fi

repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_dir"
build_dir="$repo_dir/target/appimage"
mkdir -p "$build_dir" "$repo_dir/dist"

cargo build --release --locked --target x86_64-unknown-linux-gnu
cargo metadata --locked --no-deps --format-version 1 > "$build_dir/metadata.json"
version=$(python3 -c 'import json,sys; print(next(p["version"] for p in json.load(open(sys.argv[1]))["packages"] if p["name"] == "yeravich"))' "$build_dir/metadata.json")
target_dir=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["target_directory"])' "$build_dir/metadata.json")

# 固定打包工具版本和校验值，避免 continuous 更新后产物不可复现。
linuxdeploy="$build_dir/linuxdeploy-x86_64.AppImage"
if [[ ! -f "$linuxdeploy" ]]; then
    curl --fail --location --retry 3 \
        'https://github.com/linuxdeploy/linuxdeploy/releases/download/1-alpha-20251107-1/linuxdeploy-x86_64.AppImage' \
        --output "$linuxdeploy.download"
    mv "$linuxdeploy.download" "$linuxdeploy"
fi
echo "c20cd71e3a4e3b80c3483cef793cda3f4e990aca14014d23c544ca3ce1270b4d  $linuxdeploy" | sha256sum --check
chmod +x "$linuxdeploy"

app_dir=$(mktemp -d "$build_dir/AppDir.XXXXXX")
trap 'rm -rf -- "$app_dir"' EXIT

# winit 等通过 dlopen 加载这些库，ldd 无法发现，需显式交给 linuxdeploy。
# 显卡驱动、图形服务、字体和 Portal 仍使用宿主桌面提供的资源。
libraries=(
    libX11.so.6 libX11-xcb.so.1 libXcursor.so.1 libXi.so.6
    libXrandr.so.2 libXinerama.so.1 libXrender.so.1 libXfixes.so.3
    libxcb.so.1 libxkbcommon.so.0 libxkbcommon-x11.so.0
    libwayland-client.so.0 libwayland-cursor.so.0 libwayland-egl.so.1
)
library_args=()
for library in "${libraries[@]}"; do
    library_path=$(ldconfig -p | awk -v name="$library" '$1 == name && /x86-64/ && !found {print $NF; found=1} END {if (!found) exit 1}') || {
        echo "Missing library: $library. See the AppImage build dependencies in README.md." >&2
        exit 1
    }
    library_args+=(--library "$library_path")
done

# wl-paste 用于 Wayland 文本带入；随包分发，避免用户另行安装工具。
wl_paste=$(command -v wl-paste)
mkdir -p "$app_dir/usr/share/X11"
cp -a /usr/share/X11/locale "$app_dir/usr/share/X11/locale"
install -Dm644 /usr/share/doc/libx11-data/copyright "$app_dir/usr/share/doc/libx11-data/copyright"

export ARCH=x86_64
export APPIMAGE_EXTRACT_AND_RUN=1
export LINUXDEPLOY_OUTPUT_VERSION="$version"
export LDAI_OUTPUT="$repo_dir/dist/Yeravich-$version-x86_64.AppImage"
export LDAI_NO_APPSTREAM=1
"$linuxdeploy" \
    --appdir "$app_dir" \
    --executable "$target_dir/x86_64-unknown-linux-gnu/release/yeravich" \
    --executable "$wl_paste" \
    --desktop-file data/com.zonowry.yeravich.desktop \
    --icon-file data/com.zonowry.yeravich.svg \
    --custom-apprun data/AppRun \
    "${library_args[@]}" \
    --output appimage

cd "$repo_dir/dist"
sha256sum "$(basename "$LDAI_OUTPUT")" > "$(basename "$LDAI_OUTPUT").sha256"
echo "AppImage: $LDAI_OUTPUT"
