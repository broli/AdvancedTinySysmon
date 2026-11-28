# AdvancedTinySysmon (`adsymon`)

Linux system monitoring daemon in Rust with zero external crate dependencies. Polls `/proc`, `/sys`, `statvfs`, and `nvidia-smi` (for NVIDIA GPUs) and atomically writes `status.json` to `$XDG_RUNTIME_DIR/adsymon` (or `/dev/shm/adsymon`) for status bars and desktop widgets.

## Build & Install

```bash
cargo install --path .
```

Or build without installing:

```bash
cargo build --release
./target/release/adsymon list
./target/release/adsymon list gpu
./target/release/adsymon gpu help
```

## Usage

Global options must come before target flags. Target flags are repeatable and subcategory flags attach to the preceding target:

```bash
adsymon \
  -i 3000 \
  --cpu --usage --temp \
  --memory --usage \
  --gpu pci-0000:00:02.0 --usage \
  --gpu pci-0000:01:00.0 --usage --mem --temp \
  --disk nvme0n1 --usage --fill --temp \
  --fans hwmon4/fan1 --rpm \
  --fans hwmon4/fan2 --rpm
```

If no targets are specified, `adsymon` defaults to `--cpu --usage --memory --usage`.

### Global Options

- `-i, --interval <MS>`: poll interval in ms (default: `1000`)
- `-o, --output-dir <DIR>`: output directory (default: `$XDG_RUNTIME_DIR/adsymon` or `/dev/shm/adsymon`)
- `--once`: poll once and exit
- `--stdout`, `--debug`: print JSON to stdout instead of writing `status.json`
- `-h, --help`, `-V, --version`: print help or version

### Targets & Metrics

- `--cpu [--usage] [--temp]`: `usage_pct`, `temp_c`
- `--memory [--usage]`: `total_kb`, `available_kb`, `used_kb`, `usage_pct`, `swap_total_kb`, `swap_used_kb`
- `--gpu <DEVICE> [--usage] [--mem] [--temp]`: `usage_pct` (or `freq_mhz`, `max_freq_mhz`, `freq_pct`), `vram_used_mb`, `vram_total_mb`, `vram_used_pct`, `temp_c`
- `--disk <DEVICE> [--usage] [--fill] [--temp]`: `usage_pct`, `read_kb`, `write_kb`, `io_ticks_ms`, `fill_pct`, `used_kb`, `avail_kb`, `total_kb`, `temp_c` (`<DEVICE>` can be a block name like `nvme0n1`, `/dev/nvme0n1`, or a mount path like `/`)
- `--fans <DEVICE> [--rpm]`: `rpm` (e.g. `hwmon4/fan1`)

## Output & Smoothing

Volatile readings (`usage_pct`, `temp_c`, `freq_mhz`, `freq_pct`, `rpm`, `vram_used_mb`, `vram_used_pct`, `available_kb`, `used_kb`, `swap_used_kb`) are averaged over the last 3 samples in memory to dampen short spikes. Static totals and monotonic counters (`total_kb`, `swap_total_kb`, `vram_total_mb`, `max_freq_mhz`, `read_kb`, `write_kb`, `io_ticks_ms`, `avail_kb`, `fill_pct`) are written verbatim.

Example `status.json`:

```json
{"timestamp_ms":1732814000123,"cpu":{"usage_pct":14.23,"temp_c":48.50},"memory":{"total_kb":32768000,"available_kb":18432000,"used_kb":14336000,"usage_pct":43.75,"swap_total_kb":0,"swap_used_kb":0},"gpu":{"pci-0000:01:00.0":{"usage_pct":12,"vram_used_mb":2048,"vram_total_mb":8188,"vram_used_pct":25.01,"temp_c":46.00}}}
```

## Frontends & Development

- Noctalia plugin: [frontends/noctalia/README.md](frontends/noctalia/README.md)
- `justfile` tasks: `just check`, `just test`, `just build`, `just install`, `just watch`
- Design notes: [docs/diary/001-initial-concept.md](docs/diary/001-initial-concept.md), [docs/architecture/01-cli-and-targets.md](docs/architecture/01-cli-and-targets.md)
