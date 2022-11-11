use std::collections::HashMap;

mod mcufinder {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    pub struct Files {
        #[serde(rename = "Files")]
        pub files: Vec<File>,
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    pub struct File {
        #[serde(rename = "URL")]
        pub url: String,
        // #[serde(rename = "displayName")]
        // pub display_name: String,
        pub id_file: String,
        pub name: String,
        // #[serde(rename = "related_MCU_count")]
        // pub related_mcu_count: String,
        pub title: String,
        pub r#type: String,
        // #[serde(rename = "versionNumber")]
        // pub version_number: String,
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    pub struct Mcus {
        #[serde(rename = "MCUs")]
        pub mcus: Vec<Mcu>,
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    pub struct Mcu {
        #[serde(rename = "RPN")]
        pub rpn: String,
        pub files: Vec<McuFile>,
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    pub struct McuFile {
        pub file_id: String,
    }
}

impl From<mcufinder::File> for stm32_data_serde::chip::Doc {
    fn from(file: mcufinder::File) -> Self {
        Self {
            name: file.name,
            title: file.title,
            url: file.url,
            r#type: parse_document_type(&file.r#type).to_string(),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct AllMcuFiles(HashMap<String, stm32_data_serde::chip::Doc>);

impl AllMcuFiles {
    pub fn parse() -> anyhow::Result<Self> {
        let j = std::fs::read_to_string("sources/mcufinder/files.json")?;
        let parsed: mcufinder::Files = serde_json::from_str(&j)?;
        let all_mcu_files = parsed
            .files
            .into_iter()
            .map(|file| (file.id_file.clone(), file.into()))
            .collect();
        Ok(Self(all_mcu_files))
    }
}

#[derive(Debug, PartialEq)]
pub struct PerMcuFiles(HashMap<String, Vec<String>>);

impl PerMcuFiles {
    pub fn parse() -> anyhow::Result<Self> {
        let j = std::fs::read_to_string("sources/mcufinder/mcus.json")?;
        let parsed: mcufinder::Mcus = serde_json::from_str(&j)?;

        let mut per_mcu_files = HashMap::<String, Vec<String>>::new();

        for mcu in parsed.mcus {
            let rpn = mcu.rpn;
            let files = mcu.files.into_iter().map(|file| file.file_id);
            per_mcu_files.entry(rpn.to_string()).or_default().extend(files);
        }

        Ok(Self(per_mcu_files))
    }
}

pub struct Docs {
    pub all_mcu_files: AllMcuFiles,
    pub per_mcu_files: PerMcuFiles,
}

impl Docs {
    pub fn parse() -> anyhow::Result<Self> {
        Ok(Self {
            all_mcu_files: AllMcuFiles::parse()?,
            per_mcu_files: PerMcuFiles::parse()?,
        })
    }

    pub fn documents_for(&self, chip_name: &str) -> Vec<stm32_data_serde::chip::Doc> {
        let mut docs: Vec<_> = self
            .per_mcu_files
            .0
            .get(chip_name)
            .into_iter()
            .flatten()
            .flat_map(|id| {
                if let Some(file) = self.all_mcu_files.0.get(id) {
                    let order = order_doc_type(&file.r#type);
                    Some((order, file))
                } else {
                    None
                }
            })
            .collect();
        docs.sort_by_key(|(order, file)| (*order, file.name.clone()));

        docs.into_iter().map(|(_order, file)| file.clone()).collect()
    }
}

fn parse_document_type(t: &str) -> &'static str {
    match t {
        "Reference manual" => "reference_manual",
        "Programming manual" => "programming_manual",
        "Datasheet" => "datahseet", // TODO: fix me
        "Errata sheet" => "errata_sheet",
        "Application note" => "application_note",
        "User manual" => "user_manual",
        _ => panic!("Unknown doc type {t}"),
    }
}

fn order_doc_type(t: &str) -> u8 {
    match t {
        "reference_manual" => 0,
        "programming_manual" => 1,
        "datahseet" => 2, // TODO: fix me
        "errata_sheet" => 3,
        "application_note" => 4,
        _ => panic!("Unknown doc type {t}"),
    }
}
