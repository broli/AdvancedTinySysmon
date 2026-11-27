use std::fmt::{self, Write as _};
use std::fs;
use std::io;
use std::path::Path;

use crate::modules::{MetricValue, ModuleReading};

pub fn write_json(out: &mut String, timestamp_ms: u128, readings: &[ModuleReading]) {
    out.clear();
    let _ = try_write_json(out, timestamp_ms, readings);
}

fn try_write_json(
    out: &mut String,
    timestamp_ms: u128,
    readings: &[ModuleReading],
) -> fmt::Result {
    write!(out, "{{\"timestamp_ms\":{timestamp_ms},\"modules\":{{")?;

    let mut wrote_mod = false;
    for reading in readings {
        if reading.fields.is_empty() {
            continue;
        }
        if wrote_mod {
            out.push(',');
        }
        wrote_mod = true;
        write!(out, "\"{}\":{{", reading.name)?;
        for (f_idx, (key, val)) in reading.fields.iter().enumerate() {
            if f_idx > 0 {
                out.push(',');
            }
            match val {
                MetricValue::U64(v) => write!(out, "\"{key}\":{v}")?,
                MetricValue::F64(v) => write!(out, "\"{key}\":{v:.2}")?,
            }
        }
        out.push('}');
    }

    out.push_str("}}\n");
    Ok(())
}

pub fn write_atomic(dir: &Path, filename: &str, content: &str) -> io::Result<()> {
    let tmp_path = dir.join(format!(".{filename}.tmp"));
    let final_path = dir.join(filename);
    fs::write(&tmp_path, content.as_bytes())?;
    fs::rename(&tmp_path, &final_path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_json_and_skips_empty_readings() {
        let readings = vec![
            ModuleReading {
                name: "failed",
                fields: vec![],
            },
            ModuleReading {
                name: "cpu",
                fields: vec![
                    ("usage_pct", MetricValue::F64(12.345)),
                    ("cores", MetricValue::U64(8)),
                ],
            },
        ];
        let mut buf = String::new();
        write_json(&mut buf, 1234, &readings);
        assert_eq!(
            buf,
            "{\"timestamp_ms\":1234,\"modules\":{\"cpu\":{\"usage_pct\":12.35,\"cores\":8}}}\n"
        );
    }

    #[test]
    fn write_atomic_replaces_file_without_leaving_tmp() {
        let dir = std::env::temp_dir().join(format!("atsm_atomic_test_{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();

        write_atomic(&dir, "status.json", "{\"a\":1}\n").unwrap();
        write_atomic(&dir, "status.json", "{\"a\":2}\n").unwrap();

        assert_eq!(
            fs::read_to_string(dir.join("status.json")).unwrap(),
            "{\"a\":2}\n"
        );
        assert!(!dir.join(".status.json.tmp").exists());

        fs::remove_dir_all(&dir).unwrap();
    }
}
