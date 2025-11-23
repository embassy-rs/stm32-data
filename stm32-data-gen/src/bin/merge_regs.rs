use std::cmp::Ordering;
use std::path::PathBuf;
use std::{env, fs};

use glob::{GlobError, glob};
use serde_yaml::{Mapping, Value};

/// Extracts byte_offset from a YAML value as an integer
fn get_byte_offset(item: &Value) -> Option<i64> {
    item.get("byte_offset").and_then(|v| v.as_i64())
}

/// Extracts bit_offset from a YAML value as an integer
fn get_bit_offset(item: &Value) -> Option<i64> {
    item.get("bit_offset").and_then(|v| v.as_i64())
}

/// Compares two items by byte_offset for sorting
fn item_key_cmp(a: &Value, b: &Value) -> Ordering {
    let a_offset = get_byte_offset(a).unwrap_or(0);
    let b_offset = get_byte_offset(b).unwrap_or(0);
    a_offset.cmp(&b_offset)
}

/// Compares two fields by bit_offset for sorting
fn field_key_cmp(a: &Value, b: &Value) -> Ordering {
    let a_offset = get_bit_offset(a).unwrap_or(0);
    let b_offset = get_bit_offset(b).unwrap_or(0);
    a_offset.cmp(&b_offset)
}

/// Checks if two items are the same (by name and byte_offset)
fn items_equal(a: &Value, b: &Value) -> bool {
    let a_name = a.get("name").and_then(|v| v.as_str());
    let b_name = b.get("name").and_then(|v| v.as_str());
    let a_offset = get_byte_offset(a);
    let b_offset = get_byte_offset(b);

    a_name.is_some() && b_name.is_some() && a_name == b_name && a_offset == b_offset
}

/// Checks if two fields are the same (by name and bit_offset)
fn fields_equal(a: &Value, b: &Value) -> bool {
    let a_name = a.get("name").and_then(|v| v.as_str());
    let b_name = b.get("name").and_then(|v| v.as_str());
    let a_offset = get_bit_offset(a);
    let b_offset = get_bit_offset(b);

    a_name.is_some() && b_name.is_some() && a_name == b_name && a_offset == b_offset
}

/// Merges new block items into origin, avoiding duplicates
fn merge_block(origin: &mut Vec<Value>, new: Vec<Value>) {
    for newval in new {
        let mut found = false;
        for val in origin.iter() {
            if items_equal(val, &newval) {
                found = true;
                break;
            }
        }
        if !found {
            origin.push(newval);
        }
    }
    origin.sort_by(item_key_cmp);
}

/// Merges new field items into origin, avoiding duplicates
fn merge_fields(origin: &mut Vec<Value>, new: Vec<Value>) {
    for newval in new {
        let mut found = false;
        for val in origin.iter() {
            if fields_equal(val, &newval) {
                found = true;
                break;
            }
        }
        if !found {
            origin.push(newval);
        }
    }
    origin.sort_by(field_key_cmp);
}

/// Recursively merges new dictionary into origin
fn merge_dicts(origin: &mut Value, new: Value) {
    match (origin, new) {
        (Value::Mapping(origin_map), Value::Mapping(new_map)) => {
            for (k, v) in new_map.iter() {
                if origin_map.contains_key(k) {
                    match (origin_map.get_mut(k).unwrap(), v) {
                        (Value::Mapping(_), Value::Mapping(_)) => {
                            merge_dicts(origin_map.get_mut(k).unwrap(), v.clone());
                        }
                        (Value::Sequence(origin_seq), Value::Sequence(new_seq)) => {
                            let key_str = k.as_str().unwrap_or("");
                            if key_str == "items" {
                                merge_block(origin_seq, new_seq.clone());
                            } else if key_str == "fields" {
                                merge_fields(origin_seq, new_seq.clone());
                            }
                        }
                        (origin_val, new_val) => {
                            *origin_val = new_val.clone();
                        }
                    }
                } else {
                    origin_map.insert(k.clone(), v.clone());
                }
            }
        }
        _ => {}
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    let mut reg_map = Value::Mapping(Mapping::new());

    // Process each register file from command line arguments
    for regfile in args[1..]
        .iter()
        .filter_map(|p| glob(p).ok())
        .flatten()
        .filter_map(|p| p.ok())
    {
        println!("Loading {}", regfile.to_str().unwrap_or(""));

        match fs::read_to_string(&regfile) {
            Ok(content) => match serde_yaml::from_str::<Value>(&content) {
                Ok(y) => {
                    merge_dicts(&mut reg_map, y);
                }
                Err(e) => {
                    eprintln!("Error parsing YAML from {}: {}", regfile.to_str().unwrap_or(""), e);
                }
            },
            Err(e) => {
                eprintln!("Error reading file {}: {}", regfile.to_str().unwrap_or(""), e);
            }
        }
    }

    // Write merged result to file
    match serde_yaml::to_string(&reg_map) {
        Ok(yaml_output) => match fs::write("regs_merged.yaml", yaml_output) {
            Ok(_) => {
                println!("Successfully wrote merged registers to regs_merged.yaml");
            }
            Err(e) => {
                eprintln!("Error writing to regs_merged.yaml: {}", e);
            }
        },
        Err(e) => {
            eprintln!("Error converting to YAML: {}", e);
        }
    }
}
