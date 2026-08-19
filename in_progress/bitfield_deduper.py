import argparse
import re
from pathlib import Path
from collections import defaultdict

import pandas as pd

BITFIELD_RE = re.compile(
    r"\b([A-Za-z_][A-Za-z0-9_]*)\s*" r"\[\s*(\d+)\s*(?::\s*(\d+)\s*)?\s*\]",
    re.IGNORECASE,
)


def scan_pdfs(pdf_dir: Path):
    # field -> set of (msb, lsb)
    ranges = defaultdict(set)

    # field -> set of PDF names
    pdfs = defaultdict(set)

    # field -> set of (PDF, page)
    locations = defaultdict(set)

    occurrences = []

    for pdf_path in sorted(pdf_dir.glob("*.pdf")):
        print(f"Scanne {pdf_path.name} ...")

        try:
            with __import__("pdfplumber").open(pdf_path) as pdf:

                for page_no, page in enumerate(pdf.pages, start=1):
                    text = page.extract_text() or ""

                    for match in BITFIELD_RE.finditer(text):
                        name = match.group(1)

                        msb = int(match.group(2))
                        lsb = int(match.group(3)) if match.group(3) is not None else msb

                        key = name.upper()

                        ranges[key].add((msb, lsb))
                        pdfs[key].add(pdf_path.name)
                        locations[key].add((pdf_path.name, page_no))

                        occurrences.append(
                            {
                                "Bitfeld": key,
                                "MSB": msb,
                                "LSB": lsb,
                                "PDF": pdf_path.name,
                                "Seite": page_no,
                                "Schreibweise": match.group(0),
                            }
                        )

        except Exception as e:
            print(f"  FEHLER: {e}")

    return ranges, pdfs, locations, occurrences


def format_ranges(ranges):
    def sort_key(r):
        msb, lsb = r
        return (-msb, -lsb)

    return ", ".join(f"[{msb}:{lsb}]" for msb, lsb in sorted(ranges, key=sort_key))


def main():
    parser = argparse.ArgumentParser(
        description="Extrahiert alle Bitfield-Schreibweisen aus PDFs."
    )

    parser.add_argument(
        "directory", nargs="?", default=".", help="Verzeichnis mit PDFs"
    )

    args = parser.parse_args()

    pdf_dir = Path(args.directory)

    ranges, pdfs, locations, occurrences = scan_pdfs(pdf_dir)

    # ---------------------------------------------------------
    # Summary
    # ---------------------------------------------------------

    summary = []

    for field in sorted(ranges):
        field_ranges = ranges[field]
        field_pdfs = sorted(pdfs[field], key=str.lower)

        summary.append(
            {
                "Bitfeld": field,
                "Varianten": format_ranges(field_ranges),
                "Anzahl Varianten": len(field_ranges),
                "PDFs": "; ".join(field_pdfs),
                "Anzahl PDFs": len(field_pdfs),
                "Fundstellen": len(locations[field]),
            }
        )

    summary_df = pd.DataFrame(summary)

    summary_df.to_csv(
        "bitfield_summary.csv",
        index=False,
        encoding="utf-8-sig",
    )

    # ---------------------------------------------------------
    # Einzelne Fundstellen
    # ---------------------------------------------------------

    occurrences_df = pd.DataFrame(occurrences)

    occurrences_df.to_csv(
        "bitfield_occurrences.csv",
        index=False,
        encoding="utf-8-sig",
    )

    # ---------------------------------------------------------
    # Ausgabe
    # ---------------------------------------------------------

    print()
    print("=" * 80)
    print(f"{len(summary_df)} eindeutige Bitfelder gefunden")
    print("=" * 80)
    print()

    if not summary_df.empty:
        print(
            summary_df[
                [
                    "Bitfeld",
                    "Varianten",
                    "Anzahl PDFs",
                    "Fundstellen",
                ]
            ].to_string(index=False)
        )

    print()
    print("Geschrieben:")
    print("  bitfield_summary.csv")
    print("  bitfield_occurrences.csv")


if __name__ == "__main__":
    main()
