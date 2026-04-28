use std::collections::HashMap;
use std::fmt::Write;

use anyhow::anyhow;
use chiptool::ir::IR;
use chiptool::validate;

pub struct Registers {
    /// Maps the file name (without the .yaml extension) to the IR object which is parsed from the mcu .svd file
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
            // we allow register overlap and field overlap for now
            let validate_option = validate::Options {
                allow_register_overlap: true,
                allow_field_overlap: true,
                allow_enum_dup_value: false,
                allow_unused_enums: false,
                allow_unused_fieldsets: false,
            };
            let err_vec = validate::validate(&ir, validate_option);
            let mut err_string = err_vec.iter().fold(String::new(), |mut acc, cur| {
                acc.push_str(cur);
                acc.push('\n');
                acc
            });

            let mut check_case = |s: &String| {
                if !s.is_ascii() || s.chars().filter(|c| c.is_alphabetic()).any(|c| !c.is_uppercase()) {
                    writeln!(&mut err_string, "{} is not ascii uppercase", s).unwrap();
                }
            };

            for (n, e) in &ir.enums {
                check_case(n);

                for v in &e.variants {
                    check_case(&v.name);
                }
            }

            for (n, b) in &ir.blocks {
                check_case(n);

                for v in &b.items {
                    check_case(&v.name);
                }
            }

            for (n, b) in &ir.fieldsets {
                check_case(n);

                for v in &b.fields {
                    check_case(&v.name);
                }
            }

            if !err_string.is_empty() {
                return Err(anyhow!(format!("\n{ff}:\n{err_string}")));
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
