pub mod cpu;
pub mod memory;
pub mod zram;

pub enum MetricValue {
    U64(u64),
    F64(f64),
}

pub struct ModuleReading {
    pub name: &'static str,
    pub fields: Vec<(&'static str, MetricValue)>,
}

pub enum Module {
    Cpu(cpu::CpuState),
    Memory,
    Zram,
}

impl Module {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Cpu(_) => "cpu",
            Self::Memory => "memory",
            Self::Zram => "zram",
        }
    }

    pub fn poll(&mut self, fields: &mut Vec<(&'static str, MetricValue)>) -> Result<(), String> {
        fields.clear();
        match self {
            Self::Cpu(state) => state.poll(fields),
            Self::Memory => memory::poll(fields),
            Self::Zram => zram::poll(fields),
        }
    }
}

pub const MODULES: &[(&str, &str)] = &[
    ("cpu", "Aggregate CPU utilization percentage from /proc/stat"),
    ("memory", "RAM and swap usage statistics from /proc/meminfo"),
    ("zram", "zram compressed block device stats from /sys/block/zram*"),
];

pub fn print_modules() {
    println!("Available modules:");
    for (name, desc) in MODULES {
        println!("  {name:<10} {desc}");
    }
}

pub fn create_module(name: &str) -> Result<Module, String> {
    match name {
        "cpu" => Ok(Module::Cpu(cpu::CpuState::init()?)),
        "memory" | "mem" => Ok(Module::Memory),
        "zram" => Ok(Module::Zram),
        _ => Err(format!(
            "unknown module '{name}' (run with --list-modules to see available modules)"
        )),
    }
}
