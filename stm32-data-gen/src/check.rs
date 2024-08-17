use std::collections::HashMap;
use std::hash::Hash;

use stm32_data_serde::Chip;

pub fn check(chip: &Chip) {
    for core in &chip.cores {
        let peris = mapify(&core.peripherals, |p| &p.name);
        for ch in &core.dma_channels {
            let dma = peris.get(&ch.dma).unwrap();
            let signal = ch.name.strip_prefix(&format!("{}_", dma.name)).unwrap();
            if dma.interrupts.iter().find(|i| i.signal == signal).is_none() {
                panic!("{}: missing irq for ch {}", chip.name, ch.name);
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
