pub mod cpu;
pub mod disk;
pub mod fans;
pub mod gpu;
pub mod memory;

use std::fs;
use std::path::Path;

use crate::cli::Target;

const AVERAGE_WINDOW: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MetricValue {
    U64(u64),
    F64(f64),
}

#[derive(Clone, Copy)]
struct Ring<T: Copy, const N: usize> {
    buf: [T; N],
    len: usize,
    next: usize,
}

impl<T: Copy, const N: usize> Ring<T, N> {
    fn new(first: T) -> Self {
        Self {
            buf: [first; N],
            len: 1,
            next: 1 % N,
        }
    }

    fn push(&mut self, val: T) -> &[T] {
        self.buf[self.next] = val;
        self.next = (self.next + 1) % N;
        if self.len < N {
            self.len += 1;
        }
        &self.buf[..self.len]
    }
}

#[derive(Clone, Copy)]
enum SampleRing {
    U64(Ring<u64, AVERAGE_WINDOW>),
    F64(Ring<f64, AVERAGE_WINDOW>),
}

impl SampleRing {
    fn new(initial: MetricValue) -> Self {
        match initial {
            MetricValue::U64(v) => Self::U64(Ring::new(v)),
            MetricValue::F64(v) => Self::F64(Ring::new(v)),
        }
    }

    fn push_and_average(&mut self, val: MetricValue) -> MetricValue {
        match (self, val) {
            (Self::U64(ring), MetricValue::U64(v)) => {
                let s = ring.push(v);
                let avg = s.iter().map(|&x| x as f64).sum::<f64>() / s.len() as f64;
                MetricValue::U64(avg.round() as u64)
            }
            (Self::F64(ring), MetricValue::F64(v)) => {
                let s = ring.push(v);
                MetricValue::F64(s.iter().sum::<f64>() / s.len() as f64)
            }
            (ring, new_val) => {
                *ring = Self::new(new_val);
                new_val
            }
        }
    }
}

#[derive(Default)]
pub struct FieldHistory {
    entries: Vec<(&'static str, SampleRing)>,
}

impl FieldHistory {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
        }
    }

    pub fn average(&mut self, fields: &mut [(&'static str, MetricValue)]) {
        for (key, val) in fields.iter_mut() {
            // Static totals and monotonic counters should reflect the latest reading verbatim.
            if matches!(
                *key,
                "total_kb"
                    | "swap_total_kb"
                    | "vram_total_mb"
                    | "max_freq_mhz"
                    | "read_kb"
                    | "write_kb"
                    | "io_ticks_ms"
                    | "avail_kb"
                    | "fill_pct"
            ) {
                continue;
            }
            if let Some((_, ring)) = self.entries.iter_mut().find(|(k, _)| k == key) {
                *val = ring.push_and_average(*val);
            } else {
                self.entries.push((*key, SampleRing::new(*val)));
            }
        }
    }
}

pub struct Reading {
    pub category: &'static str,
    pub device_id: Option<String>,
    pub fields: Vec<(&'static str, MetricValue)>,
}

pub enum Poller {
    Cpu(cpu::CpuPoller),
    Memory,
    Gpu(gpu::GpuPoller),
    Disk(disk::DiskPoller),
    Fans(fans::FansPoller),
}

impl Poller {
    pub fn from_target(target: &Target) -> Result<Self, String> {
        match target {
            Target::Cpu { usage, temp } => Ok(Self::Cpu(cpu::CpuPoller::init(*usage, *temp)?)),
            Target::Memory { .. } => Ok(Self::Memory),
            Target::Gpu {
                device,
                usage,
                vram,
                temp,
            } => Ok(Self::Gpu(gpu::GpuPoller::init(
                device, *usage, *vram, *temp,
            )?)),
            Target::Disk {
                device,
                usage,
                fill,
                temp,
            } => Ok(Self::Disk(disk::DiskPoller::init(
                device, *usage, *fill, *temp,
            )?)),
            Target::Fans { device, .. } => Ok(Self::Fans(fans::FansPoller::init(device)?)),
        }
    }

    pub fn category(&self) -> &'static str {
        match self {
            Self::Cpu(_) => "cpu",
            Self::Memory => "memory",
            Self::Gpu(_) => "gpu",
            Self::Disk(_) => "disk",
            Self::Fans(_) => "fans",
        }
    }

    pub fn device_id(&self) -> Option<&str> {
        match self {
            Self::Cpu(_) | Self::Memory => None,
            Self::Gpu(g) => Some(&g.id),
            Self::Disk(d) => Some(&d.id),
            Self::Fans(f) => Some(&f.id),
        }
    }

    pub fn key(&self) -> (&'static str, Option<&str>) {
        (self.category(), self.device_id())
    }

    pub fn poll(&mut self, fields: &mut Vec<(&'static str, MetricValue)>) -> Result<(), String> {
        fields.clear();
        match self {
            Self::Cpu(p) => p.poll(fields),
            Self::Memory => memory::poll(fields),
            Self::Gpu(p) => p.poll(fields),
            Self::Disk(p) => p.poll(fields),
            Self::Fans(p) => p.poll(fields),
        }
    }
}

pub fn list_devices(category: Option<&str>) -> Result<(), String> {
    match category {
        None => {
            println!("Available categories:");
            println!("  cpu        Aggregate CPU usage and package temperature");
            println!("  memory     System RAM and swap usage");
            println!("  gpu        DRM/PCI GPU usage, VRAM, and temperature");
            println!("  disk       Block device I/O stats and temperature");
            println!("  fans       hwmon fan speed sensors (RPM)");
            Ok(())
        }
        Some("cpu") => {
            println!("cpu (aggregate system CPU; no device argument needed)");
            Ok(())
        }
        Some("memory" | "mem") => {
            println!("memory (system RAM and swap; no device argument needed)");
            Ok(())
        }
        Some("gpu") => gpu::list(),
        Some("disk") => disk::list(),
        Some("fans") => fans::list(),
        Some(other) => Err(format!("unknown category '{other}'")),
    }
}

pub fn read_trimmed(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

pub fn read_u64(path: &Path) -> Result<u64, String> {
    let raw =
        fs::read_to_string(path).map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    raw.trim()
        .parse::<u64>()
        .map_err(|_| format!("invalid integer in {}", path.display()))
}

pub fn read_hwmon_temp_c(hwmon_dir: &Path) -> Result<f64, String> {
    let entries = fs::read_dir(hwmon_dir)
        .map_err(|e| format!("failed to read {}: {e}", hwmon_dir.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if let Ok(sub) = fs::read_dir(&path) {
            for item in sub.flatten() {
                let name = item.file_name();
                let Some(name_str) = name.to_str() else {
                    continue;
                };
                if name_str.starts_with("temp") && name_str.ends_with("_input") {
                    let milli = read_u64(&item.path())?;
                    return Ok(milli as f64 / 1000.0);
                }
            }
        }
    }
    Err(format!(
        "no temp*_input found under {}",
        hwmon_dir.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn averages_last_three_f64_samples() {
        let mut history = FieldHistory::with_capacity(2);
        let samples = [0.0, 100.0, 20.0, 90.0];
        let mut out = Vec::new();

        for val in samples {
            let mut fields = vec![("usage_pct", MetricValue::F64(val))];
            history.average(&mut fields);
            let MetricValue::F64(avg) = fields[0].1 else {
                panic!("expected F64");
            };
            out.push(avg);
        }

        // [0] -> 0, [0, 100] -> 50, [0, 100, 20] -> 40, [100, 20, 90] -> 70
        let expected = [0.0, 50.0, 40.0, 70.0];
        for (got, exp) in out.iter().zip(expected.iter()) {
            assert!((got - exp).abs() < 1e-9, "got {got}, expected {exp}");
        }
    }

    #[test]
    fn averages_last_three_u64_samples_with_rounding() {
        let mut history = FieldHistory::with_capacity(2);
        let samples = [0u64, 100, 0, 50];
        let mut out = Vec::new();

        for val in samples {
            let mut fields = vec![("usage_pct", MetricValue::U64(val))];
            history.average(&mut fields);
            let MetricValue::U64(avg) = fields[0].1 else {
                panic!("expected U64");
            };
            out.push(avg);
        }

        // [0] -> 0, [0, 100] -> 50, [0, 100, 0] -> 33, [100, 0, 50] -> 50
        assert_eq!(out, vec![0, 50, 33, 50]);
    }
}
