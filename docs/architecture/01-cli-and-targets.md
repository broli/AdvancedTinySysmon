# Architecture 01 — CLI Grammar, Targets & Output Layout

**Date:** 2026-11-27  
**Status:** Decided

---

## 1. CLI Model (Hybrid)

`adsymon` separates **one-shot inspection commands** from **daemon monitoring mode**:

### One-Shot Commands
- `adsymon help` / `adsymon --help` — top-level usage and available categories.
- `adsymon <category> help` — level-1 help for a specific category (e.g. `adsymon gpu help`, `adsymon cpu help`), listing its target selector syntax and subcategory flags.
- `adsymon list` — lists all supported monitoring categories.
- `adsymon list <category>` — discovers and prints available hardware targets for that category (e.g. `adsymon list gpu`, `adsymon list disk`, `adsymon list fans`).

### Daemon Mode (Repeatable Target Groups)
When launched without a one-shot command, `adsymon` parses a sequence of **category flags** (`--cpu`, `--memory`, `--gpu`, `--disk`, `--fans`), where each category flag opens a new target group and subsequent **subcategory flags** attach to that group:

```bash
adsymon \
  --gpu /dev/dri/by-path/pci-0000:01:00.0-render --mem --temp \
  --gpu /dev/dri/by-path/pci-0000:00:02.0-render --mem \
  --cpu --usage --temp \
  --memory --usage
```

Global daemon options (e.g. `--interval <ms>`, `--output-dir <path>`) can appear before the target groups.

---

## 2. Categories & Subcategories

| Category | Target Selector | Subcategories (Metrics) | Notes |
| --- | --- | --- | --- |
| `cpu` | Optional (default: aggregate) or core/package | `--usage`, `--temp` | Aggregate + per-package temp via `hwmon` |
| `memory` | None (system-wide) | `--usage` (RAM + swap) | System RAM & swap only |
| `gpu` | Device path / card / PCI ID | `--usage`, `--mem` (VRAM), `--temp` | VRAM lives here, scoped per GPU device |
| `disk` | Block device / mount (e.g. `nvme0n1`) | `--usage`, `--temp` | I/O or capacity + `hwmon` / SMART temp |
| `fans` | Fan / `hwmon` channel | `--rpm` / `--usage` (%), `--temp` | Multiple fans can be repeated |

*Note:* If a category flag is passed with no subcategory flags, it can either default to all metrics for that category or require explicit flags (to be finalized).

---

## 3. Conceptual Target Model (Parser -> Main Loop)

The CLI parser acts as a small state machine: each level-1 flag (`--gpu`, `--cpu`, etc.) pushes the previous target and starts a new one; each subcategory flag toggles a bit/bool on the active target.

```rust
enum Target {
    Cpu { usage: bool, temp: bool },
    Memory { usage: bool },
    Gpu { device: String, usage: bool, vram: bool, temp: bool },
    Disk { device: String, usage: bool, temp: bool },
    Fans { device: String, rpm: bool },
}
```

Once parsed, the main loop owns `Vec<Target>` (plus any per-target state like previous CPU/GPU tick counters) and polls only what was requested.

---

## 4. Output Layout (`status.json` in `tmpfs`)

All enabled targets are serialized into a single `status.json` file in RAM (`$XDG_RUNTIME_DIR/adsymon/status.json` or `/dev/shm/adsymon/status.json`) via atomic temp-file + `rename`, so frontends (Waybar, Noctalia, etc.) read one consistent snapshot per interval.

### Multi-Device Keying: Hardware ID Map

When multiple instances of the same category are monitored (e.g. two `--gpu` flags), each instance is keyed by its stable hardware/sysfs identifier in `status.json`:

```json
"gpu": {
  "pci-0000:01:00.0": { "vram_used_mb": 4096, "temp_c": 62 },
  "pci-0000:00:02.0": { "vram_used_mb": 512,  "temp_c": 44 }
}
```

This leaves no ambiguity for consumers regardless of CLI flag ordering.
