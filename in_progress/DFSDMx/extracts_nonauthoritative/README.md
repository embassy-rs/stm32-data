# NOT AUTHORITATIVE — cubedb/CubeMX extract mirrors

Everything in this directory is a mirror of `sources/cubedb/mcu/IP/`
config data (plus hash/notes from the study that produced it). It is study
material only — **the TRMs (`in_progress/docs/trms/`) are the source of
truth**. Never cite these files as evidence for a data change.

## Contents

- `*.yaml` (27 files) — register maps extracted per chip/config from the
  cubedb IP configs (e.g. `f413_DFSDM1.yaml` ←
  `DFSDM-dfsdm1_v1_0_4ch_F413_Cube`). 12 byte-unique files; the rest are
  duplicates: `l4x1 == l4x2 == l412 == f412`, `l4r9 == l4r5`,
  `l562 == l552`, `mp157 == mp153`, `f7x9 == f7x7`,
  `h7b3_DFSDM2 == h7b3_DFSDM1` (CubeMX hands DFSDM2 the full 8ch/8f map).
- `alle_hashes/` — per-extract sha256 names.
- `diffs.md` — the cubedb config inventory script (`DFSDM-...` → ConfigFile
  per family), hash dedup, TRM→chip mapping, and bunching by shape.
  Note: its script reads `sources/cubedb/mcu/*.xml` — run from the repo root.
- `layouts.md` — per-TRM register-layout notes (CH/FLT cluster offsets,
  HW_STUFF ranges).

## Known CubeMX errors found (vs the TRMs — compensated by block mapping)

- `f413_DFSDM1.yaml`: 8ch/4f content, TRM (rm0430) says DFSDM1 is 4ch/2f
  ("DFSDM1 (NBT=4, NBF=2)").
- `l4p5_DFSDM1.yaml`: 8ch/4f, TRM (rm0432 Table 184) says L4P/Q is 4ch/2f.
- `h735_DFSDM.yaml`: CH cluster missing DLYR entirely; rm0468 has CH0-7 DLYR.
- `l4p5`/`l4r5`/`h735`: JEXTSEL[2:0]; TRMs specify JEXTSEL[4:0].
- `diffs.md` bunching header for the h735/l4p5/l4r5 class says
  "JEXTSEL 4bit, EXMINCH/EXMAXCH 2bit" — the extracts actually contain
  3-bit / 3-bit.
