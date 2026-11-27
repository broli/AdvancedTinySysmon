use std::env;
use std::path::PathBuf;

use crate::modules::print_modules;

#[derive(Debug, Clone)]
pub struct Config {
    pub modules: Vec<String>,
    pub interval_ms: u64,
    pub output_dir: PathBuf,
    pub once: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            modules: vec!["cpu".to_string(), "memory".to_string()],
            interval_ms: 1000,
            output_dir: default_output_dir(),
            once: false,
        }
    }
}

fn default_output_dir() -> PathBuf {
    if let Ok(runtime_dir) = env::var("XDG_RUNTIME_DIR")
        && !runtime_dir.is_empty()
    {
        return PathBuf::from(runtime_dir).join("atsm");
    }
    PathBuf::from("/dev/shm/atsm")
}

pub enum ParseOutcome {
    Run(Config),
    ExitSuccess,
}

fn print_help() {
    println!(
        "\
atsm {}

USAGE:
    atsm [OPTIONS]

OPTIONS:
    -m, --modules <LIST>     Comma-separated modules [default: cpu,memory]
    -i, --interval <MS>      Polling interval in ms [default: 1000]
    -o, --output-dir <DIR>   Output directory [default: $XDG_RUNTIME_DIR/atsm or /dev/shm/atsm]
        --once               Poll once and exit
        --list-modules       List available modules and exit
    -h, --help               Print help
    -V, --version            Print version",
        env!("CARGO_PKG_VERSION")
    );
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

pub fn parse_args<I>(args: I) -> Result<ParseOutcome, String>
where
    I: IntoIterator<Item = String>,
{
    let mut cfg = Config::default();
    let mut iter = args.into_iter();
    let _ = iter.next();

    while let Some(arg) = iter.next() {
        let (flag, inline) = match arg.split_once('=') {
            Some((k, v)) if arg.starts_with("--") => (k, Some(v)),
            _ => (arg.as_str(), None),
        };

        match flag {
            "-h" | "--help" => {
                print_help();
                return Ok(ParseOutcome::ExitSuccess);
            }
            "-V" | "--version" => {
                println!("atsm {}", env!("CARGO_PKG_VERSION"));
                return Ok(ParseOutcome::ExitSuccess);
            }
            "--list-modules" => {
                print_modules();
                return Ok(ParseOutcome::ExitSuccess);
            }
            "--once" => cfg.once = true,
            "-m" | "--modules" => {
                let val = next_val(flag, inline, &mut iter)?;
                cfg.modules = val
                    .split(',')
                    .map(|s| s.trim().to_ascii_lowercase())
                    .filter(|s| !s.is_empty())
                    .collect();
                if cfg.modules.is_empty() {
                    return Err("at least one module must be specified".to_string());
                }
            }
            "-i" | "--interval" => {
                let val = next_val(flag, inline, &mut iter)?;
                let ms: u64 = val
                    .parse()
                    .map_err(|_| format!("invalid interval '{val}': expected integer ms"))?;
                if ms == 0 {
                    return Err("interval must be greater than 0 ms".to_string());
                }
                cfg.interval_ms = ms;
            }
            "-o" | "--output-dir" => {
                let val = next_val(flag, inline, &mut iter)?;
                cfg.output_dir = PathBuf::from(val);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    Ok(ParseOutcome::Run(cfg))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_modules_and_inline_flags() {
        let args = vec![
            "atsm".into(),
            "--modules=cpu, zram".into(),
            "-i".into(),
            "500".into(),
            "--output-dir=/tmp/out".into(),
            "--once".into(),
        ];
        let ParseOutcome::Run(cfg) = parse_args(args).unwrap() else {
            panic!("expected Run");
        };
        assert_eq!(cfg.modules, vec!["cpu", "zram"]);
        assert_eq!(cfg.interval_ms, 500);
        assert_eq!(cfg.output_dir, PathBuf::from("/tmp/out"));
        assert!(cfg.once);
    }

    #[test]
    fn rejects_zero_interval() {
        let args = vec!["atsm".into(), "-i".into(), "0".into()];
        assert!(parse_args(args).is_err());
    }
}
