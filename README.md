# AdvancedTinySysmon (`atsm`)

Small Rust daemon that polls selected Linux metrics from `/proc` and `/sys` and writes `status.json` atomically into RAM-backed `tmpfs` (`$XDG_RUNTIME_DIR/atsm` or `/dev/shm/atsm`) for status bars and scripts (Waybar, Noctalia, AGS, eww, `jq`, etc.).

## Usage

```bash
cargo build --release

./target/release/atsm --list-modules
./target/release/atsm --modules cpu,memory,zram --once
./target/release/atsm --modules cpu,memory,zram --interval 1000
```

## Options

| Flag | Description | Default |
| --- | --- | --- |
| `-m, --modules <LIST>` | Comma-separated modules to enable | `cpu,memory` |
| `-i, --interval <MS>` | Polling interval in ms | `1000` |
| `-o, --output-dir <DIR>` | Output directory (prefer `tmpfs`) | `$XDG_RUNTIME_DIR/atsm` or `/dev/shm/atsm` |
| `--once` | Poll once and exit | `false` |
| `--list-modules` | Print available modules and exit | `false` |
| `-h, --help` | Print help | - |
| `-V, --version` | Print version | - |
