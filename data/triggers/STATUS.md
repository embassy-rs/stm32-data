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
e.g. F100 TIM8/TIM5, L0 TIM15, G0 TIM2_CCx, WL ADC TIM1_TRGO vs TRGO2,
and the H7 ADC EXTSEL enums — see 0130's header: every H7 SVD carries a
byte-identical EXTSEL enum, even on dies with no HRTIM and no TIM23/24).

## Fully RM-verified in this pass

F1 DAC (RM0008/RM0041/RM0401), F1 ADC (RM0008, matches SVD), F3 DAC all
groups (RM0364/0365/0366/0313/0316), F0 DAC + F0 ADC (RM0091/RM0360),
L0 DAC + L0 ADC + L0 COMP→TIM (RM0367/0376/0377/0451), L1 DAC (RM0038),
L4(5|6) DAC (RM0351 T125; die-pruned per RM0394 T1), L4(7-A) DFSDM (RM0351), L5 DFSDM + breaks
(RM0438 T209/210), H7 DAC + H7 DFSDM H743/745 groups (RM0399/RM0433),
G4 ADC + DAC incl. sawtooth muxes + TIM1 COMP inputs (RM0440 — see
"Pending work": the rest of RM0440's timer interconnect tables are NOT
extracted), G0 ADC (RM0454
T57; RM0444 truncated but interconnect matrix agrees), C0 ADC (RM0490 T68),
U5 DAC + U5 ADC (RM0456 T342/T307/T308), WL DAC + WL ADC + WL COMP→TIM
(RM0453), WB ADC (RM0434 T92/93), WBA5 all + WBA6 TIM1/LPTIM/ADC4
(RM0493/RM0515), U0 DAC + TIM1 COMP (RM0503), **H7 ADC all five lines**
(RM0399 T214/215 H745/747/755/757; RM0433 T210/211 H742/743/750/753;
RM0468 T226/227 + T242 H723/733/725/735/730; RM0477 T229 H7R3/R7/S3/S7 —
rules 0128-0130, 0132; added for embassy-rs/stm32-data#945).
SVD-derived and RM-spot-checked: F4 ADC (+ F401/411/410 split), F7 ADC (new).
SVD-derived only (RM ADC chapter not extracted): H7A3/B3/B0 ADC (0131, from
stm32h7b3 — RM0455 folder truncated at ch. 6; enum spot-checked against the
RM0433/RM0399 tables, HRTIM rows dropped as no die on that line has HRTIM).

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

- **H723/733/725/735/730 line** (RM0468 was not in the archive when 0026
  and 0116-0121 were written; the folder has since been added and its ADC
  chapters are extracted — its ADC rule 0130 IS RM-verified, and 0026 (DAC)
  has since been re-checked against RM0468 ch. 31: the HRTIM rows were
  dropped because the RM table has no trg9/trg10; trg4/trg7 are still
  untranscribed, see Known structural anomalies. The DFSDM/TIM rules
  0116-0121 have not been re-checked against it).
- **H7A3/B3/B0** (RM0455 folder truncated at ch. 6 — no ADC/DAC/DFSDM/TIM
  chapters on disk): 0027 (DAC), 0110-0115 (DFSDM/TIM), and 0131 (ADC) is
  SVD-derived, see its header.
- **MP1** (out of scope by decision): 0104-0109 kept from original hand
  rules, unverified.

## Pending work — RM tables never extracted (no rule exists)

The rewrite pass was **rule-driven**: work items were derived from the
existing rule set, and verification was scoped to rules that exist. RM
tables that never had a corresponding hand rule were never examined, so
"RM-verified" above means "the existing rules check out", not "everything
in the RM is captured". Confirmed instance:

- **H7 ADC trigger tables — DONE.** `rules.yaml` had **no** rule matching
  any `STM32H7*:ADC*` key before this pass (embassy-rs/stm32-data#945:
  H755 timers could not trigger its ADCs at all). All five H7 lines now have
  rules 0128-0132; see "Fully RM-verified" above for provenance per line.
- **G4 timer interconnects (RM0440) — DONE (2026-09 follow-up).** All ten
  G4 timers now have interconnect rules (`0047`-`0047i`): TIM1/8/20 full
  sets (TI1/ITR/ETR/BRK/BRK2/SYS_BRK/OCREF_CLR), TIM2-5 TI1-TI4/ITR/ETR/
  OCREF_CLR, TIM15-17 TI1/TI2/ITR/BRK/SYS_BRK/OCREF_CLR. TIM2-5 TI tables
  (T287-290) needed reconciliation against the ch. 11 matrix (Tables 77-80)
  due to a column-collapse artifact in the markdown — see the 0047c-f file
  headers. Per-die filtering (e.g. HRTIM sources on G431-class dies
  without HRTIM1) is intentionally not done in the data: per CONVENTIONS.md
  it belongs in stm32-data-gen, which now implements it (see "Per-die
  trimming in generated data" below).

The same blind spot likely exists for other families whose RMs contain
interconnect matrices that were never transcribed into hand rules
(candidates to audit: U5/U3 (RM0456/RM0487 interconnect chapters), H5
(RM0481 §15.3), L4/L4+/L5 (RM0351/RM0432/RM0438 — note RM0432 is
truncated on disk), H7 (RM0399/RM0433), F3 (RM0316)). G4 was confirmed
and extracted in the 2026-09 follow-up. Before extracting more, decide
per family whether embassy-stm32 actually consumes `TIMX_*` signals
there (it does for WBA and G4, which is why those rules exist).

## Per-die trimming in generated data

`stm32-data-gen` implements the filtering that CONVENTIONS.md assigns to the
consumer generator:

- `trigger::source_origin()` classifies every source: system-level events
  (`PVD`, `CORTEX_M4_LOCKUP`, ...), exact peripheral names (`TIM1_TRGO` ->
  `TIM1`, `EXTI11_TRG` -> `EXTI`, `RTC_*` -> `RTC`), peripheral-name
  prefixes for sources that omit the instance number (`HRTIM*` -> `HRTIM1`,
  `TAMP*`, `GPDMA*`, `LPTIMOUT` -> any `LPTIM*`), and multi-candidate
  sources (`COMP1_OUT_OR_COMP2_OUT`, `USB_SOF_SYNC` -> `USB`/`USB_OTG_FS`/
  `USB_OTG_HS`). An unrecognized spelling makes `Triggers::new()` panic, so
  a new fragment with a new source must be classified explicitly instead of
  silently escaping the filter.
- In `process_core`, after `apply_extras`, each peripheral's `triggers`
  list is retained against the chip's final peripheral map — the emitted
  JSON can therefore never advertise a trigger whose originating
  peripheral is absent from the same JSON.

Validation of the switch-on, against a full pre-switch dump of all
`(chip, peripheral, signal, source)` tuples: the regeneration removed
exactly **14,385** tuples across **498 chip variants** and added none; an
exhaustive re-check found 0 over-trims (removed although the peripheral
exists), 0 under-trims (kept although it is absent) and 0 dropped
system-level sources; every hardcoded `triggers::X` reference in
embassy-stm32's example directories still resolves on its target chip;
`cargo test -p stm32-data-gen` passes. Largest groups, each cross-checked
against the RM product tables cited above: G4 12,226 (HRTIM outside
G474/G484, TIM20/TIM5/ADC4-5/COMP5-7 outside the dies RM0440 T2 grants
them), L4 1,002 (DFSDM1 only on L45x/46x, and no TIM4/TIM8 anywhere in
L41-46, per RM0394 T1), F1 288 (TIM1/TIM8 absent on F101, TIM4 absent on
die sizes that lack it, per RM0008), then H7 286, G0 153, L0 149,
U5/U3/U0 99, WBA 72, F0 44, H5 40, F3 26.

A companion audit found **19 rows that are dead on every chip their rule
matches** (CONVENTIONS rule 1) and removed them from six fragments;
`trigger-rules-build` plus a full regeneration then produced byte-identical
output (they had already been trimmed at generation): `TIM8_TRGO` in 0006
(F101 — the CONVENTIONS example), TIM4/5/7/8 in 0013 (L45x/46x, RM0394 T1),
TIM3/TIM6 in 0024a's `L0(31|41)` rule (RM0377 T2: Category 2 = L031/L041
has neither timer), `HRTIM_DAC_TRG1/2` in 0026 (RM0468 ch. 31 has no
trg9/trg10) and in 0027 (RM0455 has no HRTIM chapter), and TIM4/TIM8 in
0081a's `L4(1-6)` rule (RM0394 T1).

## Known structural anomalies

- `0052` (`^STM32WBA6[245].*:TIM1`) effectively covers only WBA624/644
  (first-match-wins with `0051`); TIM4_TRGO is unreachable on WBA622/625.
- F101/F103 DAC rules inherited TIM8/TIM5 rows from RM0008's table. Split
  by availability in the trim pass: F101 has no TIM8 at any density (RM0008
  ch. 14 grants TIM8 only to high/XL-density F103; stm32f101xb.h/
  stm32f101xe.h and cubedb agree), so 0006's `TIM8_TRGO` row was removed
  outright; high-density F101 does have TIM5 (stm32f101xe.h), so 0006's
  `TIM5_TRGO` row stays and survives trimming on the 15 F101 dies that have
  a DAC1. 0007 (F103) is unchanged: only high-density F103 has DAC1 and
  those dies have both TIM5 and TIM8.
- Rule `0035` (G0 ADC) also matches G0x0 chips, but G0x0 has no TIM2
  (stm32g030xx.h and cubedb). No split rule is needed anymore: per-die
  trimming drops TIM2_TRGO@ADC_EXT_TRG2 for G030/G050/G070 at generation,
  while the row stays in the data per the no-trimming-in-data rule.
- ADC common blocks (`ADC123_COMMON`, `ADC12_COMMON`, `ADC1_COMMON`, ...)
  no longer receive triggers: all loose `:ADC.*` peripheral patterns were
  narrowed to `:ADC(\d+)?` (2026-09 follow-up).
- cubedb omits COMP2 on WBA62/65 (the SVDs and RM0515 have it); the COMP2
  rows are kept in the data. Generation now trims `COMP2_OUT` on exactly
  the four variants whose cubedb XMLs lack the Ip (WBA62CG/CI, WBA65CG/CI;
  the other WBA62/65 variants have COMP2 and keep their rows), so the JSON
  stays self-consistent — a cubedb fix upstream restores them with no rule
  change.
- **DFSDM-break rules: the trailing `.*` in the `:TIM1.*` match patterns
  swallows `:TIM15/16/17` keys.** `new_regex_map` wraps every pattern in
  `^...$`, yet `^STM32L5.*:TIM1.*$` still full-matches the lookup key
  `STM32L5xx:TIM16`; with first-match-wins, all 14 `:TIM16.*`/`:TIM17.*`
  DFSDM-break blocks plus `STM32L4(1|2|3|4|5|6).*:TIM15.*` (15 blocks in
  total) serve no `(chip, peripheral)` pair at all — those TIM15/16/17
  instead carry their family's TIM1 block (BREAK0+BREAK1). Where the dead
  block is RM-verified the output is wrong: RM0438 T210 routes
  break[0]->TIM1/TIM15, break[1]->TIM1 break2 + TIM16, break[2]->TIM8/TIM17,
  so L5:TIM16 should carry BREAK1 only and L5:TIM17 BREAK2 (both currently
  carry BREAK0+1, and TIM17 lacks BREAK2). Most families happen to order
  their `:TIM15.*` block before `:TIM1.*`; every TIM16/TIM17 block comes
  after it. L4(7-A), F412, F7(67) and F4(13|23) have no dedicated
  TIM15/16/17 DFSDM blocks and live on the same spill (their break tables
  are unverified — RM0394's DFSDM chapter is absent from `tmp/rm`, and
  L4(1-6)'s own dead TIM15 block says BREAK2 where RM0438 routes break[0]
  to TIM15). Fix = drop the trailing `.*` after `:TIM1` in the 12 TIM1
  blocks — deliberately deferred out of the per-die-trimming change because
  it *adds* tuples and deserves its own diff review. (`STM32F1.*:DAC.*`
  also never matches — harmless: 0002/0006/0007/0008/0009 cover every F1
  die with a DAC.)
- **Per-package cubedb gaps surfaced by trimming.** Some cubedb package
  XMLs omit peripherals that the family header defines for every package,
  so generation trims exactly those sources on exactly those variants:
  `STM32H7R3R8Vx.xml` lacks TIM3/TIM4/TIM16 (and LPTIM1) while
  `STM32H7R3L8Hx.xml` has them and `stm32h7r3xx.h` defines the `*_BASE`
  addresses (same pattern on H7S3R8/V8 and the H7R7/H7S7 A8/I8/Z8 variants
  for LPTIM1/TIM4); `STM32H7A3R(G-I)Tx.xml`, `STM32H7B0RBTx.xml` and
  `STM32H7B3RITx.xml` lack COMP2 although `stm32h7a3xx.h` defines
  `COMP2_BASE` (their sibling packages of the same die have it). The JSON
  stays self-consistent; fixing cubedb upstream restores the trigger rows
  with no rule change.
- **`0026` (H723-line DAC) is missing RM0468 ch. 31's trg4 (`tim5_trgo`)
  and trg7 (`tim8_trgo`) rows** — the fragment predates RM0468 landing in
  `tmp/rm/` (its old header claimed no RM existed on disk). A completeness
  gap, not a trimming issue: pending transcription.

## Refresh checklist when a missing RM arrives

1. Unpack the markdown under `tmp/rm/`, read `RM_PARSING_NOTES.md`.
2. Re-verify the affected fragments (table numbers are cited in the
   per-file headers), edit `data/triggers/raw/`.
3. `cargo run --release --bin trigger-rules-build`, then a full
   `cargo run --release --bin stm32-data-gen` (its validation catches
   duplicate/ill-formed entries).
4. Dump the `(chip, peripheral, signal, source)` tuples from
   `build/data/chips/*.json` before and after, and diff them: per-die
   trimming may only REMOVE tuples, and only where the source's
   peripheral is absent from that chip's JSON.
