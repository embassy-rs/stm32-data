use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fmt::{Debug, Write as _};
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

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

struct Dedup {
    peripherals: HashSet<blake3::Hash>,
    interrupts: HashSet<blake3::Hash>,
    dma_channels: HashSet<blake3::Hash>,
    pins: HashSet<blake3::Hash>,
}

pub struct Gen {
    opts: Options,
    dedup: Mutex<Dedup>,
}

impl Gen {
    pub fn new(opts: Options) -> Self {
        Self {
            opts,
            dedup: Mutex::new(Dedup {
                peripherals: HashSet::new(),
                interrupts: HashSet::new(),
                dma_channels: HashSet::new(),
                pins: HashSet::new(),
            }),
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
                if let Some(old_version) = peripheral_versions.insert(bi.kind.clone(), bi.version.clone()) {
                    if old_version != bi.version {
                        panic!(
                            "Peripheral {} has multiple versions: {} and {}",
                            bi.kind, old_version, bi.version
                        );
                    }
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

        let chip_dir = self
            .opts
            .out_dir
            .join("src/chips")
            .join(chip_core_name.to_ascii_lowercase());
        fs::create_dir_all(&chip_dir).unwrap();

        // ==============================
        // generate pac.rs

        let data = generate::render(&ir, &gen_opts()).unwrap().to_string();
        let data = data.replace("] ", "]\n");

        // Remove inner attributes like #![no_std]
        let data = Regex::new("# *! *\\[.*\\]").unwrap().replace_all(&data, "");

        let mut file = File::create(chip_dir.join("pac.rs")).unwrap();
        file.write_all(data.as_bytes()).unwrap();
        file.write_all(extra.as_bytes()).unwrap();

        let mut device_x = String::new();

        for irq in &core.interrupts {
            writeln!(&mut device_x, "PROVIDE({} = DefaultHandler);", irq.name).unwrap();
        }

        // ==============================
        // generate metadata.rs

        let out_dir = self.opts.out_dir.clone();
        let meta_dir = out_dir.join("src").join("chips").join("metadata");

        // (peripherals, interrupts, dma_channels) are often equal across multiple chips.
        // To reduce bloat, deduplicate them.
        let mod_peripherals = stringify_module(&core.peripherals, "peripherals", &meta_dir);
        let mod_interrupts = stringify_module(&core.interrupts, "interrupts", &meta_dir);
        let mod_dma_channels = stringify_module(&core.dma_channels, "dma_channels", &meta_dir);
        let mod_pins = stringify_module(&core.pins, "pins", &meta_dir);

        let (write_peripherals, write_interrupts, write_dma_channels, write_pins) = {
            let mut dedup = self.dedup.lock().unwrap();

            (
                dedup.peripherals.insert(mod_peripherals.hash.clone()),
                dedup.interrupts.insert(mod_interrupts.hash.clone()),
                dedup.dma_channels.insert(mod_dma_channels.hash.clone()),
                dedup.pins.insert(mod_pins.hash.clone()),
            )
        };

        if write_peripherals {
            let rendered_peripherals = format!(
                "pub(crate) static PERIPHERALS: &[Peripheral] = {};",
                mod_peripherals.contents
            );

            let mut rendered_peripherals = regex!("\":ir_for:([a-z0-9]+):\"")
                .replace_all(&rendered_peripherals, "&$1::REGISTERS")
                .to_string();

            for (module, version) in &peripheral_versions {
                writeln!(
                    &mut rendered_peripherals,
                    "#[path=\"../../registers/{}_{}.rs\"] pub mod {};",
                    module, version, module
                )
                .unwrap();
            }

            fs::write(&mod_peripherals.path, &rendered_peripherals).unwrap();
        }

        if write_interrupts {
            fs::write(
                &mod_interrupts.path,
                format!(
                    "pub(crate) static INTERRUPTS: &[Interrupt] = {};",
                    mod_interrupts.contents
                ),
            )
            .unwrap();
        }

        if write_dma_channels {
            fs::write(
                &mod_dma_channels.path,
                format!(
                    "pub(crate) static DMA_CHANNELS: &[DmaChannel] = {};",
                    mod_dma_channels.contents
                ),
            )
            .unwrap();
        }

        if write_pins {
            fs::write(
                &mod_pins.path,
                format!("pub(crate) static PINS: &[Pin] = {};", mod_pins.contents),
            )
            .unwrap();
        }

        let memories = chip
            .memory
            .iter()
            .map(|memory| stringify(memory))
            .collect::<Vec<_>>()
            .join(",");

        let data = format!(
            "include!(\"../metadata/{}\");
            include!(\"../metadata/{}\");
            include!(\"../metadata/{}\");
            include!(\"../metadata/{}\");
            use crate::metadata::PeripheralRccKernelClock::{{Clock, Mux}};
            pub static METADATA: Metadata = Metadata {{
                name: {:?},
                family: {:?},
                line: {:?},
                memory: &[{}],
                peripherals: PERIPHERALS,
                nvic_priority_bits: {:?},
                interrupts: INTERRUPTS,
                dma_channels: DMA_CHANNELS,
                pins: PINS,
            }};",
            &mod_interrupts.file_name,
            &mod_dma_channels.file_name,
            &mod_pins.file_name,
            &mod_peripherals.file_name,
            &chip.name,
            &chip.family,
            &chip.line,
            memories,
            &core.nvic_priority_bits,
        );

        fs::write(chip_dir.join("metadata.rs"), &data).unwrap();

        // ==============================
        // generate device.x

        fs::write(chip_dir.join("device.x"), &device_x).unwrap();

        peripheral_versions
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

        let chips = self.opts.chips.clone();

        #[cfg(feature = "rayon")]
        let chips = chips.par_iter();

        #[cfg(not(feature = "rayon"))]
        let chips = chips.iter();

        let (peripheral_versions, chip_core_names): (Vec<HashSet<(String, String)>>, Vec<Vec<String>>) = chips
            .map(|chip_name| {
                println!("Generating {}...", chip_name);

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

        #[cfg(feature = "rayon")]
        let peripheral_iter = peripheral_versions.par_iter();

        #[cfg(not(feature = "rayon"))]
        let peripheral_iter = peripheral_versions.iter();

        peripheral_iter
            .map(|(module, version)| {
                println!("loading {} {}", module, version);

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

                fs::write(
                    self.opts
                        .out_dir
                        .join("src/peripherals")
                        .join(format!("{}_{}.rs", module, version)),
                    generate::render(&ir, &gen_opts()).unwrap().to_string(),
                )
                .unwrap();

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

                    fs::write(
                        self.opts
                            .out_dir
                            .join("src/registers")
                            .join(format!("{}_{}.rs", module, version)),
                        &file,
                    )
                    .unwrap();
                }

                ()
            })
            .collect::<Vec<_>>();

        // Generate Cargo.toml
        let mut contents = include_bytes!("../res/Cargo.toml").to_vec();
        for name in &chip_core_names {
            writeln!(&mut contents, "{} = []", name.to_ascii_lowercase()).unwrap();
        }
        fs::write(self.opts.out_dir.join("Cargo.toml"), contents).unwrap();

        // Generate src/all_chips.rs
        {
            let contents = gen_all_chips(&self.opts.chips);
            fs::write(self.opts.out_dir.join("src/all_chips.rs"), &contents).unwrap();
        }

        // Generate src/all_peripheral_versions.rs
        {
            let contents = gen_all_peripheral_versions(&peripheral_versions);
            fs::write(self.opts.out_dir.join("src/all_peripheral_versions.rs"), &contents).unwrap();
        }

        // copy misc files
        fs::write(self.opts.out_dir.join("README.md"), include_bytes!("../res/README.md")).unwrap();
        fs::write(self.opts.out_dir.join("build.rs"), include_str!("../res/build.rs")).unwrap();
        fs::write(self.opts.out_dir.join("src/lib.rs"), include_str!("../res/src/lib.rs")).unwrap();
        fs::write(
            self.opts.out_dir.join("src/common.rs"),
            chiptool::generate::COMMON_MODULE,
        )
        .unwrap();
        fs::write(
            self.opts.out_dir.join("src/metadata.rs"),
            include_str!("../res/src/metadata.rs"),
        )
        .unwrap();
    }
}

fn stringify<T: Debug>(metadata: T) -> String {
    let mut metadata = format!("{:#?}", metadata);
    if metadata.starts_with('[') {
        metadata = format!("&{}", metadata);
    }

    metadata.replace(": [", ": &[")
}

struct Module {
    contents: String,
    hash: blake3::Hash,
    file_name: String,
    path: PathBuf,
}

fn stringify_module<T: Debug>(metadata: T, r#type: &str, root: &Path) -> Module {
    let contents = stringify(metadata);
    let hash = hash(&contents.as_bytes());
    let file_name = format!("{}_{}.rs", r#type, hash.to_hex());
    let path = root.join(&file_name);

    Module {
        contents,
        hash,
        file_name,
        path,
    }
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
