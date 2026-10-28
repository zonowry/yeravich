#!/usr/bin/env bash
set -euo pipefail

if [[ $(uname -s) != Darwin ]]; then
    echo 'Application signing requires macOS.' >&2
    exit 1
fi
if [[ $# != 1 || ! -d $1/Contents/MacOS ]]; then
    echo "Usage: $0 path/to/Yeravich.app" >&2
    exit 1
fi
if [[ -z ${MACOS_SIGNING_IDENTITY:-} || $MACOS_SIGNING_IDENTITY == - ]]; then
    echo 'MACOS_SIGNING_IDENTITY must name a fixed certificate; ad-hoc signing is not supported here.' >&2
    exit 1
fi

app_dir=$1
signing_args=(--force --sign "$MACOS_SIGNING_IDENTITY" --timestamp=none)
if [[ -n ${MACOS_SIGNING_KEYCHAIN:-} ]]; then
    signing_args+=(--keychain "$MACOS_SIGNING_KEYCHAIN")
fi
codesign "${signing_args[@]}" "$app_dir"
codesign --verify --deep --strict "$app_dir"

# 检查签名确实包含证书；只校验完整性不能区分证书签名和 ad-hoc。
work_dir=$(mktemp -d)
trap 'rm -rf -- "$work_dir"' EXIT
codesign --display --extract-certificates="$work_dir/certificate-" "$app_dir"
if [[ ! -s $work_dir/certificate-0 ]]; then
    echo 'The application signature does not contain a signing certificate.' >&2
    exit 1
fi
echo 'Certificate signature verified. This does not imply Apple notarization or Gatekeeper approval.'
