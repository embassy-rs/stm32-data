# in_progress workspace

Scratch/analysis workspace for the DFSDM bring-up work. Everything here is
local working material — nothing in this directory is consumed by the build.

## Authority model

| Location | Status |
|---|---|
| `docs/trms/` | **Authoritative** — the 15 full TRM PDFs |
| `docs/extracted_chapters/` | **Authoritative** (derived, verbatim) — pdftotext-extracted DFSDM chapters of those TRMs, produced by `docs/extract_chapters.py` |
| `DFSDMx/TODO.md` | Working plan — the actionable result of the TRM audit (companion: `embassy-stm32/src/dfsdm/TODO-v2.md` on the HAL side) |
| `DFSDMx/extracts_nonauthoritative/` | **NOT authoritative** — mirrors of cubedb/CubeMX IP configs, hash diffs, layout notes |
| `scratch_nonauthoritative/` | **NOT authoritative** — bitfield-analysis tooling and its generated CSVs |

**Rule: TRMs are the source of truth.** Anything in a
`*_nonauthoritative/` directory may be stale, oversized, or plain wrong —
never use it as evidence for a data change. Known examples of CubeMX data
diverging from the TRMs (all compensated in the generator by TRM-derived
block mappings): F413 DFSDM1 sized 8ch/4f instead of 4ch/2f; L4P/Q sized
8ch/4f instead of 4ch/2f; H723 config missing CHxDLYR; L4P/L4R/H723 configs
carrying JEXTSEL[2:0] where the TRMs specify JEXTSEL[4:0].
