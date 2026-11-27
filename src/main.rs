mod cli;
mod modules;
mod output;

use std::env;
use std::fs;
use std::process;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use cli::{ParseOutcome, parse_args};
use modules::{Module, ModuleReading, create_module};
use output::{write_atomic, write_json};

const STATUS_FILE: &str = "status.json";

fn main() {
    let cfg = match parse_args(env::args()) {
        Ok(ParseOutcome::Run(cfg)) => cfg,
        Ok(ParseOutcome::ExitSuccess) => return,
        Err(err) => {
            eprintln!("error: {err}");
            process::exit(2);
        }
    };

    let mut active: Vec<Module> = Vec::with_capacity(cfg.modules.len());
    for name in &cfg.modules {
        match create_module(name) {
            Ok(module) => {
                if !active.iter().any(|m| m.name() == module.name()) {
                    active.push(module);
                }
            }
            Err(err) => {
                eprintln!("error: {err}");
                process::exit(2);
            }
        }
    }

    if let Err(err) = fs::create_dir_all(&cfg.output_dir) {
        eprintln!("error creating {}: {err}", cfg.output_dir.display());
        process::exit(1);
    }

    let interval = Duration::from_millis(cfg.interval_ms);

    // Give delta-based modules (cpu) a short baseline window before the first poll.
    if active.iter().any(|m| matches!(m, Module::Cpu(_))) {
        thread::sleep(Duration::from_millis(100).min(interval));
    }

    let mut readings: Vec<ModuleReading> = active
        .iter()
        .map(|m| ModuleReading {
            name: m.name(),
            fields: Vec::with_capacity(8),
        })
        .collect();
    let mut json_buf = String::with_capacity(512);

    loop {
        for (module, reading) in active.iter_mut().zip(readings.iter_mut()) {
            if let Err(err) = module.poll(&mut reading.fields) {
                eprintln!("warn [{}]: {err}", module.name());
            }
        }

        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);

        write_json(&mut json_buf, timestamp_ms, &readings);
        if let Err(err) = write_atomic(&cfg.output_dir, STATUS_FILE, &json_buf) {
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
