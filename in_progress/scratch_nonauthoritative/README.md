# NOT AUTHORITATIVE — bitfield analysis tooling + generated outputs

uv-managed Python project used during the DFSDM study: bitfield extraction
and comparison across the cubedb extracts (`../DFSDMx/extracts_nonauthoritative/`)
and the PDFs (`../docs/`). Outputs and inputs alike are scratch — the TRMs
are the source of truth.

## Contents

- Tooling: `main.py`, `pdf_regexer.py`, `bitfield_deduper.py`,
  `compare_bitfields.py`, `search_bitfield_combinations.py`
  (`pyproject.toml` / `uv.lock` / `.python-version` moved with the project).
- Generated CSVs: `bitfield*.csv`, `bitfeld_treffer.csv` (9 files).

Run from this directory (`uv run ...`); the CSVs are regenerable outputs.
