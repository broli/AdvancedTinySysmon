# AdvancedTinySysmon

Linux system monitoring daemon in Rust. Polls hardware and OS metrics from `/proc` and `/sys` and writes `status.json` atomically into `tmpfs` (`$XDG_RUNTIME_DIR/atsm` or `/dev/shm/atsm`) so bars and desktop shells (Waybar, Noctalia, AGS, eww, `jq` scripts) can read current state without spawning subprocesses per widget.

## Current Status

The current binary (`atsm`) is an initial working scaffold implementing:

- `cpu` — aggregate CPU usage from `/proc/stat`
- `memory` — RAM and swap usage from `/proc/meminfo`
- `zram` — compressed block device stats from `/sys/block/zram*`

Design notes and the target CLI/module architecture (repeatable device flags, `gpu`/`disk`/`fans`, `list` discovery) live in:

- [docs/diary/001-initial-concept.md](docs/diary/001-initial-concept.md)
- [docs/architecture/01-cli-and-targets.md](docs/architecture/01-cli-and-targets.md)

## Build & Run

```bash
cargo build --release
./target/release/atsm --list-modules
./target/release/atsm --modules cpu,memory,zram --once
./target/release/atsm --modules cpu,memory,zram --interval 1000
```

Options:

- `-m, --modules <LIST>` — comma-separated modules (default: `cpu,memory`)
- `-i, --interval <MS>` — poll interval in ms (default: `1000`)
- `-o, --output-dir <DIR>` — output directory (default: `$XDG_RUNTIME_DIR/atsm` or `/dev/shm/atsm`)
- `--once` — poll once and exit
- `--list-modules` — print available modules and exit
- `-h, --help`, `-V, --version`
