use std::collections::HashMap;
use std::hash::Hash;

use stm32_data_serde::Chip;
use stm32_data_serde::chip::TempCalibration;

pub fn check(chip: &Chip) {
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
