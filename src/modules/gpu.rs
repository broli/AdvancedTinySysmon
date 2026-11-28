use std::fs;
use std::path::PathBuf;
use std::process::Command;

use crate::modules::{MetricValue, read_hwmon_temp_c, read_trimmed, read_u64};

struct GpuEntry {
    id: String,
    card: String,
    driver: String,
    pci_slot: Option<String>,
    card_dir: PathBuf,
    device_dir: PathBuf,
}

pub struct GpuPoller {
    pub id: String,
    driver: String,
    pci_slot: Option<String>,
    card_dir: PathBuf,
    device_dir: PathBuf,
    usage: bool,
    vram: bool,
    temp: bool,
}

impl GpuPoller {
    pub fn init(spec: &str, usage: bool, vram: bool, temp: bool) -> Result<Self, String> {
        let entries = discover_gpus()?;
        let matched = resolve_gpu(&entries, spec).ok_or_else(|| {
            format!("GPU '{spec}' not found (run `adsymon list gpu` to see available GPUs)")
        })?;

        Ok(Self {
            id: matched.id.clone(),
            driver: matched.driver.clone(),
            pci_slot: matched.pci_slot.clone(),
            card_dir: matched.card_dir.clone(),
            device_dir: matched.device_dir.clone(),
            usage,
            vram,
            temp,
        })
    }

    pub fn poll(&mut self, fields: &mut Vec<(&'static str, MetricValue)>) -> Result<(), String> {
        if self.driver == "nvidia"
            && let Some(slot) = &self.pci_slot
        {
            return poll_nvidia_smi(slot, self.usage, self.vram, self.temp, fields);
        }

        if self.usage {
            if let Ok(pct) = read_u64(&self.device_dir.join("gpu_busy_percent")) {
                fields.push(("usage_pct", MetricValue::U64(pct)));
            } else if let Ok(freq) = read_u64(&self.card_dir.join("gt_act_freq_mhz")) {
                fields.push(("freq_mhz", MetricValue::U64(freq)));
                if let Ok(max_freq) = read_u64(&self.card_dir.join("gt_max_freq_mhz")) {
                    fields.push(("max_freq_mhz", MetricValue::U64(max_freq)));
                    if max_freq > 0 {
                        let freq_pct = (freq as f64 / max_freq as f64) * 100.0;
                        fields.push(("freq_pct", MetricValue::F64(freq_pct)));
                    }
                }
            }
        }

        if self.vram {
            let used_mb = read_u64(&self.device_dir.join("mem_info_vram_used"))
                .ok()
                .map(|b| b / (1024 * 1024));
            let total_mb = read_u64(&self.device_dir.join("mem_info_vram_total"))
                .ok()
                .map(|b| b / (1024 * 1024));
            push_vram_fields(used_mb, total_mb, fields);
        }

        if self.temp
            && let Ok(temp_c) = read_hwmon_temp_c(&self.device_dir.join("hwmon"))
        {
            fields.push(("temp_c", MetricValue::F64(temp_c)));
        }

        if fields.is_empty() {
            return Err(format!("no readable metrics for GPU '{}'", self.id));
        }
        Ok(())
    }
}

fn poll_nvidia_smi(
    pci_slot: &str,
    usage: bool,
    vram: bool,
    temp: bool,
    fields: &mut Vec<(&'static str, MetricValue)>,
) -> Result<(), String> {
    let output = Command::new("nvidia-smi")
        .args([
            "--query-gpu=utilization.gpu,memory.used,memory.total,temperature.gpu",
            "--format=csv,noheader,nounits",
            "-i",
            pci_slot,
        ])
        .output()
        .map_err(|e| format!("failed to run nvidia-smi for '{pci_slot}': {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "nvidia-smi failed for '{pci_slot}': {}",
            stderr.trim()
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stats = parse_nvidia_smi_csv(&stdout)?;

    if usage {
        fields.push(("usage_pct", MetricValue::U64(stats.usage_pct)));
    }
    if vram {
        push_vram_fields(Some(stats.vram_used_mb), Some(stats.vram_total_mb), fields);
    }
    if temp {
        fields.push(("temp_c", MetricValue::F64(stats.temp_c)));
    }
    Ok(())
}

fn push_vram_fields(
    used_mb: Option<u64>,
    total_mb: Option<u64>,
    fields: &mut Vec<(&'static str, MetricValue)>,
) {
    if let Some(used) = used_mb {
        fields.push(("vram_used_mb", MetricValue::U64(used)));
    }
    if let Some(total) = total_mb {
        fields.push(("vram_total_mb", MetricValue::U64(total)));
    }
    if let (Some(used), Some(total)) = (used_mb, total_mb)
        && total > 0
    {
        fields.push((
            "vram_used_pct",
            MetricValue::F64((used as f64 / total as f64) * 100.0),
        ));
    }
}

#[derive(Debug, PartialEq)]
struct NvidiaStats {
    usage_pct: u64,
    vram_used_mb: u64,
    vram_total_mb: u64,
    temp_c: f64,
}

fn parse_nvidia_smi_csv(output: &str) -> Result<NvidiaStats, String> {
    let line = output
        .lines()
        .find(|l| !l.trim().is_empty())
        .ok_or_else(|| "empty nvidia-smi output".to_string())?;
    let mut cols = line.split(',').map(str::trim);
    let mut next_col = || {
        cols.next()
            .ok_or_else(|| format!("unexpected nvidia-smi CSV: '{line}'"))
    };

    let usage_s = next_col()?;
    let used_s = next_col()?;
    let total_s = next_col()?;
    let temp_s = next_col()?;

    Ok(NvidiaStats {
        usage_pct: usage_s
            .parse::<u64>()
            .map_err(|_| format!("invalid nvidia-smi utilization '{usage_s}'"))?,
        vram_used_mb: used_s
            .parse::<u64>()
            .map_err(|_| format!("invalid nvidia-smi memory.used '{used_s}'"))?,
        vram_total_mb: total_s
            .parse::<u64>()
            .map_err(|_| format!("invalid nvidia-smi memory.total '{total_s}'"))?,
        temp_c: temp_s
            .parse::<f64>()
            .map_err(|_| format!("invalid nvidia-smi temperature '{temp_s}'"))?,
    })
}

pub fn list() -> Result<(), String> {
    let gpus = discover_gpus()?;
    if gpus.is_empty() {
        println!("No GPUs found in /sys/class/drm");
        return Ok(());
    }
    println!("Detected GPUs:");
    for gpu in gpus {
        println!(
            "  {:<18} {} ({})  /dev/dri/{}",
            gpu.id,
            gpu.driver,
            gpu.device_dir.display(),
            gpu.card
        );
    }
    Ok(())
}

fn is_drm_card(name: &str) -> bool {
    name.strip_prefix("card")
        .is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()))
}

fn discover_gpus() -> Result<Vec<GpuEntry>, String> {
    let entries = fs::read_dir("/sys/class/drm")
        .map_err(|e| format!("failed to read /sys/class/drm: {e}"))?;

    let mut gpus = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(card) = name.to_str() else {
            continue;
        };
        if !is_drm_card(card) {
            continue;
        }

        let card_dir = entry.path();
        let device_link = card_dir.join("device");
        let Ok(device_dir) = fs::canonicalize(&device_link) else {
            continue;
        };

        let uevent = read_trimmed(&device_dir.join("uevent")).unwrap_or_default();
        let pci_slot = parse_uevent_field(&uevent, "PCI_SLOT_NAME").map(str::to_string);
        let driver = parse_uevent_field(&uevent, "DRIVER").unwrap_or("unknown");
        let id = match &pci_slot {
            Some(slot) => format!("pci-{slot}"),
            None => card.to_string(),
        };

        gpus.push(GpuEntry {
            id,
            card: card.to_string(),
            driver: driver.to_string(),
            pci_slot,
            card_dir,
            device_dir,
        });
    }

    gpus.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(gpus)
}

fn resolve_gpu<'a>(gpus: &'a [GpuEntry], spec: &str) -> Option<&'a GpuEntry> {
    if let Some(found) = gpus
        .iter()
        .find(|g| g.id == spec || g.card == spec || format!("/dev/dri/{}", g.card) == spec)
    {
        return Some(found);
    }

    let target_canon = fs::canonicalize(spec).ok();
    for gpu in gpus {
        let slot = gpu.id.strip_prefix("pci-").unwrap_or(&gpu.id);
        if spec.contains(slot) {
            return Some(gpu);
        }
        if let Some(ref canon) = target_canon {
            let sys_dev = format!(
                "/sys/class/drm/{}/device",
                canon.file_name().and_then(|s| s.to_str()).unwrap_or("")
            );
            if let Ok(resolved) = fs::canonicalize(sys_dev)
                && resolved == gpu.device_dir
            {
                return Some(gpu);
            }
        }
    }

    None
}

fn parse_uevent_field<'a>(uevent: &'a str, key: &str) -> Option<&'a str> {
    uevent
        .lines()
        .filter_map(|line| line.split_once('='))
        .find_map(|(k, v)| (k == key).then_some(v.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_uevent_fields() {
        let sample = "DRIVER=amdgpu\nPCI_SLOT_NAME=0000:01:00.0\n";
        assert_eq!(parse_uevent_field(sample, "DRIVER"), Some("amdgpu"));
        assert_eq!(
            parse_uevent_field(sample, "PCI_SLOT_NAME"),
            Some("0000:01:00.0")
        );
    }

    #[test]
    fn filters_drm_card_names() {
        assert!(is_drm_card("card0"));
        assert!(is_drm_card("card12"));
        assert!(!is_drm_card("card0-DP-1"));
        assert!(!is_drm_card("renderD128"));
    }

    #[test]
    fn parses_nvidia_smi_output() {
        let stats = parse_nvidia_smi_csv("29, 5536, 8188, 55.11\n").unwrap();
        assert_eq!(
            stats,
            NvidiaStats {
                usage_pct: 29,
                vram_used_mb: 5536,
                vram_total_mb: 8188,
                temp_c: 55.11,
            }
        );
    }
}
