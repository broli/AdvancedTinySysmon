mod cli;
mod modules;
mod output;

use std::env;
use std::fs;
use std::process;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use cli::{Command, parse_args, print_help};
use modules::{FieldHistory, Poller, Reading, list_devices};
use output::{write_atomic, write_json};

const STATUS_FILE: &str = "status.json";

fn main() {
    let cmd = match parse_args(env::args()) {
        Ok(cmd) => cmd,
        Err(err) => {
            eprintln!("error: {err}");
            process::exit(2);
        }
    };

    let cfg = match cmd {
        Command::Help { category } => {
            if let Err(err) = print_help(category.as_deref()) {
                eprintln!("error: {err}");
                process::exit(2);
            }
            return;
        }
        Command::Version => {
            println!("adsymon {}", env!("CARGO_PKG_VERSION"));
            return;
        }
        Command::List { category } => {
            if let Err(err) = list_devices(category.as_deref()) {
                eprintln!("error: {err}");
                process::exit(2);
            }
            return;
        }
        Command::Run(cfg) => cfg,
    };

    let mut pollers: Vec<Poller> = Vec::with_capacity(cfg.targets.len());
    for target in &cfg.targets {
        match Poller::from_target(target) {
            Ok(poller) => {
                if !pollers.iter().any(|p| p.key() == poller.key()) {
                    // Group by category at startup so output serialization is a single pass.
                    let insert_at = pollers
                        .iter()
                        .rposition(|p| p.category() == poller.category())
                        .map_or(pollers.len(), |idx| idx + 1);
                    pollers.insert(insert_at, poller);
                }
            }
            Err(err) => {
                eprintln!("error: {err}");
                process::exit(2);
            }
        }
    }

    if !cfg.stdout
        && let Err(err) = fs::create_dir_all(&cfg.output_dir)
    {
        eprintln!("error creating {}: {err}", cfg.output_dir.display());
        process::exit(1);
    }

    let interval = Duration::from_millis(cfg.interval_ms);
    if pollers.iter().any(|p| matches!(p, Poller::Cpu(_))) {
        thread::sleep(Duration::from_millis(100).min(interval));
    }

    let mut readings: Vec<Reading> = pollers
        .iter()
        .map(|p| Reading {
            category: p.category(),
            device_id: p.device_id().map(str::to_string),
            fields: Vec::with_capacity(8),
        })
        .collect();
    let mut histories: Vec<FieldHistory> = (0..pollers.len())
        .map(|_| FieldHistory::with_capacity(8))
        .collect();
    let mut json_buf = String::with_capacity(1024);

    loop {
        for ((poller, reading), history) in pollers
            .iter_mut()
            .zip(readings.iter_mut())
            .zip(histories.iter_mut())
        {
            match poller.poll(&mut reading.fields) {
                Ok(()) => history.average(&mut reading.fields),
                Err(err) => eprintln!("warn [{}]: {err}", poller.category()),
            }
        }

        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);

        write_json(&mut json_buf, timestamp_ms, &readings);
        if cfg.stdout {
            print!("{json_buf}");
        } else if let Err(err) = write_atomic(&cfg.output_dir, STATUS_FILE, &json_buf) {
            eprintln!(
                "error writing to {}: {err}",
                cfg.output_dir.join(STATUS_FILE).display()
            );
            process::exit(1);
        }

        if cfg.once {
            break;
        }

        thread::sleep(interval);
    }
}
