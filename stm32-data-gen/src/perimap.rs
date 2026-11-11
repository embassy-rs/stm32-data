//! Maps `chip:peripheral:ip-version` keys to the register block that describes
//! the peripheral.
//!
//! The mapping itself lives in `data/perimap.yaml`; see that file for the
//! matching and ordering rules.
//!
//! This lookup is the single hottest operation in chip generation (there is one
//! per peripheral per core), so [`Perimap`] does not simply hand every key to a
//! `RegexSet` over all ~830 patterns. Instead each pattern is indexed by the
//! first bytes of its longest literal run: a pattern can only match a key that
//! contains all of its literals, which narrows the table down to the handful of
//! patterns that can possibly match, and only those regexes are evaluated.

use std::collections::HashMap;

use regex::Regex;

use crate::config::{self, PerimapEntry};

/// How many bytes of a pattern's longest literal form the index key.
const PREFIX: usize = 4;

/// The register block that backs a peripheral.
#[derive(Debug, Clone)]
pub struct PerimapBlock {
    /// Register file kind, e.g. `usart`. Selects `data/registers/<kind>_<version>.yaml`.
    pub kind: String,
    /// Register file version, e.g. `v1`.
    pub version: String,
    /// Block name within that register file, e.g. `USART`.
    pub block: String,
}

/// First-match-wins lookup from `{chip}:{peripheral}:{ip_version}` to a block.
pub struct Perimap {
    /// Patterns in `data/perimap.yaml` order.
    patterns: Vec<Regex>,
    /// Literal substrings every match of the corresponding pattern contains.
    literals: Vec<Vec<Vec<u8>>>,
    /// Pattern indices bucketed by the first [`PREFIX`] bytes of their longest
    /// literal. A pattern appears in at most one bucket.
    by_prefix: HashMap<[u8; PREFIX], Vec<usize>>,
    /// Patterns whose longest literal is too short to bucket; always checked.
    unindexed: Vec<usize>,
    blocks: Vec<PerimapBlock>,
}

impl Perimap {
    pub fn new() -> Self {
        let entries: &[PerimapEntry] = &config::PERIMAP;

        let mut patterns = Vec::with_capacity(entries.len());
        let mut literals = Vec::with_capacity(entries.len());
        let mut blocks = Vec::with_capacity(entries.len());
        let mut by_prefix: HashMap<[u8; PREFIX], Vec<usize>> = HashMap::new();
        let mut unindexed = Vec::new();

        for (i, entry) in entries.iter().enumerate() {
            // Anchored exactly like the `regex_map`-based lookup this replaces,
            // so `|` at the top level of a pattern keeps its old meaning.
            let pattern = format!("^{}$", entry.pattern);
            let re = Regex::new(&pattern)
                .unwrap_or_else(|e| panic!("bad pattern {:?} in data/perimap.yaml: {e}", entry.pattern));

            let lits = literal_runs(&entry.pattern);
            // Bucket by the longest literal that contains a `:`. Keys are
            // colon-separated, so those are far more selective than a bare
            // `STM32...` chip prefix.
            let bucket = lits
                .iter()
                .filter(|l| l.contains(&b':'))
                .max_by_key(|l| l.len())
                .or_else(|| lits.iter().max_by_key(|l| l.len()));
            match bucket {
                Some(longest) if longest.len() >= PREFIX => {
                    let mut key = [0u8; PREFIX];
                    key.copy_from_slice(&longest[..PREFIX]);
                    by_prefix.entry(key).or_default().push(i);
                }
                _ => unindexed.push(i),
            }

            patterns.push(re);
            literals.push(lits);
            blocks.push(PerimapBlock {
                kind: entry.kind.clone(),
                version: entry.version.clone(),
                block: entry.block.clone(),
            });
        }

        Self {
            patterns,
            literals,
            by_prefix,
            unindexed,
            blocks,
        }
    }

    /// Find the register block for `{chip}:{peripheral}:{ip_version}`.
    ///
    /// Entries are tried in `data/perimap.yaml` order and the first match wins.
    pub fn get(&self, key: &str) -> Option<&PerimapBlock> {
        let bytes = key.as_bytes();

        // A pattern can only match if all of its literals occur in the key, so
        // scanning the key for indexed literals leaves very few candidates.
        let mut candidates = self.unindexed.clone();
        if bytes.len() >= PREFIX {
            for at in 0..=bytes.len() - PREFIX {
                let Ok(prefix) = <[u8; PREFIX]>::try_from(&bytes[at..at + PREFIX]) else {
                    continue;
                };
                let Some(indexed) = self.by_prefix.get(&prefix) else {
                    continue;
                };
                for &i in indexed {
                    if self.literals[i].iter().all(|lit| memmem(bytes, lit)) && !candidates.contains(&i) {
                        candidates.push(i);
                    }
                }
            }
        }

        candidates.sort_unstable();
        candidates
            .into_iter()
            .find(|&i| self.patterns[i].is_match(key))
            .map(|i| &self.blocks[i])
    }
}

fn memmem(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.len() > haystack.len() {
        return false;
    }
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// Extract the literal substrings that every match of `pattern` contains.
///
/// Only runs outside of parentheses are returned: a group may be an alternation
/// such as `([HU]5|WBA)`, where each side contributes a different literal and
/// neither is guaranteed. Likewise a pattern with a top-level `|` yields no
/// runs at all. That keeps the result conservative: a pattern whose runs are
/// not all present in a key can never match that key.
fn literal_runs(pattern: &str) -> Vec<Vec<u8>> {
    let b = pattern.as_bytes();
    let mut runs: Vec<Vec<u8>> = Vec::new();
    let mut run = 0usize;
    let mut i = 0usize;
    let mut depth = 0usize;
    let mut top_level_alternative = false;

    // Keep the literal `b[start..end]`, if there is one.
    macro_rules! push_run {
        ($start:expr, $end:expr) => {
            if depth == 0 && $end > $start {
                runs.push(b[$start..$end].to_vec());
            }
        };
    }

    macro_rules! end_run {
        ($end:expr) => {
            push_run!(run, $end)
        };
    }

    // A quantifier (`*`, `+`, `?`, `{n,m}`) applies to the one literal
    // character in front of it, so that character may be absent from a
    // matching string. Everything before it is still guaranteed.
    macro_rules! end_run_before_quantifier {
        ($end:expr) => {
            push_run!(run, $end.saturating_sub(1))
        };
    }

    while i < b.len() {
        match b[i] {
            // Escapes contribute nothing: `\d` is a class, and even for a
            // punctuation escape like `\.` we do not bother tracking it.
            b'\\' => {
                end_run!(i);
                i = (i + 2).min(b.len());
                run = i;
            }
            // Character class `[...]` (with a leading `^`, and a `]` in first
            // position being a literal).
            b'[' => {
                end_run!(i);
                i += 1;
                if i < b.len() && b[i] == b'^' {
                    i += 1;
                }
                if i < b.len() && b[i] == b']' {
                    i += 1;
                }
                while i < b.len() && b[i] != b']' {
                    if b[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
                i = (i + 1).min(b.len());
                run = i;
            }
            // Quantifier `{n,m}`.
            b'{' => {
                end_run_before_quantifier!(i);
                while i < b.len() && b[i] != b'}' {
                    i += 1;
                }
                i = (i + 1).min(b.len());
                run = i;
            }
            // Group opening, possibly `(?P<name>`, `(?<name>`, `(?:`, `(?=`, ...
            b'(' => {
                end_run!(i);
                depth += 1;
                i += 1;
                if i < b.len() && b[i] == b'?' {
                    i += 1;
                    if i < b.len() && (b[i] == b'P' || b[i] == b'<') {
                        while i < b.len() && b[i] != b'>' {
                            i += 1;
                        }
                        i = (i + 1).min(b.len());
                    } else {
                        while i < b.len() && matches!(b[i], b'=' | b'!' | b':' | b'<') {
                            i += 1;
                        }
                    }
                }
                run = i;
            }
            b')' => {
                end_run!(i);
                depth = depth.saturating_sub(1);
                i += 1;
                run = i;
            }
            b'|' => {
                end_run!(i);
                if depth == 0 {
                    top_level_alternative = true;
                }
                i += 1;
                run = i;
            }
            // Quantifiers: the atom before them is optional.
            b'*' | b'+' | b'?' => {
                end_run_before_quantifier!(i);
                i += 1;
                run = i;
            }
            // Zero-width anchors and the any-character wildcard break a run
            // without making anything optional.
            b'^' | b'$' | b'.' => {
                end_run!(i);
                i += 1;
                run = i;
            }
            _ => i += 1,
        }
    }
    end_run!(i);

    if top_level_alternative {
        // Alternation at the top level: each side contributes a different
        // literal, so nothing is guaranteed.
        runs.clear();
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_conservative_literal_runs() {
        assert_eq!(
            literal_runs("STM32H503.*:GTZC:.*"),
            [b"STM32H503".to_vec(), b":GTZC:".to_vec()]
        );
        assert_eq!(
            literal_runs(".*:ADC\\d*_COMMON:.*"),
            [b":ADC".to_vec(), b"_COMMON:".to_vec()]
        );
        // `([HU]5|WBA)` is an alternation, so neither `5` nor `WBA` is guaranteed.
        assert_eq!(
            literal_runs("STM32([HU]5|WBA).*:SAI\\d?:.*"),
            [b"STM32".to_vec(), b":SAI".to_vec(), b":".to_vec()]
        );
        // A top-level alternation guarantees nothing.
        assert_eq!(literal_runs("STM32A|STM32B"), Vec::<Vec<u8>>::new());
        assert_eq!(literal_runs(".*"), Vec::<Vec<u8>>::new());

        // A quantifier makes the character in front of it optional.
        assert_eq!(
            literal_runs("STM32[CGHUW].*:HRTIM1?:.*"),
            [b"STM32".to_vec(), b":HRTIM".to_vec(), b":".to_vec()]
        );
        assert_eq!(literal_runs("a*bc?d"), [b"b".to_vec(), b"d".to_vec()]);
        assert_eq!(literal_runs("x\\d?y"), [b"x".to_vec(), b"y".to_vec()]);
        // `e{2,3}` may be absent, but the characters before it are not.
        assert_eq!(literal_runs("pre{2,3}post"), [b"pr".to_vec(), b"post".to_vec()]);
    }

    /// The generator resolves `data/`, `sources/` and `build/` relative to the
    /// repository root, so tests need to run from there too.
    fn chdir_repo_root() {
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
            std::env::set_current_dir(root).unwrap();
        });
    }

    /// The index must agree with scanning every pattern in order.
    #[test]
    fn indexed_lookup_matches_naive_scan() {
        chdir_repo_root();
        let entries: &[PerimapEntry] = &config::PERIMAP;
        let naive: Vec<Regex> = entries
            .iter()
            .map(|e| Regex::new(&format!("^{}$", e.pattern)).unwrap())
            .collect();
        let perimap = Perimap::new();

        let mut keys = Vec::new();
        for family in [
            "STM32H503",
            "STM32H523",
            "STM32F407",
            "STM32L476",
            "STM32G431",
            "STM32C011",
            "STM32N657",
            "STM32U575",
            "STM32WBA52",
            "STM32F103",
            "STM32H743",
            "STM32H7A3",
            "STM32WL55",
            "STM32WB55",
            "STM32U031",
            "STM32G071",
            "STM32L072",
            "STM32F030",
            "STM32L152",
            "STM32F207",
            "STM32F767",
            "STM32MP157",
            "STM32H7RS",
            "STM32U375",
        ] {
            for peri in [
                "TIM1",
                "TIM2",
                "USART1",
                "UART4",
                "LPUART1",
                "GTZC",
                "GTZC_TZSC",
                "GTZC_MPCBB1",
                "ADC1_COMMON",
                "ADC12_COMMON",
                "DFSDM1",
                "DMA1",
                "DMA2",
                "BDMA",
                "GPDMA1",
                "RCC",
                "FDCAN1",
                "FDCANRAM1",
                "SAI1",
                "SAI2",
                "SPI2",
                "RNG",
                "AES",
                "USB",
                "COMP1",
                "OPAMP1",
                "DAC1",
                "RTC",
                "I2C1",
                "SYSCFG",
            ] {
                for version in [
                    "usart:sci2_v1_1",
                    "usart:sci3_v3_0",
                    "TIM1:tim1_v1_0",
                    "gtzc:tzsc1_v2_2_rc1_Mustang4M",
                    "gtzc:v1",
                    "dfsdm1:v1_0_4ch",
                    "STM32G4_dma_v1_0",
                    "adc1_v1_0",
                    "bsec2_v1",
                    "STM32N6_rif_v1",
                    "dac:dac1_v1_0",
                    "spi:spi1_v1_0",
                ] {
                    keys.push(format!("{family}:{peri}:{version}"));
                }
            }
        }

        let mut mismatches = Vec::new();
        for key in &keys {
            let expected = naive.iter().position(|r| r.is_match(key)).map(|i| {
                (
                    entries[i].kind.as_str(),
                    entries[i].version.as_str(),
                    entries[i].block.as_str(),
                )
            });
            let got = perimap
                .get(key)
                .map(|b| (b.kind.as_str(), b.version.as_str(), b.block.as_str()));
            if got != expected {
                mismatches.push(format!("{key}: expected {expected:?}, got {got:?}"));
            }
        }
        assert!(
            mismatches.is_empty(),
            "{} of {} keys disagree with the naive scan:\n{}",
            mismatches.len(),
            keys.len(),
            mismatches.iter().take(20).cloned().collect::<Vec<_>>().join("\n")
        );
    }
}
