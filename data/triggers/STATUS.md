# Trigger data provenance status

Last full pass: 2026-09 (data-driven rewrite of the former hand-written
`PERIPHERAL_TRIGGER_RULES` table). This file records what was verified
against which source, what could not be verified, and what to re-check when
new reference-manual (RM) material becomes available. See also
`CONVENTIONS.md` (naming rules) and `RM_PARSING_NOTES.md` (extraction intel).

## Data sources

- **SVD** — `sources/svd/` (stm32-data-sources submodule), extracted by
  `stm32-data-gen/src/bin/trigger-svd-extract.rs` (dev tool, writes
  diagnostic fragments to `tmp/svd_fragments/`).
- **RM** — ST reference manuals, markdown extraction under `tmp/rm/`
  (gitignored; not vendored). The committed raw fragments are the source of
  truth; re-extraction is a manual refresh step.

Where SVD and RM disagree, the RM wins (known SVD copy-paste errors:
e.g. F100 TIM8/TIM5, L0 TIM15, G0 TIM2_CCx, WL ADC TIM1_TRGO vs TRGO2).

## Fully RM-verified in this pass

F1 DAC (RM0008/RM0041/RM0401), F1 ADC (RM0008, matches SVD), F3 DAC all
groups (RM0364/0365/0366/0313/0316), F0 DAC + F0 ADC (RM0091/RM0360),
L0 DAC + L0 ADC + L0 COMP→TIM (RM0367/0376/0377/0451), L1 DAC (RM0038),
L4(5|6) DAC (RM0351 T125), L4(7-A) DFSDM (RM0351), L5 DFSDM + breaks
(RM0438 T209/210), H7 DAC + H7 DFSDM H743/745 groups (RM0399/RM0433),
G4 ADC + DAC incl. sawtooth muxes + TIM1 COMP inputs (RM0440 — see
"Pending work": the rest of RM0440's timer interconnect tables are NOT
extracted), G0 ADC (RM0454
T57; RM0444 truncated but interconnect matrix agrees), C0 ADC (RM0490 T68),
U5 DAC + U5 ADC (RM0456 T342/T307/T308), WL DAC + WL ADC + WL COMP→TIM
(RM0453), WB ADC (RM0434 T92/93), WBA5 all + WBA6 TIM1/LPTIM/ADC4
(RM0493/RM0515), U0 DAC + TIM1 COMP (RM0503).
SVD-derived and RM-spot-checked: F4 ADC (+ F401/411/410 split), F7 ADC (new).

## Not verified — truncated RM extractions on disk

The following raw fragments keep hand-rule content (or content derived from
CMSIS headers / RM interconnect-matrix sections, as noted in each file's
header). Re-verify when the missing chapters are added to `tmp/rm/`:

| RM (folder in tmp/rm)  | Missing chapters                                           | Affected rules                                                        |
| ---------------------- | ---------------------------------------------------------- | --------------------------------------------------------------------- |
| RM0394 (STM32L41x-46x) | truncated at ch. 2 — ADC and DFSDM chapters absent         | 0069-0072 (L4(1-6) DFSDM/TIM), L4(1-6) ADC in 0081a                   |
| RM0432 (STM32L4+)      | truncated at ch. 5 — ADC and DFSDM chapters absent         | 0076-0081 (L4+ DFSDM/TIM), L4+ ADC in 0081a                           |
| RM0481 (STM32H5)       | truncated at ch. 15 — DAC (ch. 28) and ADC (ch. 26) absent | 0031-0033 (H5 DAC), 0041 (H5 ADC)                                     |
| RM0487 (STM32U3)       | truncated at ch. 15 — ADC (ch. 23) and DAC (ch. 24) absent | U3 part of 0029 (shares U5 indices), U3 ADC not extracted             |
| RM0444 (STM32G0x1)     | truncated at ch. 13 — DAC/ADC chapters absent              | 0034 (G0 DAC per-index; source set corroborated by §9.3.4)            |
| RM0515 (STM32WBA6)     | truncated at ch. 36 — I2C/SPI/USART chapters absent        | 0055-0057, 0058, 0063-0064 (cross-checked vs identical RM0493 tables) |
| RM0399 (STM32H745/755) | truncated at ch. 30 — DFSDM chapter absent                 | 0082-0087 verified via RM0433 instead (same DFSDM IP)                 |
| RM0376 (STM32L0x2)     | truncated after ch. 20                                     | none (rules verified via RM0377/RM0367)                               |

## Not verified — RM not on disk at all

- **H723/733/725/735/730 line** (RM0468 not in the archive): 0026 (DAC),
  0116-0121 (DFSDM/TIM).
- **H7A3/B3/B0** (RM0455 not in the archive): 0027 (DAC), 0110-0115
  (DFSDM/TIM).
- **MP1** (out of scope by decision): 0104-0109 kept from original hand
  rules, unverified.

## Pending work — RM tables never extracted (no rule exists)

The rewrite pass was **rule-driven**: work items were derived from the
existing rule set, and verification was scoped to rules that exist. RM
tables that never had a corresponding hand rule were never examined, so
"RM-verified" above means "the existing rules check out", not "everything
in the RM is captured". Confirmed instance:

- **G4 timer interconnects (RM0440) — DONE (2026-09 follow-up).** All ten
  G4 timers now have interconnect rules (`0047`-`0047i`): TIM1/8/20 full
  sets (TI1/ITR/ETR/BRK/BRK2/SYS_BRK/OCREF_CLR), TIM2-5 TI1-TI4/ITR/ETR/
  OCREF_CLR, TIM15-17 TI1/TI2/ITR/BRK/SYS_BRK/OCREF_CLR. TIM2-5 TI tables
  (T287-290) needed reconciliation against the ch. 11 matrix (Tables 77-80)
  due to a column-collapse artifact in the markdown — see the 0047c-f file
  headers. Per-die trimming (e.g. HRTIM sources on G431-class dies without
  HRTIM1) is intentionally not done in the data: per CONVENTIONS.md it
  belongs in stm32-data-gen, which does not implement it yet.

The same blind spot likely exists for other families whose RMs contain
interconnect matrices that were never transcribed into hand rules
(candidates to audit: U5/U3 (RM0456/RM0487 interconnect chapters), H5
(RM0481 §15.3), L4/L4+/L5 (RM0351/RM0432/RM0438 — note RM0432 is
truncated on disk), H7 (RM0399/RM0433), F3 (RM0316)). G4 was confirmed
and extracted in the 2026-09 follow-up. Before extracting more, decide
per family whether embassy-stm32 actually consumes `TIMX_*` signals
there (it does for WBA and G4, which is why those rules exist).

## Known structural anomalies

- `0052` (`^STM32WBA6[245].*:TIM1`) effectively covers only WBA624/644
  (first-match-wins with `0051`); TIM4_TRGO is unreachable on WBA622/625.
- F101/F103 DAC rules carry TIM8/TIM5 sources because RM0008's table lists
  them, although F101 has no TIM8/TIM5 peripherals.
- Rule `0035` (G0 ADC) also matches G0x0 chips, but G0x0 has no TIM2 —
  TIM2_TRGO@ADC_EXT_TRG2 should be excluded for G030/G050/G070 (needs an
  `^STM32G0[357]0.*`-style split rule; deferred).
- ADC common blocks (`ADC123_COMMON`, `ADC12_COMMON`, `ADC1_COMMON`, ...)
  no longer receive triggers: all loose `:ADC.*` peripheral patterns were
  narrowed to `:ADC(\d+)?` (2026-09 follow-up).
- cubedb omits COMP2 on WBA62/65 (the SVDs and RM0515 have it); the COMP2
  rows are kept.

## Refresh checklist when a missing RM arrives

1. Unpack the markdown under `tmp/rm/`, read `RM_PARSING_NOTES.md`.
2. Re-verify the affected fragments (table numbers are cited in the
   per-file headers), edit `data/triggers/raw/`.
3. `cargo run --release --bin trigger-rules-build`, then a full
   `cargo run --release --bin stm32-data-gen` (its validation catches
   duplicate/ill-formed entries).
