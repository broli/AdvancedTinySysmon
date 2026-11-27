# Diary 001 — Initial Concept & CLI Vision

**Date:** 2026-11-27  
**Status:** Brain dump / Draft

## Core Idea

`adsymon` is a simple, fast, lightweight Rust daemon.

1. At startup, it parses and validates all CLI parameters and discovers the requested hardware/sensor targets.
2. It enters a main loop and stays there indefinitely, polling the configured targets and exposing their status (in RAM/`tmpfs`) for external frontends (Waybar, Noctalia, etc.).

## CLI & Parameter Model

- **Repeatable, ordered/scoped parameters:**  
  Users can repeat category flags to monitor multiple devices and attach subcategory flags to each:
  ```bash
  adsymon \
    --gpu /dev/dri/by-path/pci-0000:01:00.0-render --mem --temp \
    --gpu /dev/dri/by-path/pci-0000:00:02.0-render --mem
  ```
- **Top-level help:**  
  Basic usage and overview of categories.
- **Per-category (level-1) help:**  
  Each category has its own help subcommand, e.g.:
  ```bash
  adsymon cpu help
  adsymon gpu help
  ```
- **Hardware / module discovery (`list`):**
  - `adsymon list` — lists all available monitoring modules/categories.
  - `adsymon list gpu` — lists all detected GPU devices on the system (and likewise for `disk`, `fans`, etc.).

## Sensor Categories & Subcategories

Initial sensor categories:
- `cpu`
- `memory`
- `gpu`
- `disk`
- `fans`

Common subcategories expected across categories:
- `device` (which hardware instance to poll)
- `% usage`
- `temperature`

## Open Questions

- **Where does `vram` belong?**
  1. Inside `gpu` (e.g., `--gpu <dev> --mem` / `vram` as a GPU metric, since VRAM is physically tied to each GPU device)?
  2. Inside `memory` (grouped with system RAM and swap)?
  3. As its own top-level category?
