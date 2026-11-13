use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt::{Debug, Write as _};
use std::fs;
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Instant;

use blake3::hash;
use chiptool::generate::CommonModule;
use chiptool::{generate, ir, transform};
use lazy_regex::regex;
#[cfg(feature = "rayon")]
use rayon::prelude::*;
use regex::Regex;

mod data;
use data::*;

pub struct Options {
    pub chips: Vec<String>,
    pub out_dir: PathBuf,
    pub data_dir: PathBuf,
}

pub struct Gen {
    opts: Options,
    /// Content hashes of shared files already written this run (write-once guard
    /// for parallel chips that map to the same content-addressed file).
    seen: Mutex<HashSet<blake3::Hash>>,
    /// Per-path hash of the bytes this generator last wrote (loaded from
    /// `.state` at startup). Lets re-runs skip unchanged files even though a
    /// later rustfmt pass rewrites their on-disk formatting.
    prev_state: HashMap<String, String>,
    /// State for this run: every emitted path (written or verified current).
    state: Mutex<HashMap<String, String>>,
    /// Output paths written this run, relative to `out_dir` (forward slashes);
    /// persisted as `.changed` so downstream tooling (rustfmt in `./d gen-all`)
    /// can process only what actually changed.
    changed: Mutex<Vec<String>>,
}

impl Gen {
    pub fn new(opts: Options) -> Self {
        let prev_state = fs::read_to_string(opts.out_dir.join(".state"))
            .map(|s| parse_state(&s))
            .unwrap_or_default();

        Self {
            opts,
            seen: Mutex::new(HashSet::new()),
            prev_state,
            state: Mutex::new(HashMap::new()),
            changed: Mutex::new(Vec::new()),
        }
    }

    fn gen_chip(&self, chip_core_name: &str, chip: &Chip, core: &Core) -> BTreeMap<String, String> {
        let mut ir = ir::IR::new();

        let mut dev = ir::Device {
            nvic_priority_bits: core.nvic_priority_bits,
            interrupts: Vec::new(),
            peripherals: Vec::new(),
        };

        let mut peripheral_versions: BTreeMap<String, String> = BTreeMap::new();

        for p in &core.peripherals {
            let mut ir_peri = ir::Peripheral {
                name: p.name.clone(),
                array: None,
                base_address: p.address,
                block: None,
                description: None,
                interrupts: BTreeMap::new(),
            };

            if let Some(bi) = &p.registers {
                if let Some(old_version) = peripheral_versions.insert(bi.kind.clone(), bi.version.clone())
                    && old_version != bi.version
                {
                    panic!(
                        "Peripheral {} has multiple versions: {} and {}",
                        bi.kind, old_version, bi.version
                    );
                }
                ir_peri.block = Some(format!("{}::{}", bi.kind, bi.block));
            }

            dev.peripherals.push(ir_peri);
        }

        for irq in &core.interrupts {
            dev.interrupts.push(ir::Interrupt {
                name: irq.name.clone(),
                description: None,
                value: irq.number,
            });
        }

        ir.devices.insert("".to_string(), dev);

        let mut extra = String::new();

        for (module, version) in &peripheral_versions {
            writeln!(
                &mut extra,
                "#[path=\"../../peripherals/{}_{}.rs\"] pub mod {};",
                module, version, module
            )
            .unwrap();
        }

        // Cleanups!
        transform::sort::Sort {}.run(&mut ir).unwrap();
        transform::sanitize::Sanitize::default().run(&mut ir).unwrap();

        // ==============================
        // Setup chip dir

        let chip_root = self.opts.out_dir.join("src").join("chips");
        let chip_dir = chip_root.join(chip_core_name.to_ascii_lowercase());
        fs::create_dir_all(&chip_dir).unwrap();

        // ==============================
        // generate pac.rs
        //
        // The rendered PAC body is byte-identical for every chip that shares a
        // core, so it is written once to a shared content-addressed file; the
        // chip dir keeps a small include plus its peripheral module declarations.

        let pac_body = generate::render(&ir, &gen_opts()).unwrap().to_string();
        let pac_body = pac_body.replace("] ", "]\n");

        // Remove inner attributes like #![no_std]
        let pac_body = Regex::new("# *! *\\[.*\\]").unwrap().replace_all(&pac_body, "");

        let pac_module = Module::new(pac_body.into_owned(), "pac", &chip_root);
        self.write_shared(&pac_module);

        let mut chip_pac = format!("include!(\"../{}\");\n", pac_module.file_name);
        chip_pac.push_str(&extra);
        self.write_if_changed(&chip_dir.join("pac.rs"), &chip_pac);

        let mut device_x = String::new();

        for irq in &core.interrupts {
            writeln!(&mut device_x, "PROVIDE({} = DefaultHandler);", irq.name).unwrap();
        }

        // ==============================
        // generate metadata.rs

        let meta_dir = self.opts.out_dir.join("src").join("chips").join("metadata");

        // (peripherals, interrupts, dma_channels) are often equal across multiple chips.
        // To reduce bloat, deduplicate them into content-addressed shared modules.
        // The hash covers the *final* rendered file contents (including the
        // :ir_for: rewrite and the appended module declarations), so a shared
        // name can never disagree with what is written for it.
        let peripherals_raw = format!(
            "use super::*;\npub(crate) static PERIPHERALS: &[Peripheral] = {};",
            stringify(&core.peripherals)
        );
        let mut peripherals_contents = regex!("\":ir_for:([a-z0-9]+):\"")
            .replace_all(&peripherals_raw, "&$1::REGISTERS")
            .to_string();

        for (module, version) in &peripheral_versions {
            writeln!(
                &mut peripherals_contents,
                "#[path=\"../../registers/{}_{}.rs\"] pub mod {};",
                module, version, module
            )
            .unwrap();
        }

        let mod_peripherals = Module::new(peripherals_contents, "peripherals", &meta_dir);
        let mod_interrupts = Module::new(
            format!(
                "use super::*;\npub(crate) static INTERRUPTS: &[Interrupt] = {};",
                stringify(&core.interrupts)
            ),
            "interrupts",
            &meta_dir,
        );
        let mod_dma_channels = Module::new(
            format!(
                "use super::*;\npub(crate) static DMA_CHANNELS: &[DmaChannel] = {};",
                stringify(&core.dma_channels)
            ),
            "dma_channels",
            &meta_dir,
        );
        let mod_pins = Module::new(
            format!(
                "use super::*;\npub(crate) static PINS: &[Pin] = {};",
                stringify(&core.pins)
            ),
            "pins",
            &meta_dir,
        );

        self.write_shared(&mod_peripherals);
        self.write_shared(&mod_interrupts);
        self.write_shared(&mod_dma_channels);
        self.write_shared(&mod_pins);

        let memories = chip.memory.iter().map(stringify).collect::<Vec<_>>().join(",");

        // Only import the prescaled variant for chips that use it, so the metadata of all
        // other chips stays byte-identical.
        let kernel_clock_import = if core.peripherals.iter().any(|p| {
            p.rcc
                .as_ref()
                .is_some_and(|rcc| matches!(&rcc.kernel_clock, PeripheralRccKernelClock::PrescaledMux(_)))
        }) {
            "use crate::metadata::PeripheralRccKernelClock::{Clock, Mux, PrescaledMux};"
        } else {
            "use crate::metadata::PeripheralRccKernelClock::{Clock, Mux};"
        };

        let data = format!(
            "#[path=\"../metadata/{}\"] mod interrupts;
            #[path=\"../metadata/{}\"] mod dma_channels;
            #[path=\"../metadata/{}\"] mod pins;
            #[path=\"../metadata/{}\"] mod peripherals;
            {}
            pub static METADATA: Metadata = Metadata {{
                name: {:?},
                family: {:?},
                line: {:?},
                memory: &[{}],
                peripherals: peripherals::PERIPHERALS,
                nvic_priority_bits: {:?},
                interrupts: interrupts::INTERRUPTS,
                dma_channels: dma_channels::DMA_CHANNELS,
                pins: pins::PINS,
            }};",
            &mod_interrupts.file_name,
            &mod_dma_channels.file_name,
            &mod_pins.file_name,
            &mod_peripherals.file_name,
            kernel_clock_import,
            &chip.name,
            &chip.family,
            &chip.line,
            memories,
            &core.nvic_priority_bits,
        );

        self.write_if_changed(&chip_dir.join("metadata.rs"), &data);

        // ==============================
        // generate device.x

        self.write_if_changed(&chip_dir.join("device.x"), &device_x);

        peripheral_versions
    }

    /// Write `contents` to `path` only when the bytes differ from what this
    /// generator last wrote there (see `.state`), and record the path in the
    /// changed set. Unchanged files are left untouched so they keep their
    /// formatted on-disk state and skip downstream formatting.
    fn write_if_changed(&self, path: &Path, contents: impl AsRef<[u8]>) {
        let contents = contents.as_ref();
        let digest = hash(contents).to_hex().to_string();
        let rel = path
            .strip_prefix(&self.opts.out_dir)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/")
            .to_string();

        if self.prev_state.get(&rel) == Some(&digest) {
            self.state.lock().unwrap().insert(rel, digest);
            return;
        }

        fs::write(path, contents).unwrap();
        self.changed.lock().unwrap().push(rel.clone());
        self.state.lock().unwrap().insert(rel, digest);
    }

    /// Write a shared content-addressed module at most once per run.
    fn write_shared(&self, module: &Module) {
        if self.seen.lock().unwrap().insert(module.hash) {
            self.write_if_changed(&module.path, &module.contents);
        }
    }

    /// Remove generated files that this run did not emit: entries from previous
    /// runs (tracked in `.state`) and untracked leftovers from older layouts.
    /// Failures are reported but never fatal: a leftover file is a nuisance,
    /// whereas aborting over it produces a half-written tree. Non-generated
    /// content (`target/`, `Cargo.lock`, …) is left alone.
    fn gc_stale(&self) {
        let state = self.state.lock().unwrap();
        let mut orphans = Vec::new();
        Self::collect_orphans(&self.opts.out_dir, &self.opts.out_dir, &state, &mut orphans);
        let mut removed = 0usize;
        for rel in orphans {
            let full = self.opts.out_dir.join(&rel);
            match fs::remove_file(&full) {
                Ok(()) => removed += 1,
                Err(e) => println!("warning: could not remove stale {}: {e}", full.display()),
            }
        }
        if removed > 0 {
            println!("removed {removed} stale files");
        }
    }

    fn collect_orphans(dir: &Path, root: &Path, state: &HashMap<String, String>, out: &mut Vec<String>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if path.is_dir() {
                if name != "target" {
                    Self::collect_orphans(&path, root, state, out);
                }
                continue;
            }
            if name == ".state" || name == ".changed" || name == "Cargo.lock" {
                continue;
            }
            if !(name.ends_with(".rs") || name.ends_with(".x")) {
                continue;
            }
            let rel = path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
            if !state.contains_key(&rel) {
                out.push(rel);
            }
        }
    }

    fn write_state(&self) {
        let state_guard = self.state.lock().unwrap();
        let mut state = state_guard.iter().collect::<Vec<_>>();
        state.sort();
        let mut out = String::new();
        for (path, digest) in state {
            writeln!(&mut out, "{digest} {path}").unwrap();
        }
        drop(state_guard);
        fs::write(self.opts.out_dir.join(".state"), out).unwrap();
    }

    fn load_chip(&self, name: &str) -> Chip {
        let chip_path = self.opts.data_dir.join("chips").join(format!("{}.json", name));
        let chip = fs::read(chip_path).unwrap_or_else(|_| panic!("Could not load chip {}", name));
        serde_json::from_slice(&chip).unwrap()
    }

    pub fn run_gen(&mut self) {
        fs::create_dir_all(self.opts.out_dir.join("src/peripherals")).unwrap();
        fs::create_dir_all(self.opts.out_dir.join("src/registers")).unwrap();
        fs::create_dir_all(self.opts.out_dir.join("src/chips")).unwrap();
        fs::create_dir_all(self.opts.out_dir.join("src/chips/metadata")).unwrap();

        let start = Instant::now();

        let chips = self.opts.chips.clone();

        let n_chips = chips.len();
        println!("generating {}...", plural(n_chips, "chip"));
        let progress = Mutex::new(Progress::new("chips", n_chips));
        let section = Instant::now();

        #[cfg(feature = "rayon")]
        let chips = chips.par_iter();

        #[cfg(not(feature = "rayon"))]
        let chips = chips.iter();

        let (peripheral_versions, chip_core_names): (Vec<HashSet<(String, String)>>, Vec<Vec<String>>) = chips
            .map(|chip_name| {
                progress.lock().unwrap().step(chip_name);

                let mut chip = self.load_chip(chip_name);

                // Cleanup
                for core in &mut chip.cores {
                    for irq in &mut core.interrupts {
                        irq.name = irq.name.to_ascii_uppercase();
                    }
                    for p in &mut core.peripherals {
                        for irq in &mut p.interrupts {
                            irq.interrupt = irq.interrupt.to_ascii_uppercase();
                        }

                        if let Some(registers) = &mut p.registers {
                            registers.ir = format!(":ir_for:{}:", registers.kind);
                        }
                    }
                }

                let mut peripheral_versions: HashSet<(String, String)> = HashSet::new();
                let mut chip_core_names: Vec<String> = Vec::new();

                // Generate
                for core in &chip.cores {
                    let chip_core_name = match chip.cores.len() {
                        1 => chip_name.clone(),
                        _ => format!("{}-{}", chip_name, core.name),
                    };

                    chip_core_names.push(chip_core_name.clone());
                    peripheral_versions.extend(self.gen_chip(&chip_core_name, &chip, core));
                }

                (peripheral_versions, chip_core_names)
            })
            .unzip();

        let peripheral_versions: HashSet<(String, String)> = peripheral_versions.into_iter().flatten().collect();
        let chip_core_names: Vec<String> = chip_core_names.into_iter().flatten().collect();

        progress.lock().unwrap().finish();
        println!("chips: {n_chips} in {:.2}s", section.elapsed().as_secs_f64());

        let n_regs = peripheral_versions.len();
        println!("generating {}...", plural(n_regs, "peripheral register set"));
        let progress = Mutex::new(Progress::new("registers", n_regs));
        let section = Instant::now();

        #[cfg(feature = "rayon")]
        let peripheral_iter = peripheral_versions.par_iter();

        #[cfg(not(feature = "rayon"))]
        let peripheral_iter = peripheral_versions.iter();

        peripheral_iter.for_each(|(module, version)| {
            progress.lock().unwrap().step(&format!("{} {}", module, version));

            let regs_path = Path::new(&self.opts.data_dir)
                .join("registers")
                .join(&format!("{}_{}.json", module, version));

            let mut ir: ir::IR = serde_json::from_slice(&fs::read(regs_path).unwrap()).unwrap();

            transform::expand_extends::ExpandExtends {}.run(&mut ir).unwrap();

            transform::map_names(&mut ir, |k, s| match k {
                transform::NameKind::Block => *s = s.to_string(),
                transform::NameKind::Fieldset => *s = format!("regs::{}", s),
                transform::NameKind::Enum => *s = format!("vals::{}", s),
                _ => {}
            });

            transform::sort::Sort {}.run(&mut ir).unwrap();
            transform::sanitize::Sanitize::default().run(&mut ir).unwrap();

            let data = generate::render(&ir, &gen_opts()).unwrap().to_string();
            let data = data.replace("] ", "]\n");

            self.write_if_changed(
                &self
                    .opts
                    .out_dir
                    .join("src/peripherals")
                    .join(format!("{}_{}.rs", module, version)),
                &data,
            );

            {
                let ir = crate::data::ir::IR::from_chiptool(ir);
                let mut file = String::new();

                write!(
                    &mut file,
                    "
                            use crate::metadata::ir::*;
                            pub(crate) static REGISTERS: IR = {};
                        ",
                    stringify(&ir),
                )
                .unwrap();

                self.write_if_changed(
                    &self
                        .opts
                        .out_dir
                        .join("src/registers")
                        .join(format!("{}_{}.rs", module, version)),
                    &file,
                );
            }
        });

        // Generate Cargo.toml
        let mut contents = include_bytes!("../res/Cargo.toml").to_vec();
        for name in &chip_core_names {
            writeln!(&mut contents, "{} = []", name.to_ascii_lowercase()).unwrap();
        }
        self.write_if_changed(&self.opts.out_dir.join("Cargo.toml"), &contents);

        // Generate src/all_chips.rs
        {
            let contents = gen_all_chips(&self.opts.chips);
            self.write_if_changed(&self.opts.out_dir.join("src/all_chips.rs"), &contents);
        }

        // Generate src/all_peripheral_versions.rs
        {
            let contents = gen_all_peripheral_versions(&peripheral_versions);
            self.write_if_changed(&self.opts.out_dir.join("src/all_peripheral_versions.rs"), &contents);
        }

        // copy misc files
        self.write_if_changed(&self.opts.out_dir.join("README.md"), include_bytes!("../res/README.md"));
        self.write_if_changed(&self.opts.out_dir.join("build.rs"), include_str!("../res/build.rs"));
        self.write_if_changed(&self.opts.out_dir.join("src/lib.rs"), include_str!("../res/src/lib.rs"));
        self.write_if_changed(
            &self.opts.out_dir.join("src/common.rs"),
            chiptool::generate::COMMON_MODULE,
        );
        self.write_if_changed(
            &self.opts.out_dir.join("src/metadata.rs"),
            include_str!("../res/src/metadata.rs"),
        );

        // Remove outputs of previous runs that are no longer emitted, then
        // record the run's state and the changed-file list for downstream
        // tooling (`.changed` is consumed and removed by `./d gen-all`).
        self.gc_stale();
        self.write_state();
        {
            let mut changed = self.changed.lock().unwrap();
            changed.sort();
            changed.dedup();
            fs::write(self.opts.out_dir.join(".changed"), changed.join("\n")).unwrap();
        }

        progress.lock().unwrap().finish();
        println!("registers: {n_regs} in {:.2}s", section.elapsed().as_secs_f64());
        println!("total: {:.2}s", start.elapsed().as_secs_f64());
    }
}

fn plural(n: usize, noun: &str) -> String {
    format!("{} {}{}", n, noun, if n == 1 { "" } else { "s" })
}

/// Prints a single self-overwriting progress line while stdout is a terminal.
/// When stdout is redirected (CI logs, `| tee`) nothing is printed, so the
/// output stays down to the summary lines.
struct Progress {
    label: &'static str,
    total: usize,
    done: usize,
    width: usize,
    interactive: bool,
}

impl Progress {
    fn new(label: &'static str, total: usize) -> Self {
        Self {
            label,
            total,
            done: 0,
            width: 0,
            interactive: std::io::stdout().is_terminal(),
        }
    }

    fn step(&mut self, name: &str) {
        self.done += 1;
        if !self.interactive {
            return;
        }
        let line = format!("{} {}/{} {}", self.label, self.done, self.total, name);
        self.width = self.width.max(line.len());
        print!("\r{line}");
        let _ = std::io::stdout().flush();
    }

    fn finish(&mut self) {
        if !self.interactive || self.width == 0 {
            return;
        }
        // pad over the progress line, then go back to the start of it
        print!("\r{:width$}\r", "", width = self.width);
        let _ = std::io::stdout().flush();
        self.width = 0;
    }
}

impl Drop for Progress {
    fn drop(&mut self) {
        self.finish();
    }
}

fn stringify<T: Debug>(metadata: T) -> String {
    let mut metadata = format!("{:#?}", metadata);
    if metadata.starts_with('[') {
        metadata = format!("&{}", metadata);
    }

    metadata.replace(": [", ": &[")
}

/// A shared, content-addressed output file. The hash covers the exact bytes
/// that are written, so the file name can never disagree with its contents and
/// identical inputs across chips map to the same file.
struct Module {
    contents: String,
    hash: blake3::Hash,
    file_name: String,
    path: PathBuf,
}

impl Module {
    fn new(contents: String, kind: &str, root: &Path) -> Self {
        let hash = hash(contents.as_bytes());
        let file_name = format!("{}_{}.rs", kind, hash.to_hex());
        let path = root.join(&file_name);

        Self {
            contents,
            hash,
            file_name,
            path,
        }
    }
}

fn parse_state(s: &str) -> HashMap<String, String> {
    s.lines()
        .filter_map(|line| line.split_once(' '))
        .map(|(digest, path)| (path.to_string(), digest.to_string()))
        .collect()
}

fn gen_opts() -> generate::Options {
    generate::Options::new()
        .with_common_module(CommonModule::External("crate::common".parse().unwrap()))
        .with_skip_no_std(true)
}

fn gen_all_chips(chips: &[String]) -> String {
    let mut contents = String::new();
    writeln!(&mut contents, "pub static ALL_CHIPS: &[&str] = &[").unwrap();
    for chip in chips.iter() {
        writeln!(&mut contents, "    {:?},", chip).unwrap();
    }
    writeln!(&mut contents, "];").unwrap();
    contents
}

fn gen_all_peripheral_versions(all_versions: &HashSet<(String, String)>) -> String {
    let mut version_map = BTreeMap::<_, BTreeSet<_>>::new();
    for (kind, version) in all_versions.iter() {
        version_map.entry(kind).or_default().insert(version);
    }

    let mut contents = String::new();
    writeln!(
        &mut contents,
        "pub static ALL_PERIPHERAL_VERSIONS: &[(&str, &[&str])] = &["
    )
    .unwrap();
    for (kind, versions) in version_map.iter() {
        write!(&mut contents, "    ({:?}, &[", kind).unwrap();
        for version in versions.iter() {
            write!(&mut contents, "{:?}, ", version).unwrap();
        }
        writeln!(&mut contents, "]),").unwrap();
    }
    writeln!(&mut contents, "];").unwrap();
    contents
}
