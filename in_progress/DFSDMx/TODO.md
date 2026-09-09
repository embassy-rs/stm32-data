# DFSDM data-pipeline TODO (stm32-data)

Result of the full audit against all 15 local TRMs, metapac, cubedb and the
generated chip JSONs. Companion file on the HAL side:
`embassy-stm32/src/dfsdm/TODO-v2.md`.

---

## VERIFIED CORRECT — no action (reference)

- Block mappings currently emitted are correct per the TRM implementation
  tables: F412/F413-DFSDM1 `DFSDM_4CH_2FLT_TRG3` (rm0402/rm0430), F413-DFSDM2
  `DFSDM_8CH_4FLT_TRG3` (rm0430), F76x `DFSDM_8CH_4FLT_TRG5` (rm0410),
  H7(42..57) `DFSDM_8CH_4FLT_TRG5_ADC` (rm0433/rm0399), H7(23..35)
  `DFSDM_8CH_4FLT_DLY_TRG5_ADC` (rm0468), H7A/B-DFSDM1
  `DFSDM_8CH_8FLT_DLY_TRG5_ADC` + DFSDM2 `DFSDM_2CH_1FLT_TRG5` (rm0455),
  L496/L4A6 `DFSDM_8CH_4FLT_TRG3_ADC` (rm0351 footnote: ADC only L49x/L4Ax),
  L4P5/L4Q5 `DFSDM_4CH_2FLT_DLY_TRG5_ADC` (rm0432 Table 184), L4Rx/L4Sx
  `DFSDM_8CH_4FLT_DLY_TRG5_ADC` (rm0432 Table 183), MP15x
  `DFSDM_8CH_6FLT_DLY_TRG5_ADC_HWID` (rm0436/rm0441/rm0442).
- trigger.rs DFSDM1/DFSDM2 JTRG + break sections verified **exact** against
  the TRM trigger/break tables for: L4(789A), L4(1-6), F412, F7, L4(PQRS)
  (incl. "LTIMER1"→LPTIM1 correction), H7(42/43/53/50), H7(A|B)3
  (DFSDM1+DFSDM2), H7(23..35) — TIM23/TRGO + TIM24/TRGO at jtrg11/12 is
  TRM-correct (Table 287: jtrg9/10 reserved) and TIM23/TIM24 exist on H723,
  L5 (4 TRGO + EXTI11/15 + LPTIM1 only ✓), MP1.
- RCC DFSDM bits verified against the TRM RCC chapters for every series:
  F4 (incl. F413 DFSDM2EN on APB2ENR bit 25, rm0430-confirmed), F7
  (APB2ENR/APB2LPENR/APB2RSTR DFSDM1* + DCKCFGR1 DFSDM1SEL/ADFSDM1SEL), L4
  (APB2 + CCIPR bit 31; yaml naming DFSDMEN/DFSDMRST/DFSDMSEL vs TRM
  DFSDM1EN/DFSDM1RST/DFSDM1SEL is cosmetic — same bit positions), L4+
  (CCIPR2), L5, H7, H7AB (DFSDM1 + DFSDM2 on APB4 + DFSDM2SEL — complete),
  H723+ (DFSDM1EN/LPEN/RST bit 30 + DFSDM1SEL — complete), MP1
  (DFSDMEN/DFSDMLPEN/DFSDMRST; ADFSDM* missing, see SD7).
- TIM break enable fields (`BKDF1BKE` AF1, `BK2DF1BK1E` AF2) integrated for
  every DFSDM timer version: `data/registers/timer_v1.yaml` (AF1_1CH_CMP
  fieldset, bit 8; AF1_ADV extends it for TIM1/TIM8; TIM15/16/17 use
  AF1_1CH_CMP directly) + `timer_v3.yaml`. timer_v2/timer_l0 lack the
  fields — correct, no DFSDM chip uses those versions.

---

## TODO

- [ ] **SD1 — header.rs: DFSDM1 `_NS` alias (unlocks L552/L562).**
  Root cause: L5 is TrustZone-attributed (RM0438 `DFSDM1SEC`); its Cube
  headers define only `DFSDM1_BASE_NS` (zero plain `DFSDM1_BASE`), so
  `resolve_peri_addr` → None and the generator silently drops the DFSDM1
  peripheral — the L552/L562 chip JSONs contain no DFSDM at all.
  Fix: add to `ALT_PERI_DEFINES` in `stm32-data-gen/src/header.rs`
  (get_peri_addr, ~line 195; precedented by OCTOSPI/USB/FMC `_NS` aliases):
  `("DFSDM1", &["DFSDM1_BASE", "DFSDM1_BASE_NS"])`. generator.rs untouched.

- [ ] **SD2 — perimap.rs: F7 regex excludes F777/778/779.**
  `r"STM32F7[6].*:dfsdm1_F7_v1_0.*"` — char class `[6]` never matches F77x.
  STM32F777NI.json has DFSDM1 (cubedb) but no registers block. 11+ chip
  groups affected. Fix: `STM32F7[67].*` (covers F765/767/768/769 and
  F777/778/779 — F768/778's distinguishing digit is the 4th). Config name is
  shared (`dfsdm1_F7_v1_0_Cube`) across all F7xx — F77x get the same block,
  no new block needed.

- [ ] **SD3 — perimap.rs: L4 regexes dead/wrong (L451/452/462 + L471/475/476/
  485/486 missing).**
  - `r"STM32L4[9]2.*"` targets "L492" — no such chip. Dead.
  - `r"STM32L4[10].*"` / `r"STM32L4[11].*"` char classes only match L41x/L40x
    prefixes — no DFSDM chips there. Dead.
  - Ground truth (cubedb): L451/452/462 use config
    `dfsdm1_v1_0_4ch_L4x1_Cube`; L471/475/476/485/486 use plain
    `dfsdm1_v1_0_Cube`; L496/L4A6 use `dfsdm1_v1_0_L49_Cube` (already
    mapped ✓ via `L4[9A]`).
  - Correct targets per TRM: rm0394 (L41x-46x) = 4ch/2flt/**no ADC**/11 trg →
    **`DFSDM_4CH_2FLT_TRG3`** (the current intended `TRG3_ADC` is wrong —
    rm0351's footnote puts ADC input only on L49x/L4Ax). rm0351 (L47x/48x) =
    8ch/4flt/no ADC → **`DFSDM_8CH_4FLT_TRG3`**.
  - Fix (patterns are anchored `^$` by util::new_regex_map, so config-distinct
    patterns are safe): **replace the three dead L4 patterns above** with
    `(r"STM32L4(5|6)(1|2).*:dfsdm1_v1_0_4ch_L4x1.*", ("dfsdm","v1","DFSDM_4CH_2FLT_TRG3"))`
    `(r"STM32L4(7|8).*:dfsdm1_v1_0_Cube.*", ("dfsdm","v1","DFSDM_8CH_4FLT_TRG3"))`

- [ ] **SD4 — trigger.rs: H7B0 uncovered.** All H7A/B sections use
  `r"^STM32H7(A|B)3"` — H7B0 (5 chips; has DFSDM1+DFSDM2+LPTIM1/2/3) matches
  neither. Widen to `r"^STM32H7(A|B)"` (perimap already covers H7B0 via
  `H7[AB]`).

- [ ] **SD5 — trigger.rs: F413 JTRG names corrupted — RESOLVED, fix values.**
  RM0430 Rev 9 Table 89 prints trailing digits (TIM1_TRGO2/TIM3_TRGO3/
  TIM8_TRGO4/TIM6_TRGO1/...). Evidence they are orphaned footnote markers,
  not signal names: digits are inline full-size glyphs (page 392 rendered at
  170/400 dpi); no footnote text exists in the PDF (text layer + page 393);
  RM0430 TIMs have MMS[2:0] only — zero MMS2 in the whole RM; TRGO1/2/3/4
  tokens occur ONLY inside Table 89 (7×TRGO2, 2×TRGO3, 2×TRGO4, 1×TRGO1 =
  exactly the table's rows). Correct mapping (strip digits):
  - DFSDM1: jtrg0 TIM1_TRGO, jtrg1 TIM3_TRGO, jtrg2 TIM8_TRGO, jtrg3
    TIM10_OC1, jtrg4 N/A, jtrg5 TIM4_TRGO, jtrg6 N/A, jtrg7 TIM6_TRGO,
    jtrg8 N/A, jtrg9 EXTI11, jtrg10 EXTI15.
  - DFSDM2: jtrg0 TIM1_TRGO, jtrg1 TIM3_TRGO, jtrg2 TIM8_TRGO, jtrg3
    TIM10_OC1, jtrg4 TIM2_TRGO, jtrg5 TIM4_TRGO, jtrg6 TIM11_OC1, jtrg7
    TIM6_TRGO, jtrg8 TIM7_TRGO, jtrg9 EXTI11, jtrg10 EXTI15.
  Footnote semantics unknown — if ST ever clarifies (e.g. F423-only
  sources), revisit.
  - DFSDM1 jtrg4/jtrg6/jtrg8 stay **reserved** (currently commented out in
    trigger.rs) — do not add sources.
  - F413 break mapping (both `DFSDM1_BREAK0` *and* `DFSDM2_BREAK0` → TIM1;
    same for TIM8) is a reasonable reading of rm0430 Table 90 (single
    unlabeled table) but re-confirm during execution.

- [ ] **SD6 (dormant) — LPTIM3_ETR ← DFSDM2_BREAK0 (H7A/B).**
  Row commented out in trigger.rs (commit 20055b8, "crashes the build.rs").
  LPTIM3 exists as a peripheral on H7A3/H7B3/H7B0; the blocker is that the
  LPTIM3 **ETR input signal** is not modeled in the signal tables. To enable:
  model LPTIM ETR input signals (or special-case), then uncomment. Not
  blocking anything else.

- [ ] **SD7 (minor) — MP1 RCC missing ADFSDM bits.** rm0436/rm0441/rm0442
  RCC APB2 have `ADFSDMEN`/`ADFSDMLPEN` (bit 21, audio-clock enable for
  DFSDM) next to DFSDMEN (bit 20). `rcc_mp1.yaml` has only DFSDM* fields.
  Needed for CKOUTSRC=audio on MP1; MP1 is not embassy-supported, minor.

- [ ] **SD8 (minor) — H7A/B DFSDM2 kernel clock not muxed.** Chip JSONs wire
  DFSDM2 kernel as fixed `PCLK4`, but rm0455 defines `DFSDM2SEL` (bit 27 mux)
  and `rcc_h7ab.yaml` already carries the field. Wire the kernel mux in the
  peripheral-to-clock mapping when DFSDM2 support lands in embassy.

- [ ] **SD9 (info, no action) — MP13 chips don't exist.** cubedb has
  STM32M131/M133/M135 with DFSDM (`dfsdm1_v1_0_4ch_MP13_Cube`) but no MP13
  chips are generated in `build/data/chips/`. The perimap MP13 regex
  (`DFSDM_4CH_2FLT_DLY_TRG5_ADC_HWID`) is correct but dormant. Embassy
  already has the matching variant.

---

## REGENERATE + VERIFY

- [ ] Re-run the data + metapac generation after SD1-SD5.
- [ ] Expected newly-enabled DFSDM chips: F777/F778/F779, L451/L452/L462,
  L471/L475/L476/L485/L486, L552/L562 (SD1), H7B0 trigger signals (SD4);
  F413 trigger names corrected (SD5).
- [ ] Expected blocks after fixes: L451/452/462 → `DFSDM_4CH_2FLT_TRG3`;
  L471-486 → `DFSDM_8CH_4FLT_TRG3`; F777-779 → `DFSDM_8CH_4FLT_TRG5`;
  L552/562 → `DFSDM_4CH_4FLT_DLY_TRG5_ADC` (existing regex, currently
  unreachable). Side effect: `DFSDM_2CH_1FLT_TRG3_ADC` and
  `DFSDM_4CH_2FLT_TRG3_ADC` become fully chip-less (already reflected on the
  embassy side in HOUSEKEEPING).
- [ ] Embassy-side check matrix tracked in
  `embassy-stm32/src/dfsdm/TODO-v2.md` → VERIFY.
