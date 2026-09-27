//! Extract ADC/DAC trigger enumerated values from the SVD files in sources/svd.
//!
//! Dev tool: writes one diagnostic fragment per (svd, peripheral, register,
//! field) to tmp/svd_fragments/, with each enumerated value normalized to the
//! trigger `source` string convention used in data/triggers/. The fragments
//! are the ground truth used to (re)write data/triggers/raw/ rule fragments;
//! they are not consumed by stm32-data-gen itself.
//!
//! Field mapping:
//!   ADC EXTSEL   -> signal ADC_EXT_TRG{n}
//!   ADC JEXTSEL  -> signal ADC_JEXT_TRG{n}
//!   DAC TSELx    -> signal DAC_CHX_TRG{n}

use std::path::Path;

use regex::Regex;
use serde::Serialize;

#[derive(Debug, Serialize)]
struct Fragment {
    svd: String,
    peripheral: String,
    register: String,
    field: String,
    values: Vec<Value>,
}

#[derive(Debug, Clone, Serialize)]
struct Value {
    value: u32,
    name: String,
    desc: String,
    /// Normalized trigger source strings (more than one for combined entries
    /// like "EXTI line 11/TIM8_TRGO"). Empty for skipped values
    /// (software trigger, reserved, ...).
    sources: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum State {
    /// inside <peripheral>, waiting for its <name>
    PeripheralName,
    /// inside <peripheral> body
    Peripheral,
    /// inside <register>, waiting for its <name>
    RegisterName,
    /// inside <register> body (fields, ...)
    Register,
    /// inside <field>, waiting for its <name>
    FieldName,
    /// inside <field> body, possibly inside <enumeratedValues>
    Field,
}

fn main() {
    let out_dir = Path::new("tmp/svd_fragments");
    let _ = std::fs::remove_dir_all(out_dir);
    std::fs::create_dir_all(out_dir).unwrap();

    let mut entries: Vec<_> = std::fs::read_dir("sources/svd")
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("svd"))
        .collect();
    entries.sort();

    let mut count = 0;
    for path in entries {
        let stem = path.file_stem().unwrap().to_string_lossy().to_string();
        let text = std::fs::read_to_string(&path).unwrap();
        let fragments = extract_svd(&stem, &text);
        for frag in &fragments {
            let yaml = serde_yaml::to_string(&frag).unwrap();
            let fname = format!("{}__{}_{}_{}.yaml", stem, frag.peripheral, frag.register, frag.field);
            std::fs::write(out_dir.join(fname), yaml).unwrap();
            count += 1;
        }
    }
    println!("wrote {count} fragments to {}", out_dir.display());
}

fn field_signal_prefix(field: &str) -> Option<&'static str> {
    match field {
        "EXTSEL" => Some("ADC_EXT_TRG"),
        "JEXTSEL" => Some("ADC_JEXT_TRG"),
        f if f.starts_with("TSEL") && f[4..].chars().all(|c| c.is_ascii_digit()) => Some("DAC_CHX_TRG"),
        _ => None,
    }
}

fn extract_svd(stem: &str, text: &str) -> Vec<Fragment> {
    let mut frags: Vec<Fragment> = Vec::new();
    let mut state = State::Peripheral;
    let mut peripheral = String::new();
    let mut register = String::new();
    let mut field = String::new();
    let mut in_enums = false;
    let mut in_desc = false;
    let mut cur: Option<Value> = None;
    let mut values: Vec<Value> = Vec::new();

    for line in text.lines() {
        let t = line.trim();

        if t.starts_with("<peripheral ") || t == "<peripheral>" {
            state = State::PeripheralName;
            peripheral.clear();
            continue;
        }
        if t.starts_with("<register ") || t == "<register>" {
            state = State::RegisterName;
            register.clear();
            continue;
        }
        if t == "<field>" || t.starts_with("<field ") {
            state = State::FieldName;
            field.clear();
            continue;
        }

        if state == State::PeripheralName || state == State::RegisterName || state == State::FieldName {
            if let Some(name) = t.strip_prefix("<name>") {
                let name = name.strip_suffix("</name>").unwrap_or(name).trim().to_string();
                match state {
                    State::PeripheralName => {
                        peripheral = name;
                        state = State::Peripheral;
                    }
                    State::RegisterName => {
                        register = name;
                        state = State::Register;
                    }
                    State::FieldName => {
                        field = name;
                        state = State::Field;
                    }
                    _ => unreachable!(),
                }
                continue;
            }
        }

        if t.starts_with("<enumeratedValues") && !t.ends_with("/>") && field_signal_prefix(&field).is_some() {
            in_enums = true;
            values.clear();
            continue;
        }
        if t.starts_with("</enumeratedValues>") {
            in_enums = false;
            in_desc = false;
            cur = None;
            if !values.is_empty() {
                frags.push(Fragment {
                    svd: stem.to_string(),
                    peripheral: peripheral.clone(),
                    register: register.clone(),
                    field: field.clone(),
                    values: std::mem::take(&mut values),
                });
            }
            continue;
        }

        if in_enums {
            if t.starts_with("<enumeratedValue>") || t.starts_with("<enumeratedValue ") {
                cur = Some(Value {
                    value: 0,
                    name: String::new(),
                    desc: String::new(),
                    sources: Vec::new(),
                });
                continue;
            }
            if t.starts_with("</enumeratedValue>") {
                if let Some(v) = cur.take() {
                    values.push(v);
                }
                continue;
            }
            let Some(v) = &mut cur else { continue };
            if in_desc {
                if let Some(rest) = t.strip_suffix("</description>") {
                    v.desc.push(' ');
                    v.desc.push_str(rest);
                    in_desc = false;
                } else {
                    v.desc.push(' ');
                    v.desc.push_str(t);
                }
                continue;
            }
            if let Some(d) = t.strip_prefix("<description>") {
                if let Some(rest) = d.strip_suffix("</description>") {
                    v.desc = rest.trim().to_string();
                } else {
                    v.desc = d.trim().to_string();
                    in_desc = true;
                }
                continue;
            }
            if let Some(val) = t.strip_prefix("<value>") {
                let val = val.strip_suffix("</value>").unwrap_or(val).trim();
                v.value = parse_svd_int(val);
                continue;
            }
            if t.starts_with("<name>") {
                let n = t.trim_start_matches("<name>").trim_end_matches("</name>").trim();
                v.name = n.to_string();
                continue;
            }
        }
    }

    // normalize
    for f in &mut frags {
        for v in &mut f.values {
            v.sources = normalize(&v.desc, &v.name, cc_style(stem));
        }
    }
    frags
}

fn parse_svd_int(s: &str) -> u32 {
    let s = s.trim();
    if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u32::from_str_radix(h, 16).unwrap_or(0)
    } else if let Some(b) = s.strip_prefix("0b").or_else(|| s.strip_prefix("0B")) {
        u32::from_str_radix(b, 2).unwrap_or(0)
    } else if let Some(h) = s.strip_prefix("#") {
        u32::from_str_radix(h, 16).unwrap_or(0)
    } else {
        s.parse().unwrap_or(0)
    }
}

/// Per-family TIMx_CCy vs TIMx_CHy output convention, matching the historical
/// hand rules.
#[derive(Clone, Copy, PartialEq)]
enum CcStyle {
    Cc,
    Ch,
}

fn cc_style(stem: &str) -> CcStyle {
    if stem.starts_with("stm32f1") || stem.starts_with("stm32f4") {
        CcStyle::Ch
    } else {
        CcStyle::Cc
    }
}

/// Normalize an enumerated value (description preferred, enum name as
/// fallback) to one or more trigger source strings.
fn normalize(desc: &str, name: &str, style: CcStyle) -> Vec<String> {
    let out = normalize_one(desc, style, true);
    if out.is_empty() {
        normalize_one(name, style, false)
    } else {
        out
    }
}

fn normalize_one(text: &str, style: CcStyle, is_desc: bool) -> Vec<String> {
    let ws = Regex::new(r"\s+").unwrap();
    let mut s = ws.replace_all(text.trim(), " ").to_string();

    // "Timer N TRGO(N)": must run before the parenthetical-note strip, which
    // would otherwise eat the "(N)".
    if let Some(caps) = Regex::new(r"^Timer (\d+) TRGO\((\d)\)$").unwrap().captures(&s) {
        return vec![format!("TIM{}_TRGO{}", &caps[1], &caps[2])];
    }
    // combined entries: "EXTI line 11/TIM8_TRGO event (...)" -> EXTI + TIM8_TRGO
    if let Some(caps) = Regex::new(r"(?i)^EXTI line\s?(\d+)\s*/\s*(.+)$").unwrap().captures(&s) {
        let mut out = vec![format!("EXTI{}_TRG", &caps[1])];
        out.extend(normalize_one(&caps[2], style, is_desc));
        return out;
    }
    // strip trailing parenthetical notes and trailing "event" / "event trigger..."
    let paren = Regex::new(r"\s*\([^)]*\)\s*$").unwrap();
    while paren.is_match(&s) {
        s = paren.replace(&s, "").trim().to_string();
    }
    let event = Regex::new(r"(?i)\s+event( trigger.*)?$").unwrap();
    s = event.replace(&s, "").trim().to_string();

    if Regex::new(r"(?i)^(software|swstart|jswstart|swtrig\d*|reserved|none|external pin)$")
        .unwrap()
        .is_match(&s)
    {
        return Vec::new();
    }

    // already-canonical forms
    if let Some(caps) = Regex::new(r"^TIM(\d+)_TRGO2?$").unwrap().captures(&s) {
        return vec![caps[0].to_string()];
    }
    if let Some(caps) = Regex::new(r"^TIM(\d+)_(CC|CH|OC)(\d+)$").unwrap().captures(&s) {
        let (n, kind, m) = (caps[1].to_string(), caps[2].to_string(), caps[3].to_string());
        return vec![format_tim_cc(&n, &kind, &m, style)];
    }
    if let Some(caps) = Regex::new(r"^EXTI_?LINE\s?(\d+)$").unwrap().captures(&s) {
        return vec![format!("EXTI{}_TRG", &caps[1])];
    }
    if let Some(caps) = Regex::new(r"^EXTI(\d+)$").unwrap().captures(&s) {
        return vec![format!("EXTI{}_TRG", &caps[1])];
    }
    if let Some(caps) = Regex::new(r"^LPTIM(\d+)(_(TRGO|OUT|CH\d+))$").unwrap().captures(&s) {
        return vec![format!("LPTIM{}{}", &caps[1], &caps[2])];
    }
    if Regex::new(r"(?i)^LPTIM\d*_?OUT$").unwrap().is_match(&s) {
        return vec!["LPTIMOUT".to_string()];
    }
    if let Some(caps) = Regex::new(r"(?i)^HRTIM(\d+)?_?ADCTRG\s?(\d+)$").unwrap().captures(&s) {
        return vec![format!("HRTIM_ADC_TRG{}", &caps[2])];
    }
    if let Some(caps) = Regex::new(r"(?i)^HRTIM_?DAC_?TRG\s?(\d+)$").unwrap().captures(&s) {
        return vec![format!("HRTIM_DAC_TRG{}", &caps[1])];
    }

    // "Timer N CCx" / "Timer N CHx" / "Timer N TRGO" / "Timer N TRGO(2)"
    if let Some(caps) = Regex::new(r"^Timer (\d+) (CC|CH)(\d+)$").unwrap().captures(&s) {
        return vec![format_tim_cc(&caps[1], &caps[2], &caps[3], style)];
    }
    if let Some(caps) = Regex::new(r"^Timer (\d+) TRGO\(?(\d)?\)?$").unwrap().captures(&s) {
        let two = caps.get(2).map(|m| m.as_str()).unwrap_or("");
        return vec![format!("TIM{}_TRGO{}", &caps[1], two)];
    }
    if let Some(caps) = Regex::new(r"^EXTI line\s?(\d+)$").unwrap().captures(&s) {
        return vec![format!("EXTI{}_TRG", &caps[1])];
    }

    // enum-name forms
    if let Some(caps) = Regex::new(r"^Tim(\d+)(Cc|Ch)(\d+)$").unwrap().captures(&s) {
        return vec![format_tim_cc(&caps[1], &caps[2].to_uppercase(), &caps[3], style)];
    }
    if let Some(caps) = Regex::new(r"^TIM(\d+)(CC\d|CH\d|OC\d|TRGO2?)$").unwrap().captures(&s) {
        return vec![format_tim_name(&caps[1], &caps[2], style)];
    }
    if let Some(caps) = Regex::new(r"^Tim(\d+)(Trgo2?|Oc\d|Ch\d|Cc\d)$").unwrap().captures(&s) {
        return vec![format_tim_name(&caps[1], &caps[2].to_uppercase(), style)];
    }

    // unknown but canonical-looking all-caps token
    if is_desc
        && s.contains('_')
        && s.chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
    {
        return vec![s];
    }
    Vec::new()
}

fn format_tim_cc(n: &str, kind: &str, m: &str, style: CcStyle) -> String {
    let kind = if style == CcStyle::Ch && kind == "CC" {
        "CH"
    } else if style == CcStyle::Cc && kind == "CH" {
        "CC"
    } else {
        kind
    };
    format!("TIM{n}_{kind}{m}")
}

fn format_tim_name(n: &str, what: &str, style: CcStyle) -> String {
    let what = if style == CcStyle::Ch {
        Regex::new(r"^CC(\d)$").unwrap().replace(what, "CH$1").to_string()
    } else {
        Regex::new(r"^CH(\d)$").unwrap().replace(what, "CC$1").to_string()
    };
    format!("TIM{n}_{what}")
}
