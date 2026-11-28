use std::ffi::CString;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::modules::{MetricValue, read_hwmon_temp_c, read_trimmed, read_u64};

pub struct DiskPoller {
    pub id: String,
    block_dir: Option<PathBuf>,
    mount_path: Option<PathBuf>,
    usage: bool,
    fill: bool,
    temp: bool,
    prev: Option<(u64, Instant)>,
}

impl DiskPoller {
    pub fn init(spec: &str, usage: bool, fill: bool, temp: bool) -> Result<Self, String> {
        if spec.starts_with('/') && !spec.starts_with("/dev/") {
            let mount = PathBuf::from(spec);
            if !mount.exists() {
                return Err(format!("mount path '{spec}' not found"));
            }
            return Ok(Self {
                id: spec.to_string(),
                block_dir: None,
                mount_path: Some(mount),
                usage: false,
                fill: fill || usage,
                temp: false,
                prev: None,
            });
        }

        let name = spec.strip_prefix("/dev/").unwrap_or(spec);
        let block_dir = PathBuf::from("/sys/block").join(name);
        if !block_dir.exists() {
            return Err(format!(
                "disk '{spec}' not found (run `adsymon list disk` to see available disks)"
            ));
        }

        let prev = if usage {
            let stat = fs::read_to_string(block_dir.join("stat"))
                .map_err(|e| format!("failed to read stat for '{name}': {e}"))?;
            let (_, _, io_ticks_ms) = parse_block_stat(&stat)?;
            Some((io_ticks_ms, Instant::now()))
        } else {
            None
        };

        let mount_path = if fill {
            fs::read_to_string("/proc/mounts")
                .ok()
                .and_then(|mounts| find_mount_for_device(&mounts, name))
        } else {
            None
        };

        Ok(Self {
            id: name.to_string(),
            block_dir: Some(block_dir),
            mount_path,
            usage,
            fill,
            temp,
            prev,
        })
    }

    pub fn poll(&mut self, fields: &mut Vec<(&'static str, MetricValue)>) -> Result<(), String> {
        if self.usage
            && let Some(block_dir) = &self.block_dir
        {
            let stat = fs::read_to_string(block_dir.join("stat"))
                .map_err(|e| format!("failed to read stat for '{}': {e}", self.id))?;
            let (read_sectors, write_sectors, io_ticks_ms) = parse_block_stat(&stat)?;
            let now = Instant::now();
            let usage_pct =
                if let Some((prev_ticks, prev_at)) = self.prev.replace((io_ticks_ms, now)) {
                    let elapsed_ms = now.duration_since(prev_at).as_secs_f64() * 1000.0;
                    if elapsed_ms > 0.0 {
                        let delta_ticks = io_ticks_ms.saturating_sub(prev_ticks) as f64;
                        ((delta_ticks / elapsed_ms) * 100.0).clamp(0.0, 100.0)
                    } else {
                        0.0
                    }
                } else {
                    0.0
                };
            fields.push(("usage_pct", MetricValue::F64(usage_pct)));
            fields.push(("read_kb", MetricValue::U64(read_sectors / 2)));
            fields.push(("write_kb", MetricValue::U64(write_sectors / 2)));
            fields.push(("io_ticks_ms", MetricValue::U64(io_ticks_ms)));
        }

        if self.fill
            && let Some(mount) = &self.mount_path
            && let Ok(fs_stats) = read_fs_space(mount)
        {
            fields.push(("fill_pct", MetricValue::F64(fs_stats.fill_pct)));
            fields.push(("used_kb", MetricValue::U64(fs_stats.used_kb)));
            fields.push(("avail_kb", MetricValue::U64(fs_stats.avail_kb)));
            fields.push(("total_kb", MetricValue::U64(fs_stats.total_kb)));
        }

        if self.temp
            && let Some(block_dir) = &self.block_dir
            && let Ok(temp_c) = read_hwmon_temp_c(&block_dir.join("device/hwmon"))
        {
            fields.push(("temp_c", MetricValue::F64(temp_c)));
        }

        if fields.is_empty() {
            return Err(format!("no readable metrics for disk '{}'", self.id));
        }
        Ok(())
    }
}

pub fn list() -> Result<(), String> {
    let entries =
        fs::read_dir("/sys/block").map_err(|e| format!("failed to read /sys/block: {e}"))?;

    let mut disks = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name_str) = name.to_str() else {
            continue;
        };
        if name_str.starts_with("loop") || name_str.starts_with("ram") {
            continue;
        }
        let path = entry.path();
        let sectors = read_u64(&path.join("size")).unwrap_or(0);
        if sectors == 0 {
            continue;
        }
        let size_gb = (sectors * 512) as f64 / 1_000_000_000.0;
        let model = read_trimmed(&path.join("device/model")).unwrap_or_default();
        disks.push((name_str.to_string(), size_gb, model));
    }

    disks.sort_by(|a, b| a.0.cmp(&b.0));
    if disks.is_empty() {
        println!("No block devices found in /sys/block");
        return Ok(());
    }
    println!("Detected disks:");
    for (name, gb, model) in disks {
        if model.is_empty() {
            println!("  {name:<12} {gb:>7.1} GB");
        } else {
            println!("  {name:<12} {gb:>7.1} GB  ({model})");
        }
    }
    Ok(())
}

fn parse_block_stat(content: &str) -> Result<(u64, u64, u64), String> {
    let mut cols = [0u64; 10];
    let mut count = 0;
    for (slot, tok) in cols.iter_mut().zip(content.split_whitespace()) {
        *slot = tok
            .parse::<u64>()
            .map_err(|_| "invalid integer in /sys/block/*/stat".to_string())?;
        count += 1;
    }
    if count < 10 {
        return Err("truncated /sys/block/*/stat".to_string());
    }
    Ok((cols[2], cols[6], cols[9]))
}

struct FsSpace {
    fill_pct: f64,
    used_kb: u64,
    avail_kb: u64,
    total_kb: u64,
}

fn find_mount_for_device(mounts: &str, dev_name: &str) -> Option<PathBuf> {
    let prefix = format!("/dev/{dev_name}");
    let mut first_match: Option<PathBuf> = None;

    for line in mounts.lines() {
        let mut parts = line.split_whitespace();
        let Some(source) = parts.next() else {
            continue;
        };
        let Some(target) = parts.next() else {
            continue;
        };
        if source == prefix
            || source.strip_prefix(&prefix).is_some_and(|rest| {
                rest.starts_with('p') || rest.chars().next().is_some_and(|c| c.is_ascii_digit())
            })
        {
            let target_decoded = target.replace("\\040", " ");
            if target_decoded == "/" {
                return Some(PathBuf::from("/"));
            }
            if first_match.is_none() {
                first_match = Some(PathBuf::from(target_decoded));
            }
        }
    }

    first_match
}

#[cfg(all(target_os = "linux", target_pointer_width = "64"))]
#[repr(C)]
struct Statvfs {
    f_bsize: u64,
    f_frsize: u64,
    f_blocks: u64,
    f_bfree: u64,
    f_bavail: u64,
    f_files: u64,
    f_ffree: u64,
    f_favail: u64,
    f_fsid: u64,
    f_flag: u64,
    f_namemax: u64,
    __f_spare: [i32; 6],
}

#[cfg(all(target_os = "linux", target_pointer_width = "64"))]
unsafe extern "C" {
    fn statvfs(path: *const std::ffi::c_char, buf: *mut Statvfs) -> i32;
}

#[cfg(all(target_os = "linux", target_pointer_width = "64"))]
fn read_fs_space(mount: &Path) -> Result<FsSpace, String> {
    let c_path = CString::new(mount.as_os_str().as_bytes())
        .map_err(|_| format!("invalid mount path '{}'", mount.display()))?;
    let mut buf = std::mem::MaybeUninit::<Statvfs>::zeroed();
    let rc = unsafe { statvfs(c_path.as_ptr(), buf.as_mut_ptr()) };
    if rc != 0 {
        return Err(format!("statvfs failed for '{}'", mount.display()));
    }
    let st = unsafe { buf.assume_init() };
    let unit = if st.f_frsize > 0 {
        st.f_frsize
    } else {
        st.f_bsize
    };
    let total_bytes = st.f_blocks.saturating_mul(unit);
    let free_bytes = st.f_bfree.saturating_mul(unit);
    let avail_bytes = st.f_bavail.saturating_mul(unit);
    let used_bytes = total_bytes.saturating_sub(free_bytes);
    let non_root_total = used_bytes.saturating_add(avail_bytes);
    let fill_pct = if non_root_total > 0 {
        (used_bytes as f64 / non_root_total as f64) * 100.0
    } else {
        0.0
    };

    Ok(FsSpace {
        fill_pct,
        used_kb: used_bytes / 1024,
        avail_kb: avail_bytes / 1024,
        total_kb: total_bytes / 1024,
    })
}

#[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
fn read_fs_space(_mount: &Path) -> Result<FsSpace, String> {
    Err("filesystem space stats are only supported on 64-bit Linux".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_block_stat_fields() {
        let sample = "  100  0  2048  50  200  0  4096  80  0  123  130  0  0  0  0\n";
        assert_eq!(parse_block_stat(sample).unwrap(), (2048, 4096, 123));
    }

    #[test]
    fn rejects_truncated_block_stat() {
        assert!(parse_block_stat("1 2 3\n").is_err());
    }

    #[test]
    fn finds_root_mount_for_block_device() {
        let mounts = "/dev/nvme0n1p4 /boot vfat rw 0 0\n/dev/nvme0n1p5 / btrfs rw 0 0\n";
        assert_eq!(
            find_mount_for_device(mounts, "nvme0n1"),
            Some(PathBuf::from("/"))
        );
    }

    #[test]
    fn reads_root_fs_space() {
        let fs = read_fs_space(Path::new("/")).unwrap();
        assert!(fs.total_kb > 0);
        assert!(fs.fill_pct >= 0.0 && fs.fill_pct <= 100.0);
    }
}
