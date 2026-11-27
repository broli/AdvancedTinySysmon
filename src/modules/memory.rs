use std::fs;

use crate::modules::MetricValue;

pub fn poll(fields: &mut Vec<(&'static str, MetricValue)>) -> Result<(), String> {
    let content = fs::read_to_string("/proc/meminfo")
        .map_err(|e| format!("failed to read /proc/meminfo: {e}"))?;
    let stats = parse_meminfo(&content)?;

    let used_kb = stats.total_kb.saturating_sub(stats.available_kb);
    let usage_pct = if stats.total_kb == 0 {
        0.0
    } else {
        (used_kb as f64 / stats.total_kb as f64) * 100.0
    };
    let swap_used_kb = stats.swap_total_kb.saturating_sub(stats.swap_free_kb);

    fields.push(("total_kb", MetricValue::U64(stats.total_kb)));
    fields.push(("available_kb", MetricValue::U64(stats.available_kb)));
    fields.push(("used_kb", MetricValue::U64(used_kb)));
    fields.push(("usage_pct", MetricValue::F64(usage_pct)));
    fields.push(("swap_total_kb", MetricValue::U64(stats.swap_total_kb)));
    fields.push(("swap_used_kb", MetricValue::U64(swap_used_kb)));
    Ok(())
}

#[derive(Default)]
struct MemInfo {
    total_kb: u64,
    available_kb: u64,
    swap_total_kb: u64,
    swap_free_kb: u64,
}

fn parse_meminfo(content: &str) -> Result<MemInfo, String> {
    let mut info = MemInfo::default();
    let mut saw_total = false;

    for line in content.lines() {
        let Some((key, rest)) = line.split_once(':') else {
            continue;
        };
        let target = match key {
            "MemTotal" => {
                saw_total = true;
                &mut info.total_kb
            }
            "MemAvailable" => &mut info.available_kb,
            "SwapTotal" => &mut info.swap_total_kb,
            "SwapFree" => &mut info.swap_free_kb,
            _ => continue,
        };

        let num_str = rest
            .split_whitespace()
            .next()
            .ok_or_else(|| format!("missing value for {key} in /proc/meminfo"))?;
        *target = num_str
            .parse::<u64>()
            .map_err(|_| format!("invalid integer '{num_str}' for {key} in /proc/meminfo"))?;
    }

    if !saw_total {
        return Err("missing MemTotal in /proc/meminfo".to_string());
    }
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_meminfo_fields() {
        let sample = "\
MemTotal:       16384000 kB
MemFree:         2000000 kB
MemAvailable:    8192000 kB
SwapTotal:       4096000 kB
SwapFree:        3096000 kB
";
        let info = parse_meminfo(sample).unwrap();
        assert_eq!(info.total_kb, 16_384_000);
        assert_eq!(info.available_kb, 8_192_000);
        assert_eq!(info.swap_total_kb, 4_096_000);
        assert_eq!(info.swap_free_kb, 3_096_000);
    }

    #[test]
    fn rejects_missing_memtotal() {
        assert!(parse_meminfo("MemFree: 100 kB\n").is_err());
    }
}
