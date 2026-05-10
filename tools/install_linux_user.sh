#!/usr/bin/env sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/.." && pwd)

bin_dir=${XDG_BIN_HOME:-"$HOME/.local/bin"}
data_dir=${XDG_DATA_HOME:-"$HOME/.local/share"}
icon_path="$data_dir/icons/hicolor/256x256/apps/nokk.png"
desktop_path="$data_dir/applications/dev.n3n.Nokk.desktop"

cargo build --release -p nokk --bin nokk

install -Dm755 "$repo_root/target/release/nokk" "$bin_dir/nokk"
install -Dm644 "$repo_root/assets/nokk/preview.png" "$icon_path"

mkdir -p "$(dirname -- "$desktop_path")"
sed \
    -e "s|@BINDIR@|$bin_dir|g" \
    -e "s|@ICON@|$icon_path|g" \
    "$repo_root/packaging/linux/dev.n3n.Nokk.desktop.in" > "$desktop_path"
chmod 644 "$desktop_path"

if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$data_dir/applications" >/dev/null 2>&1 || true
fi

printf 'Installed Nokk desktop app.\n'
printf 'Run from your app launcher, or with: %s/nokk desktop\n' "$bin_dir"
