# AdvancedTinySysmon (`adsymon`)

Linux system monitoring daemon in Rust. Polls selected hardware and OS metrics from `/proc`, `/sys`, and `nvidia-smi` (for NVIDIA GPUs) and writes `status.json` atomically into `tmpfs` (`$XDG_RUNTIME_DIR/adsymon` or `/dev/shm/adsymon`) for status bars and desktop shells (Waybar, Noctalia, AGS, eww, `jq`, etc.).

Architecture and design notes live in:
- [docs/diary/001-initial-concept.md](docs/diary/001-initial-concept.md)
- [docs/architecture/01-cli-and-targets.md](docs/architecture/01-cli-and-targets.md)

## Build & Discovery

```bash
cargo build --release
./target/release/adsymon list
./target/release/adsymon list gpu
./target/release/adsymon gpu help
```

## Usage

Global options come first, followed by repeatable category groups (`--cpu`, `--memory`, `--gpu <dev>`, `--disk <dev>`, `--fans <dev>`):

```bash
./target/release/adsymon \
  --interval 1000 \
  --cpu --usage --temp \
  --memory --usage \
  --gpu pci-0000:00:02.0 --usage \
  --gpu pci-0000:01:00.0 --usage --mem --temp \
  --disk nvme0n1 --usage \
  --fans hwmon4/fan1 --rpm \
  --fans hwmon4/fan2 --rpm
```

Useful flags:
- `--once`: poll once and exit
- `--stdout` / `--debug`: print JSON to stdout instead of writing `status.json`
- `-i, --interval <MS>`: poll interval in ms (default: `1000`)
- `-o, --output-dir <DIR>`: output directory (default: `$XDG_RUNTIME_DIR/adsymon` or `/dev/shm/adsymon`)

See `just --list` for `justfile` shortcuts (`just check`, `just test`, `just build`, `just watch`).
