default:
    @just --list

# Run unit tests
test:
    cargo test

# Format, clippy, and test
check:
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    cargo test

# Format code
fmt:
    cargo fmt

# Build release binary
build:
    cargo build --release

# List categories or discovered devices (e.g. `just list gpu`)
list *args:
    cargo run --quiet -- list {{args}}

# Run a single poll (e.g. `just once --cpu --usage --memory`)
once *args:
    cargo run --quiet -- --once {{args}}

# Run release binary with arguments
run *args:
    cargo run --release -- {{args}}

monitor_args := "--cpu --usage --temp --memory --usage --gpu pci-0000:00:02.0 --usage --mem --temp --gpu pci-0000:01:00.0 --usage --mem --temp --disk nvme0n1 --usage --fill --temp --fans hwmon4/fan1 --rpm --fans hwmon4/fan2 --rpm"

# Run release binary with full system monitor profile
monitor: build
    ./target/release/adsymon {{monitor_args}}

# Same as `monitor`, but print JSON to stdout
monitor-stdout: build
    ./target/release/adsymon --stdout {{monitor_args}}

# Watch status.json in RAM
watch:
    watch -n 0.5 cat "${XDG_RUNTIME_DIR:-/dev/shm}/adsymon/status.json"

# Install adsymon into ~/.cargo/bin
install:
    cargo install --path .
