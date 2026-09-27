# RM markdown parsing notes

## Conventions the extracted data must follow

See `data/triggers/CONVENTIONS.md` (in the repo root `data/triggers/`).
Key points: signal names (`ADC_EXT_TRG{n}`, `DAC_CHX_TRG{n}`, `DFSDM1_JTRG{n}`,
`TIMX_*_IN{n}`, `LPTIM_*`, `SPI_TRG{n}`, ...), source naming per family
(`TIM{n}_CC{m}` except F1/F4 which use `CH`), skip SWSTART/software/reserved,
split `EXTI line 11 / TIM8_TRGO` cells into two entries on the same index,
never emit sources for peripherals the chips don't have.

---

## Notes per RM

### RM0351-STM32L47-48-49-4A

- Full chapters on disk (ADC 18, DAC 19, DFSDM 24, TIM1/8 30). RM title
  says L47x-4Ax but several chapters describe the shared L4 IP.
- DAC: **Table 125 "DAC trigger selection"** (§19.x): v1 3-bit TSELx[2:0],
  7 external rows TIM6@0 TIM8@1 TIM7@2 TIM5@3 TIM2@4 TIM4@5 EXTI9@6 (+SWTRIG
  @7, skip). Applies to the whole non-plus L4 line (rule 0013/0014).
- ADC: **Table 108 regular / Table 109 injected** (EXTSEL[3:0]/JEXTSEL[3:0],
  each split into a `(continued)` part — concatenate; continuation caption
  bold `**Table 108. ... (continued)**`). Sources written `TIMx_CHy` → map to
  `TIMx_CCy`; `EXTI line 11/15` → EXTI11_TRG/EXTI15_TRG. Full 16+16, no
  gaps. Cross-checks verbatim against tmp/l4work/stm32l4xx_ll_adc.h.
- DFSDM: **Table 158 "DFSDM triggers connection"** (11 inputs jtrg[10:0]:
  TIM1_TRGO@0, TIM1_TRGO2@1, TIM8_TRGO@2, TIM8_TRGO2@3, TIM3_TRGO@4,
  TIM4_TRGO@5, TIM16_OC1@6, TIM6_TRGO@7, TIM7_TRGO@8, EXTI11@9, EXTI15@10;
  EXTI plain per DFSDM convention, TIM16_OC1 kept per DFSDM-rule precedent)
  and **Table 159 "DFSDM break connection"** (break0→TIM1 BRK, break1→TIM1
  BRK2, break2→TIM8 BRK, break3→TIM8 BRK2). Note: L47-4Ax TIM15/16/17 get
  NO DFSDM break (unlike L4+ and L5).
- L4P5 DAC trap: the stm32l4p5 SVD TSEL1 enum is a stale v1 3-bit copy
  (desc even references L45x/L46x); the real L4+ DAC is v2 4-bit — use the
  CMSIS device headers (stm32l4p5xx.h `DAC_CR_TSEL1_Msk` 0xF) to tell v1/v2
  apart per die.

### RM0394-STM32L41-42-43-44-45-46

- WARNING: extraction TRUNCATED — only chapters 0-2 on disk. The ADC
  chapter (external trigger tables) and DFSDM chapter are NOT available;
  L4(1-6) ADC/DFSDM rules keep hand/derived content with deferral notes
  (see tmp/proposals/l4_adc.yaml and raw rules 0069-0072).

### RM0432-STM32L4+

- WARNING: extraction TRUNCATED at chapter 5 (PWR) — the ADC (ch 21,
  Tables 131/132), DAC (ch 22, Table 147 "DAC trigger selection") and DFSDM
  chapters are NOT on disk. The 00-index.md IS complete and is valuable
  evidence: §10.3.2 lists the ADC1 source set (TIM1/TIM2/TIM3/TIM4/TIM6/
  TIM8/TIM15 + EXTI — identical to RM0351's), §10.3.5 the DFSDM1 source set
  (TIM1/TIM3/TIM4/TIM6/TIM7/TIM8/TIM16/LPTIM1/LPTIM2 + EXTI), §10.3.6 the
  DFSDM-to-timer destinations (TIM1/TIM8/TIM15/TIM16/TIM17 — L4+ TIM15/16/17
  DO get breaks, unlike L47-4A).
- CMSIS headers distinguish L4+ IP revisions where the SVDs are stale:
  DAC_CR_TSEL1[3:0] and DFSDM_FLTCR1_JEXTSEL[4:0] on L4P5/L4R5 vs [2:0]/
  [2:0] on L431/L476. The stm32l4p5/l4r5 SVDs themselves carry stale v1/3-bit
  field widths and incomplete ADC enums (TIM4/TIM8 rows missing — the same
  deficiency as the G4 SVD, where RM0440 Tables 166-169 prove those rows
  exist). RM wins; don't trust L4+ SVD enumerations.
- L4+ ADC has a single ADC1; HRTIM does not exist on L4 (no HRTIM sources).

### RM0091-STM32F0x1-0x2-0x8

- Chapters `13-...-adc.md` / `14-...-dac.md`. Trigger tables:
  ADC **Table 43 "External triggers"** (Name/Source/EXTSEL[2:0]);
  DAC **Table 52 "External triggers"** (Source/Type/TSEL[2:0]).
- Quirk: in the DAC trigger table the "Type" column is only filled in the
  first and last rows; timer rows have an empty third cell — shift columns
  left when parsing (source is col 0, index is col 1 for middle rows).
- DAC block-diagram figure caption (blockquote starting `> Figure 48.`)
  cross-checks the bonded timer trigger sources.

### RM0360-STM32F030x4-x6-x8-xC-070x6-xB

- F0x0 (F030/F070). ADC trigger table: **Table 35 "External triggers"**.
- Quirk: RM table is generic and lists `TIM2_TRGO` at TRG2, but F0x0 has no
  TIM2 — the SVD EXTSEL enums omit it. Die/SVD wins over the generic RM row.
- F0x0 has NO DAC (no DAC chapter; no DAC in SVD).

### RM0367-STM32L0x3

- L0x3 full chapters present. ADC **Table 60**, DAC **Table 70**
  "External triggers". DAC table has the same Type-column quirk as RM0091.
- TIM2/TIM21/TIM22 COMP input muxes are in the `TIMx option register
(TIMx_OR)` bit descriptions (TI4_RMP/TI1_RMP), no numbered tables.

### RM0376-STM32L0x2

- WARNING: chapter files stop at `20-general-purpose-timers-tim2-tim3.md`
  (truncated extraction) — no TIM21/22, LPTIM, basic-timer chapters on disk.
  DAC chapter present (15), ADC (14), COMP (16).
- ADC **Table 60**, DAC **Table 70** "External triggers".

### RM0377-STM32L0x1

- L0x1 (L031/L041). ADC **Table 58 "External triggers"**: TRG4 cell is
  combined `TIM22_TRGO, TIM21_TRGO (1)` with footnote "TIM21_TRGO is
  available only in category 1 devices" — split into two entries on TRG4.
- TIM21/22 COMP input muxes in `TIM21_OR`/`TIM22_OR` register bit
  descriptions. L0x1 has a COMP chapter but NO DAC chapter (no DAC).

### RM0440-STM32G4

- Files: `21-analog-to-digital-converters-adc.md`,
  `22-digital-to-analog-converter-dac.md`,
  `29-advanced-control-timers-tim1-tim8-tim20.md`.
- ADC: **Tables 166-169** (166 ADC1/2 regular, 167 ADC1/2 injected, 168
  ADC3/4/5 regular, 169 ADC3/4/5 injected; each split into a `(continued)`
  part, and continuation captions are NOT bold — `Table 167. ... (continued)`
  plain text vs `**Table 166. ...**` bold; grep for `Table` catches both).
  Columns: `Name | Source | Type | EXTSEL/JEXTSEL[4:0]` with binary index
  values (00000..11111). Row 0 of Table 166 has the name `adc_ext_trg`
  (no 0). TRG31 is `reserved` in all four tables; Table 167's TRG28 is also
  `reserved`. Sources are lowercase (`tim1_oc1`, `hrtim_adc_trg5`,
  `lptim_out`, `EXTI line 11`): map `timN_ocM` → `TIM{N}_CC{M}` (G4 uses CC
  not CH), `lptim_out` → `LPTIMOUT`, `EXTI line N` → `EXTI{N}_TRG`,
  `hrtim_adc_trgN` → `HRTIM_ADC_TRG{N}`. HRTIM_ADC_TRG5 legitimately appears
  at two indices (e.g. Table 166 EXT TRG23 and Table 167 JEXT TRG21) — not a
  typo. The old hand rules 0042/0043 matched the RM exactly; no data fixes.
- DAC: no classic "External triggers" table. **Tables 186-189 are per-DAC
  "interconnection" tables** (186 DAC1, 187 DAC2, 188 DAC3, 189 DAC4; each
  with `(continued)` parts) covering ALL THREE trigger muxes in one table:
  `dac_chx_trg1..8` = normal TSELx[3:0] trigger inputs (DAC_CHX_TRG{n}),
  `dac_chx_trg9..14` = sawtooth RESET trigger STRSTTRIGSELx (all HRTIM:
  `hrtim_dac_reset_trg1..6` → HRTIM_DAC_RESET_TRG{n}),
  `dac_inc_chx_trg1..8` = sawtooth INCREMENT trigger STINCTRIGSELx (timers +
  EXTI10), `dac_inc_chx_trg9..14` = HRTIM step (`hrtim_dac_step_trg1..6` →
  HRTIM_DAC_STEP_TRG{n}), and `dac_chx_trg15` = `hrtim_dac_trgN` →
  HRTIM_DAC_TRG{n} where N is 1/2/3/1 for DAC1/2/3/4. TSELx=0 is the
  software trigger (absent from the tables, omitted per conventions).
  EXTI appears bare (`EXTI9`/`EXTI10`) → map to EXTI9_TRG/EXTI10_TRG.
  DAC1 and DAC4 tables are IDENTICAL; a single rule 0044 covers both.
  Signal names use `chx` for DAC1/3/4 (`(x = 1, 2)` suffix) and `ch1` for
  DAC2 — the trigger signal names are the same either way.
- TIM1/8/20 (ch 29): **Tables 263-266** ti1..ti4 input muxes (TIM1 column is
  what a `:TIM1` rule needs; the table has one column per timer and the rows
  are identical across TIM1/8/20). TI1 in1..4 = comp1..4_out; in0 is the
  TIMx_CHy pin itself (skip). Table 268 `tim_etr` mux and Table 267
  `tim_itr` internal-trigger table are nearby if needed; Table 263 caption
  quirk: row `tim_ti1_in[15:5]` is a single combined Reserved row.
- Prose references to register fields are latex-escaped
  (`\\( EXTSEL\\[4:0\\] \\)`); table text itself is plain.
- Old hand rules 0042-0047 all matched the RM verbatim — the refresh only
  added RM-provenance headers and sorted the DAC entries numerically.

### RM0399-STM32H745-755-747-757

- WARNING: extraction TRUNCATED at chapter 30 (`30-operational-amplifiers-opamp.md`) — no DFSDM, TIM, HRTIM, LPTIM chapters on disk. DAC is chapter 27 and IS present.
- DAC trigger table: **Table 231 "DAC interconnection"** (section 27.4.2). Identical 13-entry table to RM0433 Table 227 (tim1..tim15, hrtim1_dactrg1/2, lptim1/2_out, exti9). RM writes sources in lowercase (`tim1_trgo`, `exti9`, `lptim1_out`, `hrtim1_dactrg1`) — normalize per CONVENTIONS.md (`TIM1_TRGO`, `EXTI9_TRG`, `LPTIM1_TRGO`, `HRTIM_DAC_TRG1`).
- Tables are plain `Table N.` captions (not always bold) inside section prose; raw-text grep works fine.

### RM0451-STM32L0x0

- L0x0 value line (L010/L011/L021). ADC **Table 54 "External triggers"**:
  TRG0 and TRG6 are `Reserved` — L0x0 has no TIM6/TIM3 (SVD wrongly lists
  TIM6_TRGO/TIM3_TRGO there).
- No COMP chapter: L0x0 has no comparators. TIM2_OR TI4_RMP values 01/10 are
  "Reserved" and TIM21/22_OR TI1_RMP only offer GPIO — COMP sources do not
  exist on this line.
- No DAC chapter (no DAC).

### RM0456-STM32U5

- DAC ch. 35 on disk: **Table 342 "DAC interconnection"** (§35.4.2) is the
  trigger table — 11 bonded rows: tim1/2/4/5/6/7/8/15_trgo @
  dac_chx_trg1..8 (9, 10, 14 unbonded), lptim1_ch1 @ 11, lptim3_ch1 @ 12,
  exti9 @ 13. LPTIM sources are the **CH1 signals**, not TRGO. Refreshed
  rule 0029 (old hand rule missed TIM5@4/TIM8@7 and said LPTIMx_TRGO).
- ADC ch. 33 (ADC1/2): **Table 307 regular (EXTSEL)** and **Table 308
  injected (JEXTSEL, with a `(continued)` part — concatenate)**. No HRTIM
  sources on U5. Gaps: adc_ext_trg17, adc_jext_trg16/17 have no source.
  Sources lowercase (`tim1_oc1`, `lptim1_ch1`, `exti11`) → normalize
  `timN_ocM` → `TIMN_CCM`, `lptimN_chM` → `LPTIMN_CHM`, `extiN` →
  `EXTIN_TRG`. ANOMALY: Table 307 writes the LPTIM4 source as `lptim4_out`
  and Table 308 as `lptim4_out1` — same signal, mapped to LPTIM4_TRGO.
- Sub-family split (Table 301 "ADC features"): U535/545/575/585 = single
  ADC1 WITH injected; U59x/5Ax/5Fx/5Gx = ADC1/ADC2 dual WITHOUT injected
  (regular triggers only). Proposed regexes `^STM32U5[3458].*:ADC(1|2)` and
  `^STM32U5[79FG].*:ADC(1|2)` (tmp/proposals/u5_adc.yaml); `:ADC(1|2)` so
  ADC4 (ch. 34, separate IP/table) is not swallowed.
- All of TIM1/8, TIM2/3/4/5, TIM15/16/17, TIM6/7 exist on U535/545 too
  (chapters 54-57 carry U535/545 in their interconnect tables), so no
  per-subfamily trimming of timer sources is needed for the ADC/DAC rules.

### RM0487-STM32U3

- WARNING: extraction TRUNCATED at ch. 15 (GPDMA) — **no ADC (ch. 23) or
  DAC (ch. 24) chapters on disk**; the per-index EXTSEL/JEXTSEL/TSEL tables
  (Tables 206+, 226) are NOT available.
- Surviving evidence: ch. 14 peripheral interconnect matrix. §14.3.2
  "Triggers to ADCs": sources TIM1/2/3/4/6/8/15 + LPTIM1/2/3/4 + EXTI11/15,
  regular AND injected on ADC1/2. §14.3.4 "Triggers to DAC": sources
  TIM1/2/4/6/7/8/**12**/15 + LPTIM1_CH1/LPTIM3_CH1 + EXTI — U3 has TIM12
  (U5 does not) and has no TIM5.
- Consequences: U3 ADC rule is DEFERRED (proposal file tmp/proposals/
  u3_adc.yaml carries notes only, no rules); U3 rows of shared DAC rule
  0029 keep U5 indices with a deferral note in the fragment header.

### RM0313-STM32F37

- DAC trigger tables: `14-digital-to-analog-converter-dac1-and-dac2.md`,
  section 14.5.4, **Table 40. External triggers (DAC1)** and
  **Table 41. External triggers (DAC2)**. Caption pattern
  `**Table NN. External triggers (DACn)**`, columns `Source | Type | TSEL[2:0]`
  (3-bit TSEL, values as 000..111 binary).
- Prose writes timers as "Timer 6 TRGO event" (spelled out) — normalize to
  `TIM6_TRGO`.
- DAC1 vs DAC2 differ only at TSEL 011 (TIM5 vs TIM18). EXTI9 is 110 on both;
  the old hand rules had EXTI9 at 5 (wrong), and DAC1's TIM4@101 was missing.
- No F373-vs-F378 differences documented in these tables.

### RM0316-STM32F303xB-C-D-E-303x6-8-328x8-358xC-398xE

- DAC trigger tables: `16-digital-to-analog-converter-dac1-and-dac2.md`,
  section 16.5.4, **Table 105. External triggers (DAC1)** and
  **Table 106. External triggers (DAC2)**. Table 106 is split in two with a
  `(continued)` caption repeat — concatenate rows.
- Remap rows read e.g. `TIM3_TRGO event (1) or Timer 8 TRGO event (2)` with
  footnotes explaining the SYSCFG_CFGR1 DAC_TRIG_RMP bit — becomes TWO entries
  on the same TSEL index (default first, remap second).
- SVD TSEL2 enum for stm32f303 lists TIM5_TRGO@TSEL=011 — RM shows TIM15_TRGO;
  TIM5 does not exist on F303. Classic SVD copy-paste error, RM wins.
- DAC2 has NO TIM4 at TSEL=101 (unbonded); old hand rule had it.

### RM0364-STM32F334

- DAC trigger tables: `14-digital-to-analog-converter-dac1-and-dac2.md`,
  section 14.5.4, **Table 53 (DAC1)** / **Table 54 (DAC2)**.
- HRTIM sources appear as `HRTIM1_DACTRGn` — map to `HRTIM_DAC_TRGn`
  (CONVENTIONS.md naming; n matches DACTRGn numbering: TRG1@TSEL011 DAC1,
  TRG2@TSEL101 DAC1, TRG3@TSEL101 DAC2).
- Remap bits live in SYSCFG_CFGR3 (DAC1_TRIG3_RMP / DAC1_TRIG5_RMP), unlike
  other F3 where DAC_TRIG_RMP is in SYSCFG_CFGR1.
- DAC2's TSEL=101 is HRTIM1_DACTRG3 with NO alternate/default source.
- No separate DAC trigger list in the HRTIM chapter (ch 21) — DAC is a
  destination there only via ADC trigger figures; the DAC chapter tables are
  the source of truth.

### RM0365-STM32F302xB-C-D-E-302x6-8

- DAC trigger table: `16-digital-to-analog-converter-dac1.md`, section 16.5.4,
  **Table 100. External triggers (DAC1)**.
- F302 DAC is richer than F301: TIM3@001 (remap, SYSCFG_CFGR1 DAC_TRIG_RMP)
  and TIM4@101; TSEL=010 Reserved (F302 has no TIM7 — RM ch 23 covers only
  TIM6). No TIM8/TIM5 anywhere.

### RM0366-STM32F301x6-8-318x8

- DAC trigger table: `13-digital-to-analog-converter-dac1.md`, section 13.5.4,
  **Table 46. External triggers (DAC1)**.
- Only 4 external sources bonded: TIM6@000, TIM15@011, TIM2@100, EXTI9@110;
  TSEL 001/010/101 are Reserved. F301/F318 have NO TIM3/TIM4/TIM7 — the
  stm32f301.svd TSEL1 enum (TIM3@1, TIM7@2) is a copy-paste error; RM wins.
- Because one raw rule matches F301+F302+F318 but the RMs disagree, keep the
  rule at the F301/F318 intersection and put F302's extras in a separate
  earlier-ordered fragment (see tmp/proposals/f302_dac.yaml).

### RM0008-STM32F101-102-103-105-107

- DAC trigger table: `12-digital-to-analog-converter-dac.md`, **Table 74
  "External triggers"** (caption NOT bold — plain text `Table 74. External
triggers`, unlike the bold `**Table N.**` style in newer RMs). The table is
  shared across ALL sub-families (low/med/high/XL density + connectivity
  line); a single row covers variants: index 001 is "Timer 3 TRGO in
  connectivity line devices or Timer 8 TRGO in high-density and XL-density
  devices". There are no separate per-subfamily tables — pick the row variant
  matching the chips your rule matches (e.g. F105/F107 → TIM3 only).
- ANOMALY: the shared table lists TIM8_TRGO (index 001) and TIM5_TRGO
  (index 011) with no sub-family caveat beyond the TIM8 footnote, yet F101
  (all densities) and low-/medium-density F103 have no TIM5/TIM8 — the RM
  documents mux inputs whose sources don't exist on those dies. Rules
  0003/0004 split on flash-density code to handle this; the DAC rules
  0006/0007 still carry the non-existent sources from the SVD/hand rules.
- ADC trigger tables: `11-analog-to-digital-converter-adc.md`, tables 67
  (ADC1/2 regular), 68 (ADC1/2 injected), 69 (ADC3 regular), 70 (ADC3
  injected). Same multi-subfamily pattern: index 110 of tables 67/68 is a
  combined cell `EXTI line 11 / TIM8_TRGO` (footnote: TIM8 exists only in
  high-/XL-density) → split into two entries on the same index. Index 111 is
  SWSTART/JSWSTART (skip).
- Tables parse fine with markdown-it-py commonmark; cells with merged/empty
  Type column (rowspan source) come through with shifted-but-recoverable
  cells (TSEL value lands in the middle cell) — the `|` raw-text grep is
  often quicker than token-walking for these small tables.

### RM0038-STM32L100-151-152-162

- DAC trigger table: `13-digital-to-analog-converter-dac.md`, **Table 68
  "External triggers"**. L1 DAC is 1-channel 10-bit; TSELx is 3 bits
  (000..111). Rows: TIM6@000, Reserved@001, TIM7@010, TIM9@011, TIM2@100,
  TIM4@101, EXTI line9@110, SWTRIG@111 (skip). Same Type-column quirk as
  RM0091 (middle rows shift left: source col 0, TSEL col 1).
- TIM9 exists on L1 (ch. 18 "General-purpose timers TIM9 to TIM11") — keep
  TIM9_TRGO@011; the old hand rule already matched the RM exactly.

### RM0433-STM32H742-743-753-750

- DAC trigger table: **Table 227 "DAC interconnection"** (section 26.4.2). 13 entries: TIM1_TRGO@1, TIM2@2, TIM4@3, TIM5@4, TIM6@5, TIM7@6, TIM8@7, TIM15@8, HRTIM_DAC_TRG1@9, HRTIM_DAC_TRG2@10, LPTIM1_TRGO@11, LPTIM2_TRGO@12, EXTI9_TRG@13. Identical to RM0399 Table 231. The old hand rules (0028) already matched exactly.
- DFSDM chapter is 30. **Table 250 "DFSDM triggers connection"** (split with a `(continued)` caption — concatenate): jtrg0 TIM1_TRGO, 1 TIM1_TRGO2, 2 TIM8_TRGO, 3 TIM8_TRGO2, 4 TIM3_TRGO, 5 TIM4_TRGO, 6 TIM16_OC1, 7 TIM6_TRGO, 8 TIM7_TRGO, 9 HRTIM1_ADCTRG1, 10 HRTIM1_ADCTRG3, [23:11] Reserved, 24 EXTI11, 25 EXTI15, 26 LPTIMER1, 27 LPTIMER2, 28 LPTIMER3, [31:29] Reserved. Normalize `LPTIMER{n}` → `LPTIM{n}_TRGO`; EXTI stays plain `EXTI{n}` (DFSDM convention); HRTIM DFSDM sources are `HRTIM1_ADCTRG{n}`, NOT `HRTIM_ADC_TRG{n}` (DAC is the HRTIM_DAC_TRG user). Old hand rules 0082/0098 matched exactly.
- **Table 251 "DFSDM break connection"**: break0 → TIM1 BRK / TIM15 BRK; break1 → TIM1 BRK2 / TIM16 BRK; break2 → TIM8 BRK / TIM17 BRK; break3 → TIM8 BRK2. Old hand rules 0083-0087/0099-0103 matched exactly.
- This RM does NOT document H723/733/725/735/730, H7A3/B3/B0, or H745/757 DFSDM; rules for those lines kept hand content (H745/757 DFSDM assumed identical IP to H743; RM0399 is truncated so it can't corroborate).

### RM0434-STM32WB55-35

- ADC ch. 19 on disk: **Table 92 "ADC1 - External triggers for regular
  channels"** (EXTSEL[3:0]) and **Table 93 "... injected channels"**
  (JEXTSEL[3:0]). "-" rows (EXT4/5/13/14, JEXT4/5/14/15) are unbonded.
  EXTI written "EXTI Line 11/15" → EXTI11_TRG/EXTI15_TRG; JEXT3 written
  `TIM2_CH1` → TIM2_CC1.
- The WB55/35 tables are IDENTICAL to RM0471 (WB50/30) Tables 67/68 — one
  `^STM32WB.*:ADC.*` rule covers both (proposal in tmp/proposals/wb_adc.yaml).
- No DAC on WB (no DAC chapter); no COMP-in-TIM mux tables needed here
  (TIM1/2 chapters exist but the WL rules 0048/0049 use RM0453).

### RM0453-STM32WL5x

- Covers WL5x and WLE5 (folder name says WL5x only; the RM explicitly covers
  WLE5). Chapters: ADC 18, DAC 19, TIM1 25, TIM2 26, LPTIM 28.
- DAC **Table 114 "DAC interconnection"** (split into a `(continued)` part —
  concatenate): trg1 tim1_trgo, trg2 tim2_trgo, trg11 lptim1_out,
  trg12 lptim2_out, trg13 lptim3_out, trg14 exti9. LPTIM sources named
  `lptim{n}_out` in the RM but reported as `LPTIM{n}_TRGO` per the L4/H7
  DAC precedent (CONVENTIONS.md). Old hand rule 0068 matched exactly.
- TIM1 comparator mux: not a numbered table — TIM1_OR1 bit 4 TI1_RMP
  (0: I/O, 1: COMP1_OUT) in ch. 25. TIM2: TIM2_OR1 bits 3:2 TI4_RMP
  (00 GPIO, 01 COMP1_OUT, 10 COMP2_OUT, 11 COMP1_OR_COMP2_OUT) in ch. 26.
  Old hand rules 0048/0049 matched exactly.
- ADC **Table 102 "External triggers"** (EXTSEL[2:0], regular only — WL ADC
  has no injected channels): TRG0 TIM1_TRGO2, TRG1 TIM1_CC4, TRG2 TIM2_TRGO,
  TRG3 TIM2_CH4→TIM2_CC4, TRG5 TIM2_CH3→TIM2_CC3, TRG7 EXTI11→EXTI11_TRG;
  TRG4/TRG6 Reserved. ANOMALY: both wl5x_cm4 and wle5 SVD EXTSEL enums say
  TIM1_TRGO at index 0 — RM wins (TIM1_TRGO2). Proposal in
  tmp/proposals/wl_adc.yaml.

### RM0471-STM32WB50CG-30CE

- WB50/30 value line. ADC ch. 15: **Table 67 "ADC1 - External triggers for
  regular channels"** and **Table 68 "... injected channels"** — byte-for-
  byte identical to RM0434 Tables 92/93, so WB ADC rules don't need a
  WB50/30 split.
- No DAC/COMP/TSC/LCD chapters (WB50/30 lack those peripherals).

### RM0438-STM32L5

- DFSDM chapter (26) has both interconnect tables in one place:
  **Table 209 "DFSDM triggers connection"** (injected triggers, 32 inputs):
  TIM1_TRGO@0, TIM1_TRGO2@1, TIM8_TRGO@2, TIM8_TRGO2@3, jtrg[23:9]
  Reserved, EXTI11@24, EXTI15@25, "LPTIMER1"@26 (map to LPTIM1_TRGO),
  jtrg[31:27] Reserved.
- **Table 210 "DFSDM break connection"** (4 watchdog-break outputs):
  break[0] -> TIM1/TIM15 BRK; [1] -> TIM1 BRK2 / TIM16 BRK; [2] ->
  TIM8/TIM17 BRK; [3] -> TIM8 BRK2. Corroborated by the TIMx_BDTR
  BKDF1BKxE / BK2DF1BKxE bit descriptions in ch. 33 (TIM1/8) and ch. 35
  (TIM15/16/17). Old hand rules 0122-0127 matched both tables exactly.
- No L5 SVD ADC EXTSEL/JEXTSEL fragments exist in `tmp/svd_fragments/`
  (only stm32l4p5 among L4/L5) — an L5 ADC rule needs an SVD extraction
  first if it is to be cross-checked.

### RM0444-STM32G0x1

- WARNING: extraction TRUNCATED — chapter files stop at
  `13-extended-interrupt-and-event-controller-exti.md`. Chapters 14 (CRC),
  15 (ADC) and 16 (DAC) are missing, so **Table 73 (ADC "External
  triggers") and Table 85 (DAC "DAC interconnection") are not on disk**.
  The full table list IS available in `00-index.md` (Tables 71-90 are the
  ADC/DAC ones). Only the interconnect matrix (ch. 9, §§9.3.2/9.3.4)
  survives as evidence: it lists the exact source sets
  (TIM1/2/3/4/6/15+EXTI for ADC; TIM1/2/3/4/6/7/15/LPTIM1/LPTIM2+EXTI
  for DAC) and points at §15.4 / Table 73 and §16.4.7 for the indices.
- No-download policy applied: the per-index TSEL/EXTSEL tables for G0x1
  could not be re-verified; rules 0034/0035 keep the old hand-rule
  indices, cross-checked against the interconnect-matrix source lists
  and against RM0454's on-disk ADC table.
- G0B1/G0C1-specific DAC sources (if any) are not determinable from the
  on-disk material.

### RM0454-STM32G0x0

- ADC ch. 14 on disk: **Table 57 "External triggers"** (EXTSEL[2:0],
  Name/Source/binary-index columns, parseable with markdown-it-py).
  TRG2 is `Reserved` (G0x0 has no TIM2) — the key difference vs G0x1.
  TRG7 source is written `EXTI11` (normalize to `EXTI11_TRG`).
- ANOMALY: no DAC chapter on disk, yet the G070 SVD has a DAC
  peripheral (G030/G050 have none). G0x0 also has no LPTIM (no LPTIM
  chapter), so whether G070's DAC bonds the LPTIM TSEL rows is
  unverifiable on disk.
- Because rule 0035 (`^STM32G0.*:ADC.*`) also matches G0x0 chips,
  TIM2_TRGO@TRG2 is wrong for them — the rule should be split by
  sub-family (reported, not actioned).

### RM0490-STM32C0

- ADC ch. 16 on disk: **Table 68 "External triggers"** (EXTSEL[2:0]) is
  a SINGLE table for the whole C0 line with footnoted availability:
  TIM2_TRGO "(1) Available only on STM32C051/71/91/92xx"; TIM15_TRGO
  "(2) Available only on STM32C091/92xx". TRG5/TRG6 are Reserved
  everywhere. This one table authoritatively validates the three old
  hand rules 0036/0037/0038 (6/5/4 entries) — contents unchanged, only
  provenance headers updated.
- TRG7 source written `EXTI11` → `EXTI11_TRG`.
- No DAC chapter: C0 has no DAC. EXTI11 is ADC source only.

### RM0041-STM32F100

- DAC trigger table: `11-digital-to-analog-converter-dac.md`, **Table 64
  "External triggers"** (bold caption style). Index 011 is a combined cell
  "Timer 5 or Timer 15 TRGO event" → two entries (TIM5_TRGO, TIM15_TRGO);
  on high-density value line TIM15 needs AFIO_MAPR2 MISC_REMAP, else TIM5.
- ANOMALY: the stm32f100.svd DAC TSEL enum says TIM8_TRGO@1 / TIM5_TRGO@3 —
  F100 has no TIM8 (SVD copy-paste from F103); RM0041 table 64 confirms
  TIM3_TRGO@1. F100 DOES have TIM5 (RM0041 ch. 13 "TIM2 to TIM5"), so
  TIM5_TRGO@3 is kept alongside TIM15_TRGO@3.
- The figure captions are extremely verbose auto-generated alt-text
  (blockquotes after `**Figure N.**` lines) — grep for `**Table` to find
  tables quickly.

### RM0401-STM32F410

- DAC trigger table: `12-digital-to-analog-converter-dac.md`, **Table 52
  "External triggers"** (bold caption). Only TWO bonded rows: TIM5 TRGO
  (TSEL=011) and EXTI line9 (TSEL=110), plus SWTRIG (skip). Unlike the full
  7-entry F4 DAC table, F410 bonds only a subset — do not "complete" it from
  the stm32f410.svd enum, which lists all 7.

### RM0481-STM32H523-33-562-63-573

- WARNING: extraction TRUNCATED — chapter files stop at
  `15-peripherals-interconnect-matrix.md`. The DAC chapter (ch. 28, with
  **Table 281 "DAC interconnection"**) and the ADC chapter (ch. 26, with
  **Table 257 "ADC interconnection"**, covering both adc_ext_trg/EXTSEL and
  adc_jext_trg/JEXTSEL) are NOT on disk. No-download policy applied: rules
  0031/0032/0033 (H5 DAC) and 0041 (H5 ADC) keep their hand-rule content with
  "DEFERRED — no RM available on disk" headers.
- Partial corroboration from ch. 15 (peripherals interconnect matrix, on disk):
  §15.3.2 says the ADC1/2 source set is TIM2/3/4 (oc+trgo), TIM6, TIM1/8
  (oc+trgo), TIM15, LPTIM1/2 (channels), EXTI11/EXTI15 — consistent with the
  old 0041 entries (incl. LPTIM1_CH1/LPTIM2_CH1 at EXTSEL 18/19). §15.3.4 says
  the DAC source set is TIM1/2/4/5/6/7/8/15, LPTIM1/2 (ch1), EXTI9 —
  consistent with 0032's 13 entries; 0031 (H503, 9 entries, TIM3@3) and 0033
  (7 entries) are die-pruned subsets. Indices are NOT recoverable from ch. 15.
- Ch. 15 prose uses lowercase signal names (`tim1_trgo`, `lptim1_ch1`,
  `exti9`) and refers to tables in the missing chapters by number
  (`/rm0481-.../28-digital-to-analog-converter-dac/#table-281`) — dead links
  on disk. Table 133 (interconnect matrix summary) has no per-index detail.
- ADC3 (H56x/57x only) is documented only through the same ADC1/2 table;
  ADC3-specific rows unverifiable.
- Encoding quirk: the ch. 15 file is not clean UTF-8 for the Windows default
  cp1252 reader (byte 0x9d at ~offset 5382) — open with
  `encoding='utf-8', errors='replace'`.

### RM0493-STM32WBA5

- Full chapters on disk incl. TIM1 (29), LPTIM (32), ADC4 (21), I2C (38),
  USART (39), SPI (41), interconnect matrix (16). Rules 0050, 0058-0062,
  0065-0067 verified verbatim.
- TIM1 interconnect: per-input tables — **Table 255** (tim_ti1), **Table 259**
  (tim_itr, +continued), **Table 260** (tim_etr), **Tables 261/262** (tim_brk /
  tim_brk2), **Table 263** (tim_sys_brk), **Table 264** (tim_ocref_clr).
  Sys-brk4 is "HSE32 lock Security System" — the old hand rule 0050 wrote
  `HSE2_HSECSS` (no "HSE2" exists in this RM) → corrected to `HSE32_HSECSS`.
  COMP1/COMP2 rows footnoted "only available on STM32WBA54xx/55xx"; WBA52 has
  no COMP (rule regex excludes it — fine).
- LPTIM: **Tables 311-315** (ext trig / in1 / in2 / ic1 / ic2), one table for
  LPTIM1 and LPTIM2 side by side; LPTIM1 IC2_MUX1 = LSI, LPTIM2 = HSI16/256;
  LPTIM1 EXT_TRG5 Reserved, LPTIM2 = gpdma_ch4_tc.
- ADC4: **Table 150 "ADC interconnection"** — also covers internal channels
  (V SENSE etc., skip); triggers adc_trg0..7, trg3/4/6 Reserved → 5 entries.
- SPI: **Table 399 (SPI1)** 12 rows; **Table 400 (SPI3)** — SPI3 differs:
  exti8@trg5, trg7/trg9 Reserved (no LPTIM2_CH1/COMP2_OUT). I2C: **Table 357
  (I2C1)** 12 rows; **Table 358 (I2C3)** — exti8@trg5, trg7 Reserved.
  USART: **Table 377 (USART1/2)** 12 rows, trg12..15 unconnected "-".
- Cubedb peripherals grep works for availability checks (WBA52 has no COMP).

### RM0503-STM32U0

- Full chapters on disk incl. ADC (14), DAC (15), TIM1 (23). DAC trigger
  table: **Table 80 "DAC interconnection"** (§15.4.2, split into a
  `(continued)` part — concatenate; the continuation caption is plain text,
  not bold). Columns `Signal name | Source | Source type`; rows:
  dac_ch1_trg1 tim1_trgo, trg2 tim2_trgo, trg3 tim3_trgo, trg5 tim6_trgo,
  trg6 tim7_trgo, trg8 tim15_trgo, trg11 lptim1_out, trg12 lptim2_out,
  trg14 exti9 (plus dac_hold_ck LSI clock row — not a trigger, skip).
  Normalize `lptim{n}_out` → `LPTIM{n}_TRGO`, `exti9` → `EXTI9_TRG`. 8
  trigger rows; old hand rule 0040 matched exactly.
- TIM1 comparator inputs: NOT a numbered table — §23.4.29 "TIM1 timer input
  selection register (TIM1_TISEL)" bit descriptions: TI1SEL[3:0] 0001 = COMP1
  output (tim_ti1_in1), TI2SEL[3:0] 0001 = COMP2 output (tim_ti2_in1);
  TI3SEL/TI4SEL 0000 = TIM1_CH3/CH4, "Others: Reserved" (no internal sources).
  Old hand rule 0039 matched exactly.
- TIM1 ETR also has COMP sources via ETRSEL[3:0] in TIM1_AF1 (Figure 143:
  COMP1/COMP2 outputs, ADC1 AWD1-3) — captured by TIMX_ETR_IN rules, not the
  TI1/TI2 rule.

### RM0515-STM32WBA6

- WARNING: extraction TRUNCATED at ch.36 (WWDG) — **no I2C (39), USART (40),
  LPUART (41) or SPI (42) chapters on disk**; the ch.16 interconnect matrix
  references Tables 371/372 (I2C), 391 (USART), 403 (LPUART), 413/414 (SPI)
  at dead links. Rules 0055-0057 and 0063-0064 (WBA6 SPI/USART/I2C) keep
  their hand content with DEFERRED headers, cross-checked against the
  byte-identical RM0493 Tables 357/358/377/399/400. TIM1 (30), LPTIM (33),
  ADC4 (21) and the interconnect matrix (16) ARE on disk.
- TIM1 interconnect tables numbered +14 vs RM0493: **Table 269** (tim_ti1),
  **Table 273** (tim_itr, +continued), **Table 274** (tim_etr), **275/276**
  (brk/brk2), **277** (sys_brk), **278** (ocref_clr).
- ANOMALY: per-table footnotes differ! COMP2 rows are footnoted "Only
  available on STM32WBA62/63/65xx" while tim_itr3 = tim4_trgo is footnoted
  "Only available on STM32WBA62/64/65xx". Cross-checked with the
  stm32wba6x SVDs: 62/63/65 have COMP2, WBA64 does not (cubedb XML for
  WBA62/65 omits COMP2 — cubedb gap; RM+SVD agree). TIM4 exists on 62/64/65,
  NOT on 63 (cubedb peripheral lists).
- Consequence for rules 0051/0052: 0051 (WBA6[235]) correctly carries COMP2
  rows but its set mixes TIM4-having (622, 65x) and TIM4-less (623) dies, so
  ITR_IN3 stays only in 0052; first-match-wins makes 0052 effective only for
  STM32WBA624/644 (NONEMPTY — WBA64 exists in cubedb and has TIM4/16/17).
  ITR_IN7/8 (tim16_oc1/17) were ADDED to 0051 — RM Table 273 lists them
  unconditionally.
- LPTIM: **Tables 325-329**, side-by-side LPTIM1/LPTIM2 columns; identical
  structure to RM0493 Tables 311-315. Rules 0053/0054 verified verbatim.
- ADC4: **Table 154** byte-identical to RM0493 Table 150. Rule 0067 verified.
- WBA6 chips in cubedb: WBA622/623/624/625(/64x/65x part numbers WBA62CGU,
  WBA63CGU, WBA64CGU, WBA65CGU). WBA64 has I2C1/2/3/4, SPI1/3 (no SPI2),
  USART1/2/3 — rule 0055's `SPI[12]` simply never matches SPI2 there.
