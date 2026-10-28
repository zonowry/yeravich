#!/usr/bin/env bash
set -euo pipefail

repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_dir/dist"
sha256sum --check ./*.sha256
images=("$repo_dir"/dist/*.AppImage)
if [[ ${#images[@]} != 1 || ! -f ${images[0]} ]]; then
    echo 'Expected exactly one AppImage in dist/.' >&2
    exit 1
fi

work_dir=$(mktemp -d)
trap 'rm -rf -- "$work_dir"' EXIT
cd "$work_dir"
"${images[0]}" --appimage-extract > /dev/null
test -x squashfs-root/usr/bin/wl-paste
test -f squashfs-root/com.zonowry.yeravich.desktop
test -f squashfs-root/com.zonowry.yeravich.svg
test -f squashfs-root/usr/share/X11/locale/en_US.UTF-8/Compose

# 独立配置目录和虚拟 X11 桌面，不读写开发者的配置，也不访问翻译服务。
# 正常的常驻进程应一直存活，直到 timeout 主动结束它。
status=0
timeout 10s xvfb-run -a env \
    -u YERAVICH_CONFIG -u WAYLAND_DISPLAY -u WAYLAND_SOCKET \
    XDG_SESSION_TYPE=x11 XDG_CONFIG_HOME="$work_dir/config" \
    ICED_BACKEND=tiny-skia \
    "$work_dir/squashfs-root/AppRun" || status=$?
if [[ $status != 124 ]]; then
    echo "Packaged app exited unexpectedly (status $status)." >&2
    exit 1
fi
cmp "$repo_dir/config.example.toml" "$work_dir/config/yeravich/config.toml"
echo 'AppImage smoke test passed: app stayed running and created the default config.'
