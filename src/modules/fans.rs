use std::fs;
use std::path::{Path, PathBuf};

use crate::modules::{MetricValue, read_trimmed, read_u64};

pub struct FansPoller {
    pub id: String,
    input_path: PathBuf,
}

impl FansPoller {
    pub fn init(spec: &str) -> Result<Self, String> {
        let (hwmon, fan) = spec.split_once('/').ok_or_else(|| {
            format!("invalid fan ID '{spec}': expected <hwmonX>/<fanY> (see `adsymon list fans`)")
        })?;
        let input_path = Path::new("/sys/class/hwmon")
            .join(hwmon)
            .join(format!("{fan}_input"));
        if !input_path.exists() {
            return Err(format!(
                "fan sensor '{spec}' not found (run `adsymon list fans`)"
            ));
        }
        Ok(Self {
            id: spec.to_string(),
            input_path,
        })
    }

    pub fn poll(&mut self, fields: &mut Vec<(&'static str, MetricValue)>) -> Result<(), String> {
        let rpm = read_u64(&self.input_path)?;
        fields.push(("rpm", MetricValue::U64(rpm)));
        Ok(())
    }
}

pub fn list() -> Result<(), String> {
    let entries = fs::read_dir("/sys/class/hwmon")
        .map_err(|e| format!("failed to read /sys/class/hwmon: {e}"))?;

    let mut fans = Vec::new();
    for entry in entries.flatten() {
        let hwmon_name = entry.file_name();
        let Some(hwmon) = hwmon_name.to_str() else {
            continue;
        };
        let dir = entry.path();
        let chip = read_trimmed(&dir.join("name")).unwrap_or_else(|| "unknown".into());

        if let Ok(files) = fs::read_dir(&dir) {
            for f in files.flatten() {
                let fname = f.file_name();
                let Some(fname_str) = fname.to_str() else {
                    continue;
                };
                if let Some(fan) = fname_str
                    .strip_suffix("_input")
                    .filter(|s| s.starts_with("fan"))
                {
                    let label = read_trimmed(&dir.join(format!("{fan}_label")));
                    let rpm = read_u64(&f.path()).ok();
                    fans.push((format!("{hwmon}/{fan}"), chip.clone(), label, rpm));
                }
            }
        }
    }

    fans.sort_by(|a, b| a.0.cmp(&b.0));
    if fans.is_empty() {
        println!("No fan sensors found in /sys/class/hwmon");
        return Ok(());
    }
    println!("Detected fans:");
    for (id, chip, label, rpm) in fans {
        let label_str = label.as_deref().unwrap_or("-");
        let rpm_str = rpm
            .map(|r| format!("{r} RPM"))
            .unwrap_or_else(|| "n/a".into());
        println!("  {id:<16} {chip:<14} {label_str:<16} {rpm_str}");
    }
    Ok(())
}
