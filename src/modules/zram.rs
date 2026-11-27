use std::fs;
use std::path::Path;

use crate::modules::MetricValue;

pub fn poll(fields: &mut Vec<(&'static str, MetricValue)>) -> Result<(), String> {
    let mut devices = 0u64;
    let mut orig_data_bytes = 0u64;
    let mut compr_data_bytes = 0u64;
    let mut mem_used_bytes = 0u64;
    let mut disksize_bytes = 0u64;

    let entries = fs::read_dir("/sys/block")
        .map_err(|e| format!("failed to read /sys/block: {e}"))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("failed reading /sys/block entry: {e}"))?;
        let name = entry.file_name();
        let Some(name_str) = name.to_str() else {
            continue;
        };
        if !is_zram_device(name_str) {
            continue;
        }

        let dev_path = entry.path();
        let disksize = read_u64_file(&dev_path.join("disksize"))?;
        if disksize == 0 {
            continue;
        }

        devices += 1;
        disksize_bytes += disksize;

        let mm_stat_path = dev_path.join("mm_stat");
        let mm_stat = fs::read_to_string(&mm_stat_path)
            .map_err(|e| format!("failed to read {}: {e}", mm_stat_path.display()))?;
        let (orig, compr, mem) = parse_mm_stat(&mm_stat)?;
        orig_data_bytes += orig;
        compr_data_bytes += compr;
        mem_used_bytes += mem;
    }

    let ratio = if compr_data_bytes == 0 {
        0.0
    } else {
        orig_data_bytes as f64 / compr_data_bytes as f64
    };

    fields.push(("devices", MetricValue::U64(devices)));
    fields.push(("disksize_bytes", MetricValue::U64(disksize_bytes)));
    fields.push(("orig_data_bytes", MetricValue::U64(orig_data_bytes)));
    fields.push(("compr_data_bytes", MetricValue::U64(compr_data_bytes)));
    fields.push(("mem_used_bytes", MetricValue::U64(mem_used_bytes)));
    fields.push(("compression_ratio", MetricValue::F64(ratio)));
    Ok(())
}

fn is_zram_device(name: &str) -> bool {
    name.strip_prefix("zram")
        .is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()))
}

fn read_u64_file(path: &Path) -> Result<u64, String> {
    let raw = fs::read_to_string(path)
        .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    raw.trim()
        .parse::<u64>()
        .map_err(|_| format!("invalid integer in {}", path.display()))
}

/// Parses the first three fields of `/sys/block/zram*/mm_stat`:
/// `orig_data_size compr_data_size mem_used_total ...`
fn parse_mm_stat(content: &str) -> Result<(u64, u64, u64), String> {
    let mut parts = content.split_whitespace();
    let orig = parse_next_u64(&mut parts, "orig_data_size")?;
    let compr = parse_next_u64(&mut parts, "compr_data_size")?;
    let mem = parse_next_u64(&mut parts, "mem_used_total")?;
    Ok((orig, compr, mem))
}

fn parse_next_u64<'a>(
    iter: &mut impl Iterator<Item = &'a str>,
    field: &str,
) -> Result<u64, String> {
    let tok = iter
        .next()
        .ok_or_else(|| format!("missing {field} in zram mm_stat"))?;
    tok.parse::<u64>()
        .map_err(|_| format!("invalid {field} '{tok}' in zram mm_stat"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mm_stat() {
        let sample = "4096000  1024000  1100000 0 1024000 10 0 0 0\n";
        assert_eq!(parse_mm_stat(sample).unwrap(), (4_096_000, 1_024_000, 1_100_000));
    }

    #[test]
    fn rejects_truncated_mm_stat() {
        assert!(parse_mm_stat("4096000 1024000\n").is_err());
    }
}
