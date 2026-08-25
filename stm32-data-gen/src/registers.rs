use std::collections::{HashMap, HashSet};

use anyhow::anyhow;
use chiptool::ir::IR;
use chiptool::validate;
#[cfg(feature = "rayon")]
use rayon::prelude::*;

pub struct Registers {
    /// Maps the file name (without the .yaml extension) to the IR object which is parsed from the mcu .svd file
    pub registers: HashMap<String, IR>,
    pub blocks: HashMap<String, HashSet<String>>,
}

impl Registers {
    pub fn parse() -> Result<Self, anyhow::Error> {
        let files = glob::glob("data/registers/*")?;

        #[cfg(feature = "rayon")]
        let files = files.par_bridge();

        let registers: HashMap<String, IR> = files
            .map(|f| {
                let f = f?;
                let ff = f
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .strip_suffix(".yaml")
                    .unwrap()
                    .to_string();
                let ir: IR = serde_yaml::from_str(&std::fs::read_to_string(&f)?)
                    .map_err(|e| anyhow!("failed to parse {f:?}: {e:?}"))?;

                // validate yaml file
                // we allow register overlap and field overlap for now
                let validate_option = validate::Options {
                    allow_register_overlap: true,
                    allow_field_overlap: true,
                    allow_enum_dup_value: false,
                    allow_unused_enums: false,
                    allow_unused_fieldsets: false,
                };
                let err_vec = validate::validate(&ir, validate_option);
                let err_string = err_vec.iter().fold(String::new(), |mut acc, cur| {
                    acc.push_str(cur);
                    acc.push('\n');
                    acc
                });

                if !err_string.is_empty() {
                    return Err(anyhow!(format!("\n{ff}:\n{err_string}")));
                }

                Ok((ff, ir))
            })
            .collect::<Result<_, _>>()?;

        let blocks = registers
            .iter()
            .map(|(k, v)| (k.clone(), v.blocks.keys().cloned().collect()))
            .collect();

        Ok(Self { registers, blocks })
    }

    pub fn write(&self) -> Result<(), anyhow::Error> {
        std::fs::create_dir_all("build/data/registers")?;

        let registers = self.registers.iter();

        #[cfg(feature = "rayon")]
        let registers = registers.par_bridge();

        registers
            .map(|(name, ir)| {
                let dump = serde_json::to_string_pretty(ir)?;
                std::fs::write(format!("build/data/registers/{name}.json"), dump)?;

                Ok(())
            })
            .collect::<anyhow::Result<()>>()?;

        Ok(())
    }
}
