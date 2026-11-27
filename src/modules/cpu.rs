use std::fs;

use crate::modules::MetricValue;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CpuTimes {
    idle: u64,
    total: u64,
}

pub struct CpuState {
    prev: CpuTimes,
}

impl CpuState {
    pub fn init() -> Result<Self, String> {
        let content = fs::read_to_string("/proc/stat")
            .map_err(|e| format!("failed to read /proc/stat: {e}"))?;
        let prev = parse_proc_stat(&content)?;
        Ok(Self { prev })
    }

    pub fn poll(&mut self, fields: &mut Vec<(&'static str, MetricValue)>) -> Result<(), String> {
        let content = fs::read_to_string("/proc/stat")
            .map_err(|e| format!("failed to read /proc/stat: {e}"))?;
        let current = parse_proc_stat(&content)?;
        let usage_pct = compute_usage(self.prev, current);
        self.prev = current;

        fields.push(("usage_pct", MetricValue::F64(usage_pct)));
        Ok(())
    }
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

    let idle_all = vals[3] + vals[4]; // idle + iowait
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
        assert_eq!(times, CpuTimes { idle: 510, total: 665 });
    }

    #[test]
    fn computes_delta_usage() {
        let prev = CpuTimes { idle: 500, total: 1000 };
        let curr = CpuTimes { idle: 550, total: 1200 };
        // 200 total ticks, 50 idle ticks -> 150 active -> 75.0%
        let pct = compute_usage(prev, curr);
        assert!((pct - 75.0).abs() < 1e-9);
    }

    #[test]
    fn rejects_truncated_proc_stat() {
        assert!(parse_proc_stat("cpu  10 20\n").is_err());
    }
}
