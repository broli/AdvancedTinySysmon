use std::env;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Help { category: Option<String> },
    Version,
    List { category: Option<String> },
    Run(DaemonConfig),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonConfig {
    pub interval_ms: u64,
    pub output_dir: PathBuf,
    pub once: bool,
    pub stdout: bool,
    pub targets: Vec<Target>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Cpu {
        usage: bool,
        temp: bool,
    },
    Memory {
        usage: bool,
    },
    Gpu {
        device: String,
        usage: bool,
        vram: bool,
        temp: bool,
    },
    Disk {
        device: String,
        usage: bool,
        fill: bool,
        temp: bool,
    },
    Fans {
        device: String,
        rpm: bool,
    },
}

fn default_output_dir() -> PathBuf {
    if let Ok(runtime_dir) = env::var("XDG_RUNTIME_DIR")
        && !runtime_dir.is_empty()
    {
        return PathBuf::from(runtime_dir).join("adsymon");
    }
    PathBuf::from("/dev/shm/adsymon")
}

pub fn print_help(category: Option<&str>) -> Result<(), String> {
    match category {
        None => {
            println!(
                "\
adsymon {}

USAGE:
    adsymon [COMMAND]
    adsymon [GLOBAL OPTIONS] [TARGET...]

COMMANDS:
    help                 Print this help
    <category> help      Print help for a category (cpu, memory, gpu, disk, fans)
    list [category]      List categories or discovered hardware targets

GLOBAL OPTIONS:
    -i, --interval <MS>      Poll interval in ms [default: 1000]
    -o, --output-dir <DIR>   Output directory [default: $XDG_RUNTIME_DIR/adsymon or /dev/shm/adsymon]
        --once               Poll once and exit
        --stdout             Print JSON to stdout instead of writing status.json (alias: --debug)
    -h, --help               Print help
    -V, --version            Print version

TARGETS (repeatable; subcategory flags attach to the preceding category):
    --cpu [--usage] [--temp]
    --memory [--usage]
    --gpu <DEVICE> [--usage] [--mem] [--temp]
    --disk <DEVICE> [--usage] [--fill] [--temp]
    --fans <DEVICE> [--rpm]",
                env!("CARGO_PKG_VERSION")
            );
            Ok(())
        }
        Some("cpu") => {
            println!(
                "\
adsymon --cpu [OPTIONS]

OPTIONS:
    --usage    Aggregate CPU utilization percentage (default)
    --temp     CPU package temperature from hwmon"
            );
            Ok(())
        }
        Some("memory" | "mem") => {
            println!(
                "\
adsymon --memory [OPTIONS]

OPTIONS:
    --usage    RAM and swap usage statistics (default)"
            );
            Ok(())
        }
        Some("gpu") => {
            println!(
                "\
adsymon --gpu <DEVICE> [OPTIONS]

ARGS:
    <DEVICE>   DRM device path, card name, or PCI slot (see `adsymon list gpu`)

OPTIONS:
    --usage    GPU busy percentage
    --mem      VRAM used/total in MB
    --temp     GPU temperature in Celsius"
            );
            Ok(())
        }
        Some("disk") => {
            println!(
                "\
adsymon --disk <DEVICE> [OPTIONS]

ARGS:
    <DEVICE>   Block device name or /dev/<name> (see `adsymon list disk`)

OPTIONS:
    --usage    Block I/O activity stats
    --fill     Filesystem fill % and used/avail/total KB
    --temp     Drive temperature from hwmon"
            );
            Ok(())
        }
        Some("fans") => {
            println!(
                "\
adsymon --fans <DEVICE> [OPTIONS]

ARGS:
    <DEVICE>   Fan channel ID like hwmon2/fan1 (see `adsymon list fans`)

OPTIONS:
    --rpm      Fan speed in RPM (default)"
            );
            Ok(())
        }
        Some(other) => Err(format!("unknown category '{other}'")),
    }
}

fn require_before_targets(flag: &str, current: &Option<Target>) -> Result<(), String> {
    if current.is_some() {
        return Err(format!("{flag} must appear before target flags"));
    }
    Ok(())
}

fn next_val(
    flag: &str,
    inline: Option<&str>,
    iter: &mut impl Iterator<Item = String>,
) -> Result<String, String> {
    if let Some(v) = inline {
        return Ok(v.to_string());
    }
    iter.next()
        .ok_or_else(|| format!("missing value for {flag}"))
}

fn flush_target(current: Option<Target>, targets: &mut Vec<Target>) {
    let Some(mut target) = current else {
        return;
    };
    match &mut target {
        Target::Cpu { usage, temp } if !*usage && !*temp => *usage = true,
        Target::Memory { usage } => *usage = true,
        Target::Gpu {
            usage, vram, temp, ..
        } if !*usage && !*vram && !*temp => {
            *usage = true;
            *vram = true;
            *temp = true;
        }
        Target::Disk {
            usage, fill, temp, ..
        } if !*usage && !*fill && !*temp => *usage = true,
        Target::Fans { rpm, .. } => *rpm = true,
        _ => {}
    }
    targets.push(target);
}

pub fn parse_args<I>(args: I) -> Result<Command, String>
where
    I: IntoIterator<Item = String>,
{
    let raw: Vec<String> = args.into_iter().skip(1).collect();

    if raw.is_empty() {
        return Ok(Command::Help { category: None });
    }

    if let Some(first) = raw.first().map(|s| s.as_str()) {
        match first {
            "-h" | "--help" | "help" => return Ok(Command::Help { category: None }),
            "-V" | "--version" => return Ok(Command::Version),
            "list" => {
                if raw.len() > 2 {
                    return Err("usage: adsymon list [category]".to_string());
                }
                return Ok(Command::List {
                    category: raw.get(1).map(|s| s.to_ascii_lowercase()),
                });
            }
            _ if raw.get(1).map(|s| s.as_str()) == Some("help") && raw.len() == 2 => {
                return Ok(Command::Help {
                    category: Some(first.to_ascii_lowercase()),
                });
            }
            _ => {}
        }
    }

    let mut interval_ms = 1000u64;
    let mut output_dir = default_output_dir();
    let mut once = false;
    let mut stdout = false;
    let mut targets = Vec::new();
    let mut current: Option<Target> = None;

    let mut iter = raw.into_iter();
    while let Some(arg) = iter.next() {
        let (flag, inline) = match arg.split_once('=') {
            Some((k, v)) if arg.starts_with("--") => (k, Some(v)),
            _ => (arg.as_str(), None),
        };

        match flag {
            "-h" | "--help" => return Ok(Command::Help { category: None }),
            "-V" | "--version" => return Ok(Command::Version),
            "-i" | "--interval" => {
                require_before_targets(flag, &current)?;
                let val = next_val(flag, inline, &mut iter)?;
                let ms: u64 = val
                    .parse()
                    .map_err(|_| format!("invalid interval '{val}': expected integer ms"))?;
                if ms == 0 {
                    return Err("interval must be greater than 0 ms".to_string());
                }
                interval_ms = ms;
            }
            "-o" | "--output-dir" => {
                require_before_targets(flag, &current)?;
                output_dir = PathBuf::from(next_val(flag, inline, &mut iter)?);
            }
            "--once" => {
                require_before_targets(flag, &current)?;
                once = true;
            }
            "--stdout" | "--debug" => {
                require_before_targets(flag, &current)?;
                stdout = true;
            }
            "--cpu" => {
                flush_target(current.take(), &mut targets);
                current = Some(Target::Cpu {
                    usage: false,
                    temp: false,
                });
            }
            "--memory" => {
                flush_target(current.take(), &mut targets);
                current = Some(Target::Memory { usage: false });
            }
            "--gpu" => {
                flush_target(current.take(), &mut targets);
                let device = next_val(flag, inline, &mut iter)?;
                current = Some(Target::Gpu {
                    device,
                    usage: false,
                    vram: false,
                    temp: false,
                });
            }
            "--disk" => {
                flush_target(current.take(), &mut targets);
                let device = next_val(flag, inline, &mut iter)?;
                current = Some(Target::Disk {
                    device,
                    usage: false,
                    fill: false,
                    temp: false,
                });
            }
            "--fans" => {
                flush_target(current.take(), &mut targets);
                let device = next_val(flag, inline, &mut iter)?;
                current = Some(Target::Fans { device, rpm: false });
            }
            "--usage" => {
                let Some(target) = current.as_mut() else {
                    return Err("--usage must follow a category flag".to_string());
                };
                match target {
                    Target::Cpu { usage, .. }
                    | Target::Memory { usage }
                    | Target::Gpu { usage, .. }
                    | Target::Disk { usage, .. } => *usage = true,
                    Target::Fans { .. } => {
                        return Err("--usage is not supported for --fans (use --rpm)".to_string());
                    }
                }
            }
            "--mem" => {
                let Some(target) = current.as_mut() else {
                    return Err("--mem must follow --gpu".to_string());
                };
                match target {
                    Target::Gpu { vram, .. } => *vram = true,
                    _ => return Err("--mem is only supported for --gpu".to_string()),
                }
            }
            "--temp" => {
                let Some(target) = current.as_mut() else {
                    return Err("--temp must follow a category flag".to_string());
                };
                match target {
                    Target::Cpu { temp, .. }
                    | Target::Gpu { temp, .. }
                    | Target::Disk { temp, .. } => *temp = true,
                    Target::Memory { .. } | Target::Fans { .. } => {
                        return Err("--temp is not supported for this category".to_string());
                    }
                }
            }
            "--rpm" => {
                let Some(target) = current.as_mut() else {
                    return Err("--rpm must follow --fans".to_string());
                };
                match target {
                    Target::Fans { rpm, .. } => *rpm = true,
                    _ => return Err("--rpm is only supported for --fans".to_string()),
                }
            }
            "--fill" => {
                let Some(target) = current.as_mut() else {
                    return Err("--fill must follow --disk".to_string());
                };
                match target {
                    Target::Disk { fill, .. } => *fill = true,
                    _ => return Err("--fill is only supported for --disk".to_string()),
                }
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    flush_target(current.take(), &mut targets);

    if targets.is_empty() {
        targets.push(Target::Cpu {
            usage: true,
            temp: false,
        });
        targets.push(Target::Memory { usage: true });
    }

    Ok(Command::Run(DaemonConfig {
        interval_ms,
        output_dir,
        once,
        stdout,
        targets,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_repeatable_gpu_targets() {
        let args = vec![
            "adsymon",
            "--once",
            "--gpu",
            "/dev/dri/by-path/pci-0000:01:00.0-render",
            "--mem",
            "--temp",
            "--gpu",
            "/dev/dri/by-path/pci-0000:00:02.0-render",
            "--mem",
        ]
        .into_iter()
        .map(String::from);

        let Command::Run(cfg) = parse_args(args).unwrap() else {
            panic!("expected Run");
        };
        assert!(cfg.once);
        assert_eq!(
            cfg.targets,
            vec![
                Target::Gpu {
                    device: "/dev/dri/by-path/pci-0000:01:00.0-render".into(),
                    usage: false,
                    vram: true,
                    temp: true,
                },
                Target::Gpu {
                    device: "/dev/dri/by-path/pci-0000:00:02.0-render".into(),
                    usage: false,
                    vram: true,
                    temp: false,
                },
            ]
        );
    }

    #[test]
    fn parses_category_help_and_list() {
        let cmd = parse_args(["adsymon", "gpu", "help"].map(String::from)).unwrap();
        assert_eq!(
            cmd,
            Command::Help {
                category: Some("gpu".into())
            }
        );

        let cmd = parse_args(["adsymon", "list", "gpu"].map(String::from)).unwrap();
        assert_eq!(
            cmd,
            Command::List {
                category: Some("gpu".into())
            }
        );
    }

    #[test]
    fn rejects_orphan_subcategory_flag() {
        let res = parse_args(["adsymon", "--temp"].map(String::from));
        assert!(res.is_err());
    }

    #[test]
    fn shows_help_when_no_args() {
        let cmd = parse_args(["adsymon"].map(String::from)).unwrap();
        assert_eq!(cmd, Command::Help { category: None });
    }

    #[test]
    fn parses_stdout_debug_flag() {
        let Command::Run(cfg) =
            parse_args(["adsymon", "--debug", "--once", "--cpu"].map(String::from)).unwrap()
        else {
            panic!("expected Run");
        };
        assert!(cfg.stdout);
        assert!(cfg.once);
    }
}
