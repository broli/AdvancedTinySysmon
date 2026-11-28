use std::fs;
use std::path::{Path, PathBuf};

use crate::modules::{MetricValue, read_trimmed, read_u64};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CpuTimes {
    idle: u64,
    total: u64,
}

pub struct CpuPoller {
    usage: bool,
    temp: bool,
    prev: Option<CpuTimes>,
    temp_path: Option<PathBuf>,
}

impl CpuPoller {
    pub fn init(usage: bool, temp: bool) -> Result<Self, String> {
        let prev = if usage {
            let content = fs::read_to_string("/proc/stat")
                .map_err(|e| format!("failed to read /proc/stat: {e}"))?;
            Some(parse_proc_stat(&content)?)
        } else {
            None
        };

        let temp_path = if temp {
            Some(find_cpu_temp_sensor()?)
        } else {
            None
        };

        Ok(Self {
            usage,
            temp,
            prev,
            temp_path,
        })
    }

    pub fn poll(&mut self, fields: &mut Vec<(&'static str, MetricValue)>) -> Result<(), String> {
        if self.usage {
            let content = fs::read_to_string("/proc/stat")
                .map_err(|e| format!("failed to read /proc/stat: {e}"))?;
            let current = parse_proc_stat(&content)?;
            let prev = self.prev.replace(current).unwrap_or(current);
            fields.push(("usage_pct", MetricValue::F64(compute_usage(prev, current))));
        }

        if self.temp
            && let Some(path) = &self.temp_path
        {
            let milli = read_u64(path)?;
            fields.push(("temp_c", MetricValue::F64(milli as f64 / 1000.0)));
        }

        Ok(())
    }
}

fn find_cpu_temp_sensor() -> Result<PathBuf, String> {
    let entries = fs::read_dir("/sys/class/hwmon")
        .map_err(|e| format!("failed to read /sys/class/hwmon: {e}"))?;

    let mut fallback = None;
    for entry in entries.flatten() {
        let dir = entry.path();
        let name = read_trimmed(&dir.join("name")).unwrap_or_default();
        let temp1 = dir.join("temp1_input");
        if !temp1.exists() {
            continue;
        }
        if matches!(name.as_str(), "k10temp" | "coretemp" | "zenpower") {
            return Ok(temp1);
        }
        if fallback.is_none() && name == "acpitz" {
            fallback = Some(temp1);
        }
    }

    fallback.ok_or_else(|| {
        format!(
            "no CPU temperature sensor found in {}",
            Path::new("/sys/class/hwmon").display()
        )
    })
}

fn compute_usage(prev: CpuTimes, current: CpuTimes) -> f64 {
    let total_delta = current.total.saturating_sub(prev.total);
    if total_delta == 0 {
        return 0.0;
    }
    let idle_delta = current.idle.saturating_sub(prev.idle);
    ((total_delta.saturating_sub(idle_delta)) as f64 / total_delta as f64) * 100.0
}

fn parse_proc_stat(content: &str) -> Result<CpuTimes, String> {
    let first_line = content
        .lines()
        .find(|line| line.starts_with("cpu "))
        .ok_or_else(|| "missing aggregate 'cpu ' line in /proc/stat".to_string())?;

    let mut cols = first_line.split_whitespace().skip(1);
    let mut vals = [0u64; 8];
    let mut count = 0;

    for slot in &mut vals {
        let Some(token) = cols.next() else {
            break;
        };
        *slot = token
            .parse::<u64>()
            .map_err(|_| format!("invalid integer '{token}' in /proc/stat"))?;
        count += 1;
    }

    if count < 4 {
        return Err("truncated 'cpu ' line in /proc/stat".to_string());
    }

    let idle_all = vals[3] + vals[4];
    let total: u64 = vals.iter().sum();

    Ok(CpuTimes {
        idle: idle_all,
        total,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_proc_stat_line() {
        let sample = "cpu  100 20 30 500 10 0 5 0 0 0\ncpu0 50 10 15 250 5 0 2 0 0 0\n";
        let times = parse_proc_stat(sample).unwrap();
        assert_eq!(
            times,
            CpuTimes {
                idle: 510,
                total: 665
            }
        );
    }

    #[test]
    fn computes_delta_usage() {
        let prev = CpuTimes {
            idle: 500,
            total: 1000,
        };
        let curr = CpuTimes {
            idle: 550,
            total: 1200,
        };
        assert!((compute_usage(prev, curr) - 75.0).abs() < 1e-9);
    }

    #[test]
    fn rejects_truncated_proc_stat() {
        assert!(parse_proc_stat("cpu  10 20\n").is_err());
    }
}
