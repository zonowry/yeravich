#!/usr/bin/env bash
set -euo pipefail

if [[ $(uname -s) != Darwin ]]; then
    echo 'DMG verification requires macOS.' >&2
    exit 1
fi
if [[ $# != 1 || ! -f $1 ]]; then
    echo "Usage: $0 path/to/Yeravich-version-macos-arch.dmg" >&2
    exit 1
fi

repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
image_dir=$(cd -- "$(dirname -- "$1")" && pwd)
image_name=$(basename -- "$1")
cd "$image_dir"
shasum -a 256 --check "$image_name.sha256"

work_dir=$(mktemp -d)
mount_dir="$work_dir/volume"
mkdir -p "$mount_dir"
cleanup() {
    # 先卸载只读镜像，再删除临时目录；卸载失败时保留挂载点以便排查。
    if ! hdiutil detach "$mount_dir" > /dev/null 2>&1; then
        echo "Could not detach DMG at $mount_dir; leaving the temporary directory." >&2
        exit 1
    fi
    rm -rf -- "$work_dir"
}
trap cleanup EXIT
hdiutil attach "$image_dir/$image_name" -readonly -nobrowse -mountpoint "$mount_dir"

app_dir="$mount_dir/Yeravich.app"
test -x "$app_dir/Contents/MacOS/yeravich"
test -s "$app_dir/Contents/Resources/Yeravich.icns"
test "$(readlink "$mount_dir/Applications")" = /Applications
plutil -lint "$app_dir/Contents/Info.plist"
codesign --verify --deep --strict "$app_dir"

python3 - "$app_dir" "$repo_dir/config.example.toml" "$work_dir" <<'PY'
import os
from pathlib import Path
import plistlib
import subprocess
import sys

app, example, work = map(Path, sys.argv[1:])
with (app / "Contents/Info.plist").open("rb") as file:
    info = plistlib.load(file)
assert info["CFBundleIdentifier"] == "com.zonowry.yeravich"
assert info["CFBundleExecutable"] == "yeravich"
assert info["CFBundlePackageType"] == "APPL"
binary = app / "Contents/MacOS" / info["CFBundleExecutable"]

# 当前应用只链接 macOS 系统库；若未来增加非系统 dylib，必须一起打包。
linked = subprocess.check_output(["otool", "-L", str(binary)], text=True)
for line in linked.splitlines()[1:]:
    library = line.strip().split(" (", 1)[0]
    if not library.startswith(("/System/Library/", "/usr/lib/")):
        raise SystemExit(f"Unbundled runtime dependency: {library}")

# 使用显式的临时配置，不碰运行者的真实配置，也不提交翻译请求。
config = work / "config.toml"
config.write_bytes(example.read_bytes())
env = os.environ.copy()
env["YERAVICH_CONFIG"] = str(config)
process = subprocess.Popen([str(binary)], env=env)
try:
    try:
        status = process.wait(timeout=10)
    except subprocess.TimeoutExpired:
        pass
    else:
        raise SystemExit(f"Packaged app exited unexpectedly ({status}).")
finally:
    if process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
assert config.read_bytes() == example.read_bytes(), "Existing config was modified"
print("DMG smoke test passed: bundle verified, app stayed running, config preserved.")
PY
