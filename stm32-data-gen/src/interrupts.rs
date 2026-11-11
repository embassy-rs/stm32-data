use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;
use std::{fmt, thread};

use anyhow::anyhow;
use lazy_regex::regex;
use log::*;

use crate::chips::ChipGroup;
use crate::normalize_peris::normalize_peri_name;
use crate::util::new_regex_map;

mod xml {
    use serde::Deserialize;

    #[derive(Debug, Deserialize, PartialEq)]
    pub struct Ip {
        #[serde(rename = "@Name")]
        pub name: String,
        #[serde(rename = "@Version")]
        pub version: String,
        #[serde(rename = "RefParameter")]
        pub ref_parameters: Vec<RefParameter>,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    pub struct RefParameter {
        #[serde(rename = "@Name")]
        pub name: String,
        #[serde(rename = "PossibleValue", default)]
        pub possible_values: Vec<PossibleValue>,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    pub struct PossibleValue {
        #[serde(rename = "@Comment")]
        pub comment: String,
        #[serde(rename = "@Value")]
        pub value: String,
    }
}

/// One NVIC interrupt entry in the five-field IRQ grammar shared by both
/// producers of these strings:
///
/// ```text
/// {name}_IRQn:{flags}:{f2}:{f3}:{f4}
/// ```
///
/// Exactly five `:`-separated fields; no field may contain `:`.
///
/// * `{name}_IRQn` is the CMSIS interrupt name.
/// * `{flags}` is comma-joined tokens from CubeDB's flag vocabulary:
///   `N`, `W1`, `W2`, `Y`, `H3`, `nHS`, `2V`, `3V`, `nV`, `2V1`, `IF_HAL`,
///   `RTO`, `DMA`, `DMAL0`, `DMAF0`, `DMAL0_DMAMUX`, `DMAF0_DMAMUX`,
///   `DFSDM`, `EXTI` and `DMAMUX`, plus tokens CubeDB adds per chip
///   (`NO_ARG`, `RTOS`, `2H`, `3H` are all observed in real NVIC XMLs).
///   Membership is deliberately not validated here: the vocabulary belongs
///   to CubeDB, and each consumer only tests for the flags it understands.
/// * `f2`..`f4` are flag-dependent:
///   * DMA special format (flags `DMA`, `DMAL0`, `DMAF0`, `DMAL0_DMAMUX`,
///     `DMAF0_DMAMUX`): `f3` is a comma-separated list of DMA instances and
///     `f4` a `;`-separated list of channel ranges (`from` or `from,to`,
///     comma-separated), positionally paired with the `f3` entries. The
///     consumer reads `f3` and `f4`; `f2` is ignored (CubeDB writes `DMA`).
///   * `EXTI` flag: `f2` is a comma-separated list of EXTI line names
///     (e.g. `EXTI0,EXTI1`), consumed as `(EXTI, line)` signals; `f3`
///     carries the HAL handler name and `f4` the line numbers — both are
///     informational for this tool.
///   * general case (e.g. `Y`): `f2` is a comma-separated list of
///     peripheral instance names (an empty `f2` makes the consumer skip the
///     entry), `f3` the HAL handler name(s), `f4` handler argument tags
///     (e.g. `CSSEnabled`); only `f2` is consumed.
///   * system exceptions (`N`, `W1`, ... — `HardFault`, `NonMaskableInt`,
///     ...): the consumer ignores `f2`..`f4` entirely, though CubeDB may
///     still fill them (the NMI entry carries its handler and `CSSEnabled`).
///
/// The two producers of this grammar are: (a) raw CubeDB strings from
/// `<PossibleValue Value=...>` in `sources/cubedb/mcu/IP/NVIC-<chip>_Modes.xml`,
/// loaded by [`ChipInterrupts::parse`]; and (b) `package.rs::build_interrupts`
/// for chips that have no NVIC XML, which emits the degenerate subset
/// `{name}_IRQn:Y|EXTI:{peripherals}::` (single flag, empty `f3`/`f4`) under
/// the same map key — so (b) only ever serves chips where (a) is absent.
///
/// [`Display`] reproduces its input byte-for-byte:
/// `IrqSpec::parse(s)?.to_string() == s` for every well-formed string.
#[derive(Debug, PartialEq, Eq)]
pub struct IrqSpec {
    pub name: String,
    pub flags: Vec<String>,
    pub f2: String,
    pub f3: String,
    pub f4: String,
}

impl IrqSpec {
    /// Parses one raw IRQ string per the grammar documented on [`IrqSpec`].
    /// Returns an error — never panics — on a wrong field count, so one
    /// malformed entry fails its own processing instead of taking down loads.
    pub fn parse(input: &str) -> anyhow::Result<Self> {
        let fields: Vec<&str> = input.split(':').collect();
        if fields.len() != 5 {
            anyhow::bail!(
                "expected 5 colon-separated fields in IRQ string {input:?}, found {}",
                fields.len()
            );
        }
        Ok(Self {
            name: fields[0].to_string(),
            flags: fields[1].split(',').map(str::to_string).collect(),
            f2: fields[2].to_string(),
            f3: fields[3].to_string(),
            f4: fields[4].to_string(),
        })
    }
}

impl fmt::Display for IrqSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}:{}:{}",
            self.name,
            self.flags.join(","),
            self.f2,
            self.f3,
            self.f4
        )
    }
}

// STM32U3 and STM32C5 changed the advanced timer IRQ BRK/TRG names
// to include multiple signal names. This change maps
// them back to the common names.
const REPLACEMENT_IRQS: &[(&str, &str)] = &[
    ("TIM1_BRK_TERR_IERR", "TIM1_BRK"),
    ("TIM8_BRK_TERR_IERR", "TIM8_BRK"),
    ("TIM1_TRG_COM_DIR_IDX", "TIM1_TRG_COM"),
    ("TIM1_TRGI_COM_DIR_IDX", "TIM1_TRG_COM"),
    ("TIM8_TRGI_COM_DIR_IDX", "TIM8_TRG_COM"),
    ("TIM1_UPD", "TIM1_UP"),
    ("TIM8_UPD", "TIM8_UP"),
];

/// Cortex-M system exceptions. They have no peripheral behind them, so the
/// dispatch below deliberately assigns them no signals.
const SYSTEM_EXCEPTIONS: &[&str] = &[
    "NonMaskableInt",
    "HardFault",
    "MemoryManagement",
    "BusFault",
    "UsageFault",
    "SVCall",
    "DebugMonitor",
    "PendSV",
    "SysTick",
];

/// IRQ name -> `(peripheral, signal)` pairs, consulted in place of the generic
/// token-based resolution. First match wins; an empty list means the IRQ is
/// intentionally ignored.
///
/// These sit at one point in the ordered dispatch in [`ChipInterrupts::parse_and_resolve_irqs`],
/// so an entry must not be moved across the flag-driven arms without checking
/// which rule should win.
const IRQ_SIGNAL_OVERRIDES: &[(&str, &[(&str, &str)])] = &[
    // STM32U0 names its FLASH global interrupt FLASH_ECC.
    ("FLASH", &[("FLASH", "GLOBAL")]),
    ("FLASH_ECC", &[("FLASH", "GLOBAL")]),
    // STM32WL CM0+: RCC, FLASH and the CPU1 SEV event share one line.
    ("RCC_FLASH_C1SEV", &[("RCC", "GLOBAL"), ("FLASH", "GLOBAL")]),
    ("CRS", &[("RCC", "CRS")]),
    ("RCC", &[("RCC", "GLOBAL")]),
    ("RNG_CRYP", &[("RNG", "GLOBAL"), ("CRYP", "GLOBAL")]),
    ("WWDG_IWDG", &[("WWDG", "GLOBAL"), ("IWDG", "GLOBAL")]),
    // Intentionally ignored.
    ("RCC_AUDIOSYNC", &[]),
    ("HSEM", &[("HSEM", "GLOBAL")]),
    ("HSEM1", &[("HSEM", "GLOBAL")]),
    ("HSEM2", &[("HSEM", "GLOBAL")]),
    ("HSEM_S", &[("HSEM", "HSEM_S")]),
];

/// `(chip name prefix, IRQ name)` pairs that must never reach the generated
/// interrupt tables.
///
/// F100xE MISC_REMAP remaps some DMA IRQs, so ST decided to give two names to
/// the same IRQ number; `DMA2_Channel4_5` is the one we drop.
const SUPPRESSED_IRQS: &[(&str, &str)] = &[("STM32F100", "DMA2_Channel4_5")];

/// Whether `irq` is suppressed for `chip_name`; see [`SUPPRESSED_IRQS`].
fn is_suppressed_irq(chip_name: &str, irq: &str) -> bool {
    SUPPRESSED_IRQS
        .iter()
        .any(|(chip, name)| chip_name.starts_with(chip) && irq == *name)
}

#[derive(Debug)]
pub struct ChipInterrupts {
    // (nvic name, nvic version) => [raw IRQ strings, parsed lazily by
    // IrqSpec::parse so a single malformed entry cannot break loading]
    pub irqs: HashMap<(String, String), Vec<String>>,
    signals: InterruptSignals,
}

impl ChipInterrupts {
    pub fn parse() -> anyhow::Result<Self> {
        let mut irqs = HashMap::new();

        let mut files: Vec<_> = glob::glob("sources/cubedb/mcu/IP/NVIC*_Modes.xml")?
            .map(Result::unwrap)
            // .filter(|file| !file.to_string_lossy().contains("STM32MP1"))
            .collect();
        files.sort();

        for f in files {
            trace!("parsing {f:?}");

            let file = std::fs::read_to_string(&f)?;
            let parsed: xml::Ip = quick_xml::de::from_str(&file)?;

            let strings: Vec<_> = parsed
                .ref_parameters
                .into_iter()
                .filter(|param| param.name == "IRQn")
                .flat_map(|param| param.possible_values)
                // F3 can remap USB IRQs, ignore them.
                .filter(|irq| !parsed.version.starts_with("STM32F3") || !irq.comment.contains("remap"))
                .map(|irq| irq.value)
                .collect();

            irqs.insert((parsed.name, parsed.version), strings);
        }

        let signals = InterruptSignals::new();

        Ok(Self { irqs, signals })
    }

    pub(crate) fn process(
        &self,
        core: &mut stm32_data_serde::chip::Core,
        chip_name: &str,
        h: &crate::header::ParsedHeader,
        group: &ChipGroup,
    ) -> anyhow::Result<()> {
        let thread_id = thread::current().id();

        trace!(
            "{thread_id:?} parsing interrupts for chip {} core {}",
            chip_name, core.name
        );

        // =================== Populate nvic_priority_bits
        Self::populate_priority_bits(core, h);

        // =================== Populate core interrupts
        Self::populate_core_irqs(core, chip_name, h);

        // =================== Populate peripheral interrupts
        let nvic_strings = self.resolve_nvic_strings(chip_name, core, group)?;
        let chip_signals = self.parse_and_resolve_irqs(chip_name, core, nvic_strings, thread_id)?;
        Self::attach_to_core(core, chip_name, &chip_signals)?;

        Ok(())
    }

    /// Set `core.nvic_priority_bits` from the header's `__NVIC_PRIO_BITS` define.
    fn populate_priority_bits(core: &mut stm32_data_serde::chip::Core, h: &crate::header::ParsedHeader) {
        // With the current data sources, this value is always either 2 or 4, and never resolves to None
        let header_defines = h.get_defines(&core.name);
        core.nvic_priority_bits = header_defines.0.get("__NVIC_PRIO_BITS").map(|bits| *bits as u8);
    }

    /// Populate `core.interrupts` from the CMSIS header IRQs, applying the F100xE MISC_REMAP
    /// removal, the REPLACEMENT_IRQS renames and the U375 PKA patch; sorted by IRQ number.
    fn populate_core_irqs(core: &mut stm32_data_serde::chip::Core, chip_name: &str, h: &crate::header::ParsedHeader) {
        let mut header_irqs = h.get_interrupts(&core.name).clone();
        // F100xE MISC_REMAP remaps some DMA IRQs, so ST decided to give two names
        // to the same IRQ number; see SUPPRESSED_IRQS.
        header_irqs.retain(|irq, _| !is_suppressed_irq(chip_name, irq));

        for (bad, good) in REPLACEMENT_IRQS {
            if let Some(num) = header_irqs.remove(*bad) {
                header_irqs.insert(good.to_string(), num);
            }
        }

        // U375 has PKA but the CubeDB XML and CMSIS header omit it. The peripheral is
        // added in data/extra/STM32U3.yaml. Add its core vector, taken from the U385
        // (same die with PKA present).
        if chip_name.starts_with("STM32U375") {
            header_irqs.entry("PKA".to_string()).or_insert(97);
        }

        core.interrupts = header_irqs
            .iter()
            .map(|(k, v)| stm32_data_serde::chip::core::Interrupt {
                name: k.clone(),
                number: *v,
            })
            .collect();
        core.interrupts.sort_unstable_by_key(|x| x.number);
    }

    /// Pick the NVIC for this chip/core, merge NVIC1's secure IRQs into a TrustZone NVIC2
    /// selection (single-core chips only), then sort and dedup the raw IRQ strings.
    fn resolve_nvic_strings(
        &self,
        chip_name: &str,
        core: &stm32_data_serde::chip::Core,
        group: &ChipGroup,
    ) -> anyhow::Result<Vec<String>> {
        let core_name = &core.name;
        let want_nvic_name = pick_nvic(chip_name, core_name);

        let chip_nvic = group.ips.values().find(|x| x.name == want_nvic_name).ok_or_else(|| {
            anyhow::anyhow!(
                "couldn't find nvic. chip_name={chip_name} core_name={core_name} want_nvic_name={want_nvic_name}"
            )
        })?;

        let mut nvic_strings: Vec<String> = self
            .irqs
            .get(&(chip_nvic.name.clone(), chip_nvic.version.clone()))
            .ok_or_else(|| anyhow::anyhow!("Failed to find NVIC strings for chip {chip_name}"))?
            .clone();

        // For TrustZone single-core chips, pick_nvic() selects NVIC2 (non-secure). However,
        // security peripherals like GTZC have their interrupts defined only in NVIC1 (secure).
        // Include NVIC1 as well so that the interrupt table includes GTZC and other secure IRQs.
        //
        // Intentionally excludes multi-core chips (STM32H7xx, STM32WL5xx) where NVIC1 and
        // NVIC2 belong to different CPU cores and must not be merged.
        if want_nvic_name == "NVIC2"
            && (!chip_name.starts_with("STM32H7")
                || chip_name.starts_with("STM32H7R")
                || chip_name.starts_with("STM32H7S"))
            && !chip_name.starts_with("STM32WL5")
            && let Some(nvic1) = group.ips.values().find(|x| x.name == "NVIC1")
            && let Some(strings) = self.irqs.get(&(nvic1.name.clone(), nvic1.version.clone()))
        {
            nvic_strings.extend(strings.iter().cloned());
        }

        nvic_strings.sort();
        nvic_strings.dedup();

        Ok(nvic_strings)
    }

    /// Parse every raw NVIC IRQ string (see [`IrqSpec`]), fix known name typos, dispatch on
    /// the IRQ flags, and collect peripheral -> signal -> IRQ-name maps.
    fn parse_and_resolve_irqs(
        &self,
        chip_name: &str,
        core: &stm32_data_serde::chip::Core,
        nvic_strings: Vec<String>,
        thread_id: thread::ThreadId,
    ) -> anyhow::Result<HashMap<String, HashMap<String, HashSet<String>>>> {
        // peripheral -> signal -> interrupts
        let mut chip_signals = HashMap::<String, HashMap<String, HashSet<String>>>::new();

        let exists_irq: HashSet<String> = core.interrupts.iter().map(|i| i.name.clone()).collect();

        for i in &exists_irq {
            trace!("  {thread_id:?} irq in header: {i}");
        }

        trace!("  {thread_id:?} chip={chip_name}");
        for nvic_string in nvic_strings {
            trace!("  {thread_id:?} irq={nvic_string:?}");
            let spec = IrqSpec::parse(&nvic_string)?;

            let mut name = spec
                .name
                .strip_suffix("_IRQn")
                .ok_or_else(|| anyhow!("IRQ name {:?} lacks the _IRQn suffix (chip {chip_name})", spec.name))?
                .to_string();

            // Fix typo in STM32Lxx and L083 devices
            let contains_rng = || {
                [&spec.f2, &spec.f3, &spec.f4]
                    .iter()
                    .flat_map(|x| x.split(','))
                    .any(|x| x == "RNG")
            };
            if name == "AES_RNG_LPUART1" && !contains_rng() {
                name = "AES_LPUART1".to_string()
            }

            // More typos
            let name = name
                .replace("USAR11", "USART11")
                // ST NVIC XML typo seen on STM32N6: "Channe1l4" instead of "Channel14"
                .replace("Channe1l4", "Channel14")
                // STM32U3 introduced new naming for advanced timer IRQs
                // that differ from the rest of the STM32 family.
                // Map them back to the common names.
                .replace("TIM1_BRK_TERR_IERR", "TIM1_BRK")
                .replace("TIM1_TRG_COM_DIR_IDX", "TIM1_TRG_COM");
            trace!("    {thread_id:?} name={name}");

            // Skip interrupts that don't exist.
            // This is needed because NVIC files are shared between many chips.
            static EQUIVALENT_IRQS: &[(&str, &[&str])] = &[
                ("HASH_RNG", &["RNG"]),
                ("USB_HP_CAN_TX", &["CAN_TX"]),
                ("USB_LP_CAN_RX0", &["CAN_RX0"]),
                ("TIM6_DAC", &["TIM6", "DAC"]),
                ("DMA1_Ch4_7_DMA2_Ch1_5_DMAMUX_OVR", &["DMA1_Ch4_7_DMAMUX_OVR"]),
                ("USB", &["USB_DRD_FS"]),
            ];
            let mut header_name = name.clone();
            if !exists_irq.contains(&name) {
                let &(_, eq_irqs) = EQUIVALENT_IRQS
                    .iter()
                    .find(|(irq, _)| irq == &name)
                    .unwrap_or(&("", &[]));
                let Some(new_name) = eq_irqs
                    .iter()
                    .chain(
                        [REPLACEMENT_IRQS.iter().find(|(irq, _)| irq == &name)]
                            .iter()
                            .filter_map(|x| x.map(|x| &x.1)),
                    )
                    .find(|i| exists_irq.contains(**i))
                else {
                    trace!("    {thread_id:?} irq missing in C header, ignoring");
                    continue;
                };
                header_name = new_name.to_string();
            }

            // Flags.
            // Y
            //   unknown, it's in all of them
            // H3, nHS
            //   ???
            // 2V, 3V, nV, 2V1
            //   unknown, it has to do with the fact the irq is shared among N peripehrals
            // DMA, DMAL0, DMAF0, DMAL0_DMAMUX, DMAF0_DMAMUX
            //   special format for DMA
            // DFSDM
            //   special format for DFSDM
            // EXTI
            //   special format for EXTI
            let flags: Vec<_> = spec.flags.iter().map(String::as_str).collect();

            // F100xE MISC_REMAP remaps some DMA IRQs, so ST decided to give two names
            // to the same IRQ number; see SUPPRESSED_IRQS.
            if is_suppressed_irq(chip_name, &name) {
                continue;
            }
            //not supported
            if name == "LSECSSD" {
                continue;
            }

            let mut interrupt_signals = HashSet::<(String, String)>::new();
            if SYSTEM_EXCEPTIONS.contains(&name.as_str()) {
                // pass
            } else if flags
                .iter()
                .any(|flag| ["DMA", "DMAL0", "DMAF0", "DMAL0_DMAMUX", "DMAF0_DMAMUX"].contains(flag))
            {
                let mut dmas_iter = spec.f3.split(',');
                let mut chans_iter = spec.f4.split(';');
                for (dma, chan) in std::iter::zip(&mut dmas_iter, &mut chans_iter) {
                    let range = {
                        let mut ch = chan.split(',');
                        let ch_from: usize = ch
                            .next()
                            .ok_or_else(|| anyhow!("missing DMA channel bound in IRQ {nvic_string:?}"))?
                            .parse()?;
                        let ch_to = match ch.next() {
                            Some(ch_to) => ch_to.parse()?,
                            None => ch_from,
                        };
                        if ch.next().is_some() {
                            anyhow::bail!("too many bounds in DMA channel range of IRQ {nvic_string:?}");
                        }
                        ch_from..=ch_to
                    };
                    for ch in range {
                        interrupt_signals.insert((dma.to_string(), format!("CH{ch}")));
                    }
                }
                if dmas_iter.next().is_some() || chans_iter.next().is_some() {
                    anyhow::bail!("DMA instance and channel-range counts differ in IRQ {nvic_string:?}");
                }
            } else if name == "DMAMUX1" || name == "DMAMUX1_S" || name == "DMAMUX_OVR" || name == "DMAMUX1_OVR" {
                interrupt_signals.insert(("DMAMUX1".to_string(), "OVR".to_string()));
            } else if name == "DMAMUX2_OVR" {
                interrupt_signals.insert(("DMAMUX2".to_string(), "OVR".to_string()));
            } else if flags.contains(&"DMAMUX") {
                panic!("should've been handled above");
            } else if flags.contains(&"EXTI") {
                for signal in spec.f2.split(',') {
                    interrupt_signals.insert(("EXTI".to_string(), signal.to_string()));
                }
            } else if let Some(overrides) = IRQ_SIGNAL_OVERRIDES
                .iter()
                .find(|(irq, _)| name.as_str() == *irq)
                .map(|(_, signals)| *signals)
            {
                for (peri, signal) in overrides {
                    interrupt_signals.insert((peri.to_string(), signal.to_string()));
                }
            } else if name.starts_with("HSEM") {
                // Fallback for unexpected Cube NVIC names that still target HSEM.
                interrupt_signals.insert(("HSEM".to_string(), "GLOBAL".to_string()));
            } else {
                if spec.f2.is_empty() {
                    trace!("    {thread_id:?} skipping because spec.f2.is_empty()");
                    continue;
                }

                let peri_names: Vec<_> = spec
                    .f2
                    .split(',')
                    // On STM32N6 the cube NVIC XML attributes the CSI IRQ to the
                    // DCMIPP peripheral (`CSI_IRQn:Y:DCMIPP:HAL_DCMIPP_CSI_IRQHandler:`)
                    // because ST HAL routes it through `HAL_DCMIPP_CSI_IRQHandler`.
                    // The silicon has a distinct CSI block; re-attribute the IRQ
                    // to its owning peripheral so embassy drivers can bind CSI::GLOBAL.
                    .map(|x| if x == "DCMIPP" && name == "CSI" { "CSI" } else { x })
                    .map(|x| {
                        if x == "USB_DRD_FS" || x.starts_with("USBH_HS") {
                            "USB"
                        } else {
                            x
                        }
                    })
                    .map(|x| if x == "XPI1" { "XSPI1" } else { x })
                    .map(|x| if x == "XPI2" { "XSPI2" } else { x })
                    // GTZC_S and GTZC_NS are IP instance names for the GTZC controller in
                    // secure/non-secure context. The interrupt (GTZC_IRQn) belongs to GTZC_TZIC,
                    // which is the sub-peripheral that handles TrustZone violation interrupts.
                    .map(|x| {
                        if x == "GTZC_S" || x == "GTZC_NS" {
                            "GTZC_TZIC"
                        } else {
                            x
                        }
                    })
                    .map(ToString::to_string)
                    .collect();

                trace!("    {thread_id:?} peri_names: {peri_names:?}");

                let is_h5ef = chip_name.starts_with("STM32H5E") || chip_name.starts_with("STM32H5F");
                let name2 = {
                    if name == "USBWakeUp" || name == "USBWakeUp_RMP" {
                        "USB_WKUP"
                    } else if name == "HSP1" {
                        // STM32H7P has a bare HSP1 global IRQ next to HSP1_ERR/EVT/FWEVT.
                        "HSP_GLOBAL"
                    } else if name == "WKUP1_4" {
                        // STM32H7P groups the eight wakeup pins into two PWR interrupts.
                        "PWR_WKUP1_4"
                    } else if name == "WKUP5_8" {
                        "PWR_WKUP5_8"
                    } else if name == "LCD" && is_h5ef {
                        // STM32H5E/F name the LTDC IRQs LCD and LCD_GBL_ERROR
                        "LTDC_LO"
                    } else if name == "LCD_GBL_ERROR" && is_h5ef {
                        "LTDC_ER"
                    } else {
                        name.strip_suffix("_S").unwrap_or(&name)
                    }
                };

                let mut peri_signals: HashMap<_, _> = peri_names
                    .iter()
                    .map(|name| (name.clone(), Vec::<String>::new()))
                    .collect();

                let mut curr_peris = Vec::new();
                if peri_names.len() == 1 {
                    curr_peris = peri_names.clone();
                }

                // Parse IRQ interrupt_signals from the IRQ name.
                for part in tokenize_name(name2) {
                    trace!("    {thread_id:?} part={part}");

                    let part = { if part == "TAMPER" { "TAMP".to_string() } else { part } };

                    if part == "LSECSS" {
                        interrupt_signals.insert(("RCC".to_string(), "LSECSS".to_string()));
                    } else if part == "CSS" {
                        interrupt_signals.insert(("RCC".to_string(), "CSS".to_string()));
                    } else if part == "LSE" {
                        interrupt_signals.insert(("RCC".to_string(), "LSE".to_string()));
                    } else if part == "CRS" {
                        interrupt_signals.insert(("RCC".to_string(), "CRS".to_string()));
                    } else {
                        let pp = match_peris(&peri_names, &part);
                        trace!("    {thread_id:?} part={part}, pp={pp:?}");
                        if !pp.is_empty() {
                            curr_peris = pp;
                        } else {
                            if curr_peris.is_empty() {
                                anyhow::bail!(
                                    "token {part:?} of irq {name:?} matched no peripheral (chip {chip_name}, peri_names={peri_names:?})"
                                );
                            }
                            for p in &curr_peris {
                                peri_signals.entry(p.clone()).or_default().push(part.clone());
                            }
                        }
                    }
                }

                for (p, ss) in peri_signals.into_iter() {
                    let mut ss: Vec<_> = ss.iter().map(|s| s.as_str()).collect();
                    let known = self.signals.valid_signals(&p, chip_name);

                    // If we have no interrupt_signals for the peri, assume it's "global" so assign it all known ones
                    if ss.is_empty() {
                        if p.starts_with("COMP") {
                            ss = vec!["WKUP"];
                        } else {
                            ss = known.keys().copied().collect();
                        }
                    }

                    for s in ss {
                        let Some(sig) = known.get(&s) else {
                            // The Cube NVIC XML can attribute IRQs to a peripheral whose
                            // signal set does not name this line (e.g. the new N6
                            // `IAC_IRQn:Y:RIF:HAL_RIF_IRQHandler:` entry, where RIF only
                            // exposes GLOBAL). Bind it to the peripheral's global line.
                            if let Some(global) = known.get("GLOBAL") {
                                trace!("    {thread_id:?} signal: {} {} (via GLOBAL fallback)", p, global);
                                interrupt_signals.insert((p.clone(), global.to_string()));
                                continue;
                            }
                            panic!("Unknown signal {s} for peri {p} chip {chip_name}, known={known:?}, spec={spec:?}");
                        };
                        trace!("    {thread_id:?} signal: {} {}", p, sig);
                        interrupt_signals.insert((p.clone(), sig.to_string()));
                    }
                }
            }

            for (p, s) in interrupt_signals {
                let p = normalize_peri_name(&p).to_string();
                let signals = chip_signals.entry(p).or_default();
                let irqs = signals.entry(s).or_default();
                irqs.insert(header_name.clone());
            }
        }

        Ok(chip_signals)
    }

    /// Deduplicate each peripheral's signal -> IRQ mapping and write the result into
    /// `core.peripherals`, applying the LTDC, RADIO and HSEM dual-core special cases.
    fn attach_to_core(
        core: &mut stm32_data_serde::chip::Core,
        chip_name: &str,
        chip_signals: &HashMap<String, HashMap<String, HashSet<String>>>,
    ) -> anyhow::Result<()> {
        let core_name = &core.name;
        for p in &mut core.peripherals {
            if let Some(signals) = chip_signals.get(&p.name) {
                let mut all_irqs: Vec<stm32_data_serde::chip::core::peripheral::Interrupt> = Vec::new();

                // remove duplicates
                let _globals = signals.get("GLOBAL").cloned().unwrap_or_default();
                for (signal, irqs) in signals {
                    let mut irqs = irqs.clone();

                    // Special case for LTDC LO/UP signal with both LTDC_LO/UP and LTDC_LO/UP_ERR IRQs
                    if irqs.len() != 1 && p.name == "LTDC" && (signal == "LO" || signal == "UP") {
                        // Prefer the IRQ name that doesn't contain "_ERR"
                        let non_err_irqs: Vec<_> = irqs.iter().filter(|x| !x.contains("_ERR")).cloned().collect();
                        if !non_err_irqs.is_empty() {
                            // If we have non-error IRQs, keep only the first one
                            let preferred_irq = non_err_irqs[0].clone();
                            irqs.clear();
                            irqs.insert(preferred_irq);
                        }
                    }

                    // Another special case for STM32WB0, with RADIO_TXRX & RADIO_TXRX_SEQ. Copy of the LTDC_L0_ERR case.
                    if irqs.len() != 1 && p.name == "RADIO" && signal == "TXRX" {
                        // Prefer the IRQ name that doesn't contain "_SEQ"
                        let non_err_irqs: Vec<_> = irqs.iter().filter(|x| !x.contains("_SEQ")).cloned().collect();
                        if !non_err_irqs.is_empty() {
                            // If we have non-sequence IRQs, keep only the first one
                            let preferred_irq = non_err_irqs[0].clone();
                            irqs.clear();
                            irqs.insert(preferred_irq);
                        }
                    }

                    // Special case for LTDC ER signal on N6, which has multiple error IRQs
                    if irqs.len() != 1 && p.name == "LTDC" && signal == "ER" && chip_name.starts_with("STM32N6") {
                        // Prefer LTDC_LO_ERR
                        let preferred_irq = "LTDC_LO_ERR".to_string();
                        if irqs.contains(&preferred_irq) {
                            irqs.clear();
                            irqs.insert(preferred_irq);
                        }
                    }

                    // If there's a duplicate irqs in a signal other than "global", keep the non-global one.
                    if irqs.len() != 1 && signal != "GLOBAL" {
                        irqs.retain(|irq| !_globals.contains(irq));
                    }

                    // Special case for HSEM on dual-core chips
                    if irqs.len() != 1
                        && p.name == "HSEM"
                        && (core_name == "cm7" || core_name == "cm4" || core_name == "cm0p")
                    {
                        let core_idx = match core_name.as_str() {
                            "cm7" => "1",
                            "cm4" => {
                                if chip_name.starts_with("STM32H7") {
                                    "2"
                                } else {
                                    "1"
                                }
                            }
                            "cm0p" => "2",
                            _ => unreachable!(),
                        };
                        let preferred_irq = format!("HSEM{}", core_idx);
                        if irqs.contains(&preferred_irq) {
                            irqs.clear();
                            irqs.insert(preferred_irq);
                        }
                    }

                    // If there's still duplicate irqs, keep the one that matches the signal name or peri name.
                    // Only match peri name for GLOBAL signals: for specific signals (e.g. ALARM, TAMP) we
                    // want the specific IRQ (e.g. RTC_Alarm, TAMPER) over the generic peri-name one (RTC).
                    if irqs.len() != 1 {
                        let matches: Vec<_> = irqs
                            .iter()
                            .filter(|irq| {
                                (signal == "GLOBAL" && *irq == &p.name)
                                    || **irq == format!("{}_{}", p.name, signal)
                                    || (p.name == "RCC" && signal == "CRS" && *irq == "CRS")
                                    || (**irq == *signal && !irq.ends_with("_S"))
                            })
                            .cloned()
                            .collect();
                        if matches.len() == 1 {
                            irqs.clear();
                            irqs.insert(matches[0].clone());
                        }
                    }

                    // If there's still duplicate irqs, keep the one that matches the peri name.
                    // Only for GLOBAL signals, so specific signals prefer the more specific IRQ name.
                    if irqs.len() != 1 && signal == "GLOBAL" {
                        let matches_peri: Vec<_> = irqs.iter().filter(|irq| *irq == &p.name).cloned().collect();
                        if matches_peri.len() == 1 {
                            irqs.clear();
                            irqs.insert(matches_peri[0].clone());
                        }
                    }

                    // If there's still duplicate irqs, keep the one that doesn't match the peri name.
                    if irqs.len() != 1 && signal != "GLOBAL" {
                        irqs.retain(|irq| irq != &p.name);
                    }

                    if irqs.len() != 1 {
                        panic!(
                            "dup irqs on chip {:?} peri {} signal {}: {:?}",
                            chip_name, p.name, signal, irqs
                        );
                    }

                    for irq in irqs {
                        all_irqs.push(stm32_data_serde::chip::core::peripheral::Interrupt {
                            signal: signal.clone(),
                            interrupt: irq,
                        })
                    }
                }

                all_irqs.sort_by_key(|x| (x.signal.clone(), x.interrupt.clone()));
                all_irqs.dedup_by_key(|x| (x.signal.clone(), x.interrupt.clone()));

                let all_signals: HashSet<String> = all_irqs.iter().map(|i| i.signal.clone()).collect();
                if let Some(registers) = &p.registers
                    && registers.kind == "timer"
                    && (!all_signals.contains("CC") || !all_signals.contains("UP"))
                {
                    return Err(anyhow!("timer {} does not contain CC or UP in {}", p.name, chip_name));
                }

                p.interrupts = all_irqs;
            }
        }
        Ok(())
    }
}

fn tokenize_name(name: &str) -> Vec<String> {
    // Treat IRQ names are "tokens" separated by `_`, except some tokens
    // contain `_` themselves, such as `C1_RX`.
    let r = regex!(
        r"(SPDIF_RX|EP\d+_(IN|OUT)|OTG_FS|OTG_HS|USB_OTG_HS|USB1_OTG_HS|USB2_OTG_HS|USB_DRD_FS|USB_FS|C1_RX|C1_TX|C2_RX|C2_TX|[A-Z0-9]+(_\d+)*)_*"
    );
    let name = name.to_ascii_uppercase();

    r.captures_iter(&name)
        .map(|cap| cap.get(1).unwrap().as_str().to_string())
        .collect()
}

fn match_peris(peris: &[String], name: &str) -> Vec<String> {
    const PERI_OVERRIDE: &[(&str, &[&str])] = &[
        ("USB_FS", &["USB"]),
        ("USB_DRD_FS", &["USB"]),
        ("OTG_HS", &["USB_OTG_HS"]),
        ("OTG_FS", &["USB_OTG_FS"]),
        ("USB", &["USB_DRD_FS"]),
        ("USB_DRD_FS", &["USB"]),
        ("USB1_OTG_HS", &["USB1_OTG_HS"]),
        ("USB2_OTG_HS", &["USB2_OTG_HS"]), // Add this entry to handle USB2_OTG_HS
        ("UCPD1_2", &["UCPD1", "UCPD2"]),
        ("ADC1", &["ADC"]),
        // STM32H7P names the Audio Digital Filter interrupt ADF1_FLT0_IRQn
        // while the peripheral instance is called ADF.
        ("ADF1", &["ADF"]),
        ("CEC", &["HDMI_CEC"]),
        ("SPDIF_RX", &["SPDIFRX1", "SPDIFRX"]),
        ("CAN1", &["CAN"]),
        ("TEMP", &["TEMPSENS"]),
        ("DSI", &["DSIHOST"]),
        ("HRTIM1", &["HRTIM"]),
        ("GTZC", &["GTZC_TZIC"]),
        ("GTZC_S", &["GTZC_TZIC"]),
        ("GTZC_NS", &["GTZC_TZIC"]),
        ("TZIC", &["GTZC_TZIC"]),
        ("TZIC_ILA", &["GTZC_TZIC"]),
        ("ILA", &["ILA"]),
    ];

    let peri_override: HashMap<_, _> = PERI_OVERRIDE.iter().copied().collect();

    if let Some(over) = peri_override.get(name) {
        let mut res = Vec::new();
        for p in *over {
            if peris.contains(&p.to_string()) {
                res.push(p.to_string());
            }
        }
        if !res.is_empty() {
            return res;
        }
    }

    if name == "ILA" || name == "TZIC_ILA" || name == "TZIC" {
        return peris.to_vec();
    }

    if name == "GTZC" || name == "GTZC_S" || name == "GTZC_NS" {
        for p in peris {
            if p == "GTZC_TZIC" {
                return vec![p.clone()];
            }
        }
        for p in peris {
            if p == "GTZC" {
                return vec![p.clone()];
            }
        }
    }

    let mut name = name;
    let mut res = Vec::new();
    if let Some(m) = regex!(r"^(I2C|[A-Z]+)(\d+(_\d+)*)$").captures(name) {
        name = m.get(1).unwrap().as_str();
        for n in m.get(2).unwrap().as_str().split('_') {
            let p = format!("{name}{n}");
            if !peris.contains(&p) {
                return Vec::new();
            }
            res.push(p);
        }
    } else {
        for p in peris {
            if p == name || { p.starts_with(name) && regex!(r"^\d+$").is_match(p.strip_prefix(name).unwrap_or(p)) } {
                res.push(p.to_string());
            }
        }
    }

    res
}

/// Signal name -> canonical signal name, for a single peripheral.
type SignalMap<'a> = HashMap<&'a str, &'a str>;
/// Peripheral name -> its [`SignalMap`].
type PeriSignals<'a> = HashMap<&'a str, SignalMap<'a>>;
/// Peripheral name -> `(chip, signal map)` overrides that apply to one chip.
type ChipSignals<'a> = HashMap<&'a str, Vec<(&'a str, SignalMap<'a>)>>;

#[derive(Debug)]
struct InterruptSignals {
    /// Peripheral -> `(chip, signal map)` overrides that apply to one chip only.
    chip_signals: ChipSignals<'static>,
    signals: PeriSignals<'static>,
    global: HashMap<&'static str, &'static str>,
}

impl InterruptSignals {
    pub fn new() -> Self {
        const EQ_SIGNALS: &[(&str, &[&str])] = &[
            ("UP", &["UPD"]),
            ("TRG", &["TRGI"]),
            ("ER", &["ERR", "TERR", "IERR"]),
            // STM32G5 headers name the HRTIM master interrupt HRTIM1_M_IRQn.
            ("MASTER", &["M"]),
        ];

        // Special chip-specific signal mappings
        // Format: (peripheral_prefix, chip_pattern, signals)
        const CHIP_SPECIFIC_SIGNALS: &[(&str, &str, &[&str])] = &[
            // LTDC signals: STM32N6 has separate LTDC_UP and LTDC_LO interrupts; all others have a single LTDC IRQ
            ("LTDC", "STM32N6", &["ER", "LO", "UP"]),
            // STM32WB0 Specific Mappings
            ("EXTI", "STM32WB0", &["GPIOA", "GPIOB"]),
            (
                "RADIO_TIMER",
                "STM32WB0",
                &["RADIO", "TIMER", "CPU", "WKUP", "ERROR", "TXRX"],
            ),
            ("RADIO", "STM32WB0", &["TXRX", "SEQ"]),
            // STM32G5 GPDMA has per-channel IRQs (GPDMA1_CH0..) but no global one.
            (
                "GPDMA",
                "STM32G5",
                &["CH0", "CH1", "CH2", "CH3", "CH4", "CH5", "CH6", "CH7"],
            ),
            // STM32H7P GPDMA1 has 16 channels, each with its own interrupt.
            (
                "GPDMA",
                "STM32H7P",
                &[
                    "CH0", "CH1", "CH2", "CH3", "CH4", "CH5", "CH6", "CH7", "CH8", "CH9", "CH10", "CH11", "CH12",
                    "CH13", "CH14", "CH15",
                ],
            ),
            // STM32H7P HPDMA1 has 16 channels, each with its own interrupt.
            (
                "HPDMA",
                "STM32H7P",
                &[
                    "CH0", "CH1", "CH2", "CH3", "CH4", "CH5", "CH6", "CH7", "CH8", "CH9", "CH10", "CH11", "CH12",
                    "CH13", "CH14", "CH15",
                ],
            ),
        ];

        // Fallback to the general signal map
        const IRQ_SIGNALS_MAP: &[(&str, &[&str])] = &[
            ("CAN", &["TX", "RX0", "RX1", "SCE"]),
            // DCMIPP: only the GLOBAL interrupt belongs here. The separate CSI IRQ
            // (STM32N6) is attributed to the CSI peripheral instead.
            ("DCMIPP", &["GLOBAL"]),
            ("FDCAN", &["IT0", "IT1", "CAL"]),
            ("I2C", &["ER", "EV"]),
            ("I3C", &["ER", "EV", "WKUP"]),
            ("FMPI2C", &["ER", "EV"]),
            ("TIM", &["ER", "BRK", "UP", "TRG", "COM", "CC", "DIR", "IDX"]),
            // ("HRTIM", &["Master", "TIMA", "TIMB", "TIMC", "TIMD", "TIME", "TIMF"]),
            ("RTC", &["ALARM", "WKUP", "TAMP", "STAMP", "SSRU", "TIMESTAMP"]),
            ("SUBGHZ", &["RADIO"]),
            (
                "IPCC",
                &["C1_RX", "C1_TX", "C2_RX", "C2_TX", "RX0", "TX0", "RX1", "TX1"],
            ),
            (
                "HRTIM",
                &["MASTER", "TIMA", "TIMB", "TIMC", "TIMD", "TIME", "TIMF", "FLT"],
            ),
            ("COMP", &["WKUP", "ACQ"]),
            ("RCC", &["RCC", "CRS", "WAKEUP"]),
            ("MDIOS", &["GLOBAL", "WKUP"]),
            ("ETH", &["GLOBAL", "WKUP", "LPI"]),
            (
                "DFSDM",
                &["FLT0", "FLT1", "FLT2", "FLT3", "FLT4", "FLT5", "FLT6", "FLT7"],
            ),
            ("MDF", &["FLT0", "FLT1", "FLT2", "FLT3", "FLT4", "FLT5", "FLT6", "FLT7"]),
            (
                "PWR",
                &["S3WU", "WKUP", "WKUP1_4", "WKUP5_8", "PVD", "AVD", "WAKEUP", "PIN"],
            ),
            ("GTZC", &["GLOBAL", "ILA", "GTZC"]),
            ("GTZC_TZIC", &["GLOBAL", "ILA", "GTZC"]),
            ("GTZC_MPCBB", &["GLOBAL", "ILA", "GTZC"]),
            ("WWDG", &["GLOBAL", "RST"]),
            ("USB_OTG_FS", &["GLOBAL", "EP1_OUT", "EP1_IN", "WKUP"]),
            ("USB_OTG_HS", &["GLOBAL", "EP1_OUT", "EP1_IN", "WKUP", "OTG"]),
            ("USB1_OTG_HS", &["GLOBAL", "EP1_OUT", "EP1_IN", "WKUP"]),
            ("USB2_OTG_HS", &["GLOBAL", "EP1_OUT", "EP1_IN", "WKUP"]),
            ("USB", &["LP", "HP", "WKUP", "USBH", "OHCI", "EHCI"]),
            ("GPU2D", &["GLOBAL", "ER"]),
            ("SAI", &["A", "B"]),
            ("ADF", &["FLT0"]),
            ("HSP", &["GLOBAL", "ERR", "EVT", "FWEVT"]),
            ("NPU", &["IT0", "IT1", "IT2"]),
            ("RAMECC", &["ECC"]),
            ("RAMCFG", &["BKP", "ECC"]),
            ("LTDC", &["ER", "LO"]),
            ("HSEM", &["GLOBAL"]),
            ("LPDMA", &["CH0", "CH1", "CH2", "CH3", "CH4", "CH5", "CH6", "CH7"]),
        ];

        let eq_signals: HashMap<&'static str, &'static [&'static str]> = EQ_SIGNALS.iter().cloned().collect();

        let collect_signals = |signals: &[&'static str]| -> HashMap<&'static str, &'static str> {
            signals
                .iter()
                .map(|s| (*s, *s))
                .chain(
                    signals
                        .iter()
                        .filter_map(|s| Some((*eq_signals.get(*s)?, *s)))
                        .flat_map(|(k, v)| k.iter().map(move |k| (*k, v))),
                )
                .collect()
        };

        let mut chip_signals: ChipSignals<'_> = HashMap::new();

        for (peri, (chip, signals)) in CHIP_SPECIFIC_SIGNALS
            .iter()
            .filter(|(_, chip, _)| {
                if *chip == "*" {
                    panic!("use IRQ_SIGNALS_MAP for generic signals")
                } else {
                    true
                }
            })
            .map(|(peri, chip, signals)| (*peri, (*chip, collect_signals(signals))))
        {
            chip_signals.entry(peri).or_default().push((chip, signals));
        }

        Self {
            chip_signals,
            signals: IRQ_SIGNALS_MAP
                .iter()
                .map(|(peri, signals)| (*peri, collect_signals(signals)))
                .collect(),
            global: HashMap::from([("GLOBAL", "GLOBAL")]),
        }
    }

    pub fn valid_signals(&self, peri: &str, chip_name: &str) -> &HashMap<&str, &str> {
        let peri = trim_trailing_digits(peri);

        self.chip_signals
            .get(peri)
            .into_iter()
            .flatten()
            .find_map(|(chip, signals)| {
                if chip_name.starts_with(chip) {
                    Some(signals)
                } else {
                    None
                }
            })
            .unwrap_or(self.signals.get(peri).unwrap_or(&self.global))
    }
}

fn trim_trailing_digits(input: &str) -> &str {
    let new_len = input
        .char_indices()
        .rev()
        .find(|&(_, c)| !c.is_ascii_digit()) // stop at first non-digit
        .map(|(idx, c)| idx + c.len_utf8()) // keep full char
        .unwrap_or(0); // all digits case

    &input[..new_len]
}

fn pick_nvic(chip_name: &str, core_name: &str) -> String {
    static PICK_NVIC: LazyLock<regex_map::RegexMap<&str>> = LazyLock::new(|| {
        new_regex_map([
            // Exception 1: Multicore: NVIC1 is the first core, NVIC2 is the second. We have to pick the right one.
            ("STM32H7(45|47|55|57).*:cm7", "NVIC1"),
            ("STM32H7(45|47|55|57).*:cm4", "NVIC2"),
            ("STM32WL5.*:cm4", "NVIC1"),
            ("STM32WL5.*:cm0p", "NVIC2"),
            // Exception 2: TrustZone: NVIC1 is Secure mode, NVIC2 is NonSecure mode. For now, we pick the NonSecure one.
            (
                "STM32(L5|U3|U5|H5[2-7EF]|WBA5[245]|WBA6[2345]|WBA6M|WBA2[35]).*",
                "NVIC2",
            ),
            // Exception 3: NVICs are split for "bootloader" and "application", not sure what that means?
            ("STM32H7[RS].*", "NVIC2"),
            // Exception 4: NVICS are split for bootloader NVIC, secure NVIC1 and non-secure NVIC2.
            ("STM32N6.*", "NVIC2"),
            // catch-all: Most chips have a single NVIC, named "NVIC"
            (".*", "NVIC"),
        ])
    });

    PICK_NVIC
        .get(&format!("{chip_name}:{core_name}"))
        .next()
        .unwrap()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::IrqSpec;

    // Verbatim `<PossibleValue Value=...>` samples from
    // sources/cubedb/mcu/IP/NVIC-STM32C031_Modes.xml.
    const C031_IRQ_SAMPLES: &[&str] = &[
        // Empty fields (system exception).
        "HardFault_IRQn:N,W1:::",
        // NMI carrying handler name (f3) and attribute (f4).
        "NonMaskableInt_IRQn:N,IF_HAL::HAL_RCC_NMI_IRQHandler:CSSEnabled",
        // EXTI kind: f2 = EXTI lines, f4 = line numbers.
        "EXTI0_1_IRQn:Y,EXTI:EXTI0,EXTI1:HAL_GPIO_EXTI_IRQHandler:0,1",
        // Multi-peripheral f2.
        "SPI1_IRQn:Y:SPI1,I2S1::",
        // DMA flag: f3 = DMA instances, f4 = channel ranges.
        "DMA1_Channel2_3_IRQn:Y,DMAL0:DMA:DMA1:2,3",
        // Handler list in f3, argument tags in f4.
        "RTC_IRQn:Y,3H:RTC:HAL_RTC_AlarmIRQHandler,HAL_RTCEx_WakeUpTimerIRQHandler,HAL_RTCEx_TimeStampIRQHandler,HAL_RTCEx_TamperIRQHandler:Alarm,WakeUp,Timestamp,Tamper",
    ];

    #[test]
    fn irq_spec_parse_display_round_trips_c031_samples() {
        for sample in C031_IRQ_SAMPLES {
            let spec = IrqSpec::parse(sample).unwrap_or_else(|e| panic!("{sample:?}: {e}"));
            assert_eq!(spec.to_string(), *sample);
        }
    }

    #[test]
    fn irq_spec_splits_name_flags_and_three_fields() {
        let spec = IrqSpec::parse("DMA1_Channel2_3_IRQn:Y,DMAL0:DMA:DMA1:2,3").unwrap();
        assert_eq!(
            spec,
            IrqSpec {
                name: "DMA1_Channel2_3_IRQn".to_string(),
                flags: vec!["Y".to_string(), "DMAL0".to_string()],
                f2: "DMA".to_string(),
                f3: "DMA1".to_string(),
                f4: "2,3".to_string(),
            }
        );

        let spec = IrqSpec::parse("EXTI0_1_IRQn:Y,EXTI:EXTI0,EXTI1:HAL_GPIO_EXTI_IRQHandler:0,1").unwrap();
        assert_eq!(spec.flags, ["Y", "EXTI"]);
        assert_eq!(spec.f2, "EXTI0,EXTI1");
    }

    #[test]
    fn irq_spec_parse_rejects_wrong_field_counts() {
        assert!(IrqSpec::parse("").is_err());
        assert!(IrqSpec::parse("TIM1_IRQn:Y:TIM1").is_err());
        assert!(IrqSpec::parse("TIM1_IRQn:Y:TIM1:::extra").is_err());
    }
}
