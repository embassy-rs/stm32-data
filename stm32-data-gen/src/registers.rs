use std::collections::HashMap;

use anyhow::anyhow;
use chiptool::ir::{BlockItemInner, IR};

pub struct Registers {
    pub registers: HashMap<String, IR>,
}

impl Registers {
    pub fn parse() -> Result<Self, anyhow::Error> {
        let mut registers = HashMap::new();

        for f in glob::glob("data/registers/*")? {
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
            for (name, block) in &ir.blocks {
                for item in &block.items {
                    match &item.inner {
                        BlockItemInner::Block(inner_block) => {
                            if !ir.blocks.contains_key(&inner_block.block) {
                                return Err(anyhow!(
                                    "block {name} specified block {} but it does not exist",
                                    inner_block.block
                                ));
                            }
                        }
                        BlockItemInner::Register(inner_register) => {
                            if let Some(fieldset) = &inner_register.fieldset {
                                if !ir.fieldsets.contains_key(fieldset) {
                                    return Err(anyhow!(
                                        "block {name} specified fieldset {fieldset} but it does not exist",
                                    ));
                                }
                            }
                        }
                    }
                }
            }

            registers.insert(ff, ir);
        }

        Ok(Self { registers })
    }

    pub fn write(&self) -> Result<(), anyhow::Error> {
        std::fs::create_dir_all("build/data/registers")?;

        for (name, ir) in &self.registers {
            let dump = serde_json::to_string_pretty(ir)?;
            std::fs::write(format!("build/data/registers/{name}.json"), dump)?;
        }
        Ok(())
    }
}
