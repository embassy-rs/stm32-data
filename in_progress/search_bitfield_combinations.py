import argparse
import re
from pathlib import Path
from collections import defaultdict

import pandas as pd

# Beispiele:
#
#   Bits 23:16 AWDCH[7:0]:
#   Bits 23:16 AWDCH:
#   Bit 2 JOVRIE
#   Bit 31 EN
#
BITFIELD_RE = re.compile(
    r"""
    \bBits?\s+
    (?P<reg_msb>\d+)
    (?:\s*:\s*(?P<reg_lsb>\d+))?
    \s+
    (?P<name>[A-Za-z_][A-Za-z0-9_]*)
    \s*
    (?:\[
        \s*(?P<field_msb>\d+)
        \s*:\s*
        (?P<field_lsb>\d+)
        \s*
    \])?
    """,
    re.IGNORECASE | re.VERBOSE,
)


def scan_pdfs(pdf_dir: Path):
    # (name, register range, field range) -> PDFs
    combinations = defaultdict(set)

    occurrences = []

    for pdf_path in sorted(pdf_dir.glob("*.pdf")):
        print(f"Scanne {pdf_path.name} ...")

        try:
            import pdfplumber

            with pdfplumber.open(pdf_path) as pdf:

                for page_no, page in enumerate(pdf.pages, start=1):
                    text = page.extract_text() or ""

                    for match in BITFIELD_RE.finditer(text):

                        reg_msb = int(match.group("reg_msb"))
                        reg_lsb = (
                            int(match.group("reg_lsb"))
                            if match.group("reg_lsb")
                            else reg_msb
                        )

                        name = match.group("name").upper()

                        field_msb = match.group("field_msb")
                        field_lsb = match.group("field_lsb")

                        if field_msb is not None:
                            field_range = f"{field_msb}:{field_lsb}"
                        else:
                            field_range = ""

                        reg_range = (
                            f"{reg_msb}:{reg_lsb}"
                            if reg_msb != reg_lsb
                            else str(reg_msb)
                        )

                        key = (
                            name,
                            reg_range,
                            field_range,
                        )

                        combinations[key].add(pdf_path.name)

                        occurrences.append(
                            {
                                "PDF": pdf_path.name,
                                "Seite": page_no,
                                "Bitfeld": name,
                                "Registerbits": reg_range,
                                "Feldbits": field_range,
                            }
                        )

        except Exception as e:
            print(f"  FEHLER: {e}")

    return combinations, occurrences


def main():

    parser = argparse.ArgumentParser()

    parser.add_argument(
        "directory",
        nargs="?",
        default=".",
        help="Verzeichnis mit PDFs",
    )

    args = parser.parse_args()

    combinations, occurrences = scan_pdfs(Path(args.directory))

    # ---------------------------------------------------------
    # Summary
    # ---------------------------------------------------------

    summary = []

    for (name, reg_range, field_range), pdfs in sorted(combinations.items()):

        summary.append(
            {
                "Bitfeld": name,
                "Registerbits": reg_range,
                "Feldbits": field_range,
                "Kombination": (
                    f"{reg_range} -> " f"[{field_range}]" if field_range else reg_range
                ),
                "PDFs": "; ".join(sorted(pdfs, key=str.lower)),
                "Anzahl PDFs": len(pdfs),
            }
        )

    summary_df = pd.DataFrame(summary)

    summary_df.to_csv(
        "bitfield_combinations.csv",
        index=False,
        encoding="utf-8-sig",
    )

    # ---------------------------------------------------------
    # Einzelne Fundstellen
    # ---------------------------------------------------------

    occurrences_df = pd.DataFrame(occurrences)

    occurrences_df.to_csv(
        "bitfield_combination_occurrences.csv",
        index=False,
        encoding="utf-8-sig",
    )

    print()
    print(f"{len(summary_df)} eindeutige " f"Bitfeld-Kombinationen gefunden.")
    print()

    if not summary_df.empty:
        print(summary_df.to_string(index=False))

    print()
    print("Geschrieben:")
    print("  bitfield_combinations.csv")
    print("  bitfield_combination_occurrences.csv")


if __name__ == "__main__":
    main()
