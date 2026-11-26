use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use stm32_data_serde::Chip;
use stm32_data_serde::chip::TempCalibration;

pub fn check(chip: &Chip) {
    match &chip.power {
        Some(power) => {
            assert!(!power.modes.is_empty(), "{}: power.modes is empty", chip.name);
            let mut seen = HashSet::new();
            for mode in &power.modes {
                assert!(
                    seen.insert(mode.kind),
                    "{}: duplicate power mode {:?}",
                    chip.name,
                    mode.kind
                );
            }
        }
        None => {
            assert!(
                chip.name.starts_with("STM32MP1"),
                "{}: missing chip power descriptor in data/low_power.yaml",
                chip.name
            );
        }
    }

    for core in &chip.cores {
        let peris = mapify(&core.peripherals, |p| &p.name);
        // Each channel should have its own interrupt signal, except for MDMA, which uses one GLOBAL interrupt
        for ch in core.dma_channels.iter().filter(|ch| !ch.name.starts_with("MDMA")) {
            let dma = peris.get(&ch.dma).unwrap();
            let signal = ch.name.strip_prefix(&format!("{}_", dma.name)).unwrap();
            if !dma.interrupts.iter().any(|i| i.signal == signal) {
                panic!("{}: missing irq for ch {}", chip.name, ch.name);
            }
        }

        // Peripherals sharing an RCC enable bit must agree on their Stop-mode capability
        // (historically flagged in embassy-stm32/build.rs as a refcount-leak hazard).
        let mut by_enable: HashMap<(&str, &str), (&str, _)> = HashMap::new();
        for p in &core.peripherals {
            let Some(rcc) = &p.rcc else { continue };
            let key = (rcc.enable.register.as_str(), rcc.enable.field.as_str());
            if let Some((prev_name, prev_modes)) = by_enable.get(&key) {
                assert_eq!(
                    *prev_modes, rcc.low_power.modes,
                    "{}: peripherals {} and {} share RCC enable {}.{} with conflicting Stop modes ({:?} vs {:?})",
                    chip.name, prev_name, p.name, key.0, key.1, prev_modes, rcc.low_power.modes
                );
            } else {
                by_enable.insert(key, (&p.name, rcc.low_power.modes));
            }
        }

        let has_vrefintcal = peris.contains_key(&"VREFINTCAL".to_string());
        let has_tscal1 = peris.contains_key(&"TS_CAL1".to_string());
        let has_tscal2 = peris.contains_key(&"TS_CAL2".to_string());

        match &chip.adc_calibration {
            None => {
                assert!(
                    !has_vrefintcal && !has_tscal1 && !has_tscal2,
                    "{}: has calibration peripherals but adc_calibration is None",
                    chip.name
                );
            }
            Some(cal) => {
                assert_eq!(
                    cal.vrefint.is_some(),
                    has_vrefintcal,
                    "{}: adc_calibration.vrefint mismatch with VREFINTCAL peripheral",
                    chip.name
                );
                match &cal.temp {
                    Some(TempCalibration::TwoPoint(_)) => {
                        assert!(
                            has_tscal1 && has_tscal2,
                            "{}: TwoPoint temp calibration requires TS_CAL1 and TS_CAL2",
                            chip.name
                        );
                    }
                    Some(TempCalibration::OnePoint(_)) => {
                        assert!(
                            has_tscal1 && !has_tscal2,
                            "{}: OnePoint temp calibration requires TS_CAL1 without TS_CAL2",
                            chip.name
                        );
                    }
                    Some(TempCalibration::Nominal(_)) | None => {
                        assert!(
                            !has_tscal1 && !has_tscal2,
                            "{}: Nominal/None temp calibration must not have TS_CAL1/TS_CAL2",
                            chip.name
                        );
                    }
                }
            }
        }
    }
}

fn mapify<K: Eq + Hash, V>(iter: impl IntoIterator<Item = V>, f: impl Fn(&V) -> K) -> HashMap<K, V> {
    let mut res = HashMap::new();
    for v in iter {
        let k = f(&v);
        res.insert(k, v);
    }
    res
}
