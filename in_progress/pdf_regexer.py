import argparse
import re
from pathlib import Path

import pdfplumber
import pandas as pd


def search_pdfs(pdf_dir: Path, field: str):
    results = []

    pattern = re.compile(
        rf"\b{re.escape(field)}\s*(?:\[\s*(\d+)\s*:\s*(\d+)\s*\])?",
        re.IGNORECASE,
    )

    for pdf_path in sorted(pdf_dir.glob("*.pdf")):
        print(f"Scanne {pdf_path.name} ...")

        try:
            with pdfplumber.open(pdf_path) as pdf:
                for page_no, page in enumerate(pdf.pages, start=1):
                    text = page.extract_text() or ""

                    for match in pattern.finditer(text):
                        msb = match.group(1)
                        lsb = match.group(2)

                        # Zeile mit Treffer
                        line_start = text.rfind("\n", 0, match.start()) + 1
                        line_end = text.find("\n", match.end())

                        if line_end == -1:
                            line_end = len(text)

                        line = text[line_start:line_end].strip()

                        results.append(
                            {
                                "PDF": pdf_path.name,
                                "Seite": page_no,
                                "Bitfeld": match.group(0),
                                "MSB": msb or "",
                                "LSB": lsb or "",
                                "Zeile": line,
                            }
                        )

        except Exception as e:
            print(f"  FEHLER: {e}")

    return results


def main():
    parser = argparse.ArgumentParser()

    parser.add_argument("field", help='Bitfeldname, z.B. "EXMIN"')

    parser.add_argument("directory", nargs="?", default=".")

    parser.add_argument("-o", "--output", default="bitfeld_treffer.csv")

    args = parser.parse_args()

    results = search_pdfs(Path(args.directory), args.field)

    if not results:
        print("\nKeine Treffer.")
        return

    df = pd.DataFrame(results)

    print()
    print(df.to_string(index=False))

    df.to_csv(args.output, index=False, encoding="utf-8-sig")

    print(f"\n{len(df)} Treffer.")
    print(f"Gespeichert: {args.output}")


if __name__ == "__main__":
    main()
