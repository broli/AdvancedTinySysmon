use std::fmt::{self, Write as _};
use std::fs;
use std::io;
use std::path::Path;

use crate::modules::{MetricValue, Reading};

pub fn write_json(out: &mut String, timestamp_ms: u128, readings: &[Reading]) {
    out.clear();
    try_write_json(out, timestamp_ms, readings).expect("writing to String is infallible");
}

fn try_write_json(out: &mut String, timestamp_ms: u128, readings: &[Reading]) -> fmt::Result {
    write!(out, "{{\"timestamp_ms\":{timestamp_ms}")?;

    let mut current_cat: Option<&'static str> = None;
    let mut first_dev = true;

    for reading in readings {
        if reading.fields.is_empty() {
            continue;
        }
        if current_cat != Some(reading.category) {
            if current_cat.is_some() {
                out.push('}');
            }
            write!(out, ",\"{}\":{{", reading.category)?;
            current_cat = Some(reading.category);
            first_dev = true;
        }

        if let Some(dev_id) = &reading.device_id {
            if !first_dev {
                out.push(',');
            }
            first_dev = false;
            write!(out, "\"{dev_id}\":{{")?;
            write_fields(out, &reading.fields)?;
            out.push('}');
        } else {
            write_fields(out, &reading.fields)?;
        }
    }

    if current_cat.is_some() {
        out.push('}');
    }
    out.push_str("}\n");
    Ok(())
}

fn write_fields(out: &mut String, fields: &[(&'static str, MetricValue)]) -> fmt::Result {
    for (idx, (key, val)) in fields.iter().enumerate() {
        if idx > 0 {
            out.push(',');
        }
        match val {
            MetricValue::U64(v) => write!(out, "\"{key}\":{v}")?,
            MetricValue::F64(v) => write!(out, "\"{key}\":{v:.2}")?,
        }
    }
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
    fn formats_flat_and_keyed_categories() {
        let readings = vec![
            Reading {
                category: "cpu",
                device_id: None,
                fields: vec![("usage_pct", MetricValue::F64(12.345))],
            },
            Reading {
                category: "gpu",
                device_id: Some("pci-0000:01:00.0".into()),
                fields: vec![("vram_used_mb", MetricValue::U64(4096))],
            },
            Reading {
                category: "gpu",
                device_id: Some("pci-0000:00:02.0".into()),
                fields: vec![("vram_used_mb", MetricValue::U64(512))],
            },
        ];
        let mut buf = String::new();
        write_json(&mut buf, 1234, &readings);
        assert_eq!(
            buf,
            "{\"timestamp_ms\":1234,\"cpu\":{\"usage_pct\":12.35},\"gpu\":{\"pci-0000:01:00.0\":{\"vram_used_mb\":4096},\"pci-0000:00:02.0\":{\"vram_used_mb\":512}}}\n"
        );
    }

    #[test]
    fn write_atomic_replaces_file_without_leaving_tmp() {
        let dir = std::env::temp_dir().join(format!("adsymon_atomic_test_{}", std::process::id()));
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
