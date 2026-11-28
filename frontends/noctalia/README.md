# Noctalia v5+ Plugin (`broli/adsymon`)

Reads `status.json` produced by `adsymon` and exposes both:
- `group` — configurable `string_map` widget (add multiple instances to left/middle/right bar zones with any `Label -> path` entries).
- `cpu`, `memory`, `gpu` — dedicated named bar widgets.

## Install (symlink for local dev)

```fish
ln -s (pwd)/frontends/noctalia ~/.config/noctalia/plugins/adsymon
```

## Example `group` Metric Paths

- `cpu.usage_pct`, `cpu.temp_c`
- `memory.usage_pct`, `memory.used_kb`, `memory.swap_used_kb`
- `gpu.pci-0000:00:02.0.freq_mhz`
- `gpu.pci-0000:01:00.0.usage_pct`, `gpu.pci-0000:01:00.0.vram_used_mb`, `gpu.pci-0000:01:00.0.temp_c`
- `disk.nvme0n1.read_kb`, `disk.nvme0n1.write_kb`, `disk.nvme0n1.io_ticks_ms`
- `fans.hwmon4/fan1.rpm`, `fans.hwmon4/fan2.rpm`
