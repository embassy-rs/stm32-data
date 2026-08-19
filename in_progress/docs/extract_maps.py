#!/usr/bin/env python3
"""
Extract DFSDM register map section from STM32 Reference Manuals.
Starts at "register map" entry and goes to end of DFSDM chapter.

Usage:
    python extract_dfsdm_regmap.py [--input-dir DIR] [--output-dir DIR]
"""

import fitz  # PyMuPDF
import argparse
from pathlib import Path


def extract_dfsdm_regmap(pdf_path: Path, output_path: Path) -> Path | None:
    """Extract DFSDM register map section (from regmap entry to end of DFSDM chapter)."""
    doc = fitz.open(pdf_path)
    toc = doc.get_toc()

    # Find main DFSDM chapter (level 1) to determine chapter end
    main_chapter = None
    for level, title, page in toc:
        if level == 1 and "DFSDM" in title.upper():
            main_chapter = (level, title, page)
            break

    if not main_chapter:
        doc.close()
        return None

    # Find "register map" entry within DFSDM chapter
    regmap_entry = None
    for level, title, page in toc:
        title_upper = title.upper()
        if "DFSDM" in title_upper and (
            "REGISTER MAP" in title_upper or "REGISTERS" in title_upper
        ):
            if page >= main_chapter[2]:  # Must be within DFSDM chapter
                if "MAP" in title_upper:
                    regmap_entry = (level, title, page)
                    break
                elif regmap_entry is None:
                    regmap_entry = (level, title, page)

    if not regmap_entry:
        doc.close()
        return None

    start_page = regmap_entry[2] - 1  # 0-indexed

    # End: next level-1 entry after DFSDM chapter (same logic as before)
    end_page = doc.page_count
    found_start = False
    for level, title, page in toc:
        if page == main_chapter[2]:
            found_start = True
            continue
        if found_start and level == 1:
            end_page = page - 1
            break

    # Extract pages
    new_doc = fitz.open()
    for p in range(start_page, min(end_page, doc.page_count)):
        new_doc.insert_pdf(doc, from_page=p, to_page=p)

    new_doc.save(output_path)
    new_doc.close()
    doc.close()

    num_pages = end_page - start_page
    print(
        f"  {pdf_path.name}: {num_pages} pages extracted (regmap at page {start_page + 1})"
    )
    return output_path


def main():
    parser = argparse.ArgumentParser(
        description="Extract DFSDM register maps from STM32 RMs"
    )
    parser.add_argument(
        "--input-dir", default="./trms", help="Input directory with RM PDFs"
    )
    parser.add_argument(
        "--output-dir", default="./extracted_maps", help="Output directory"
    )
    args = parser.parse_args()

    input_dir = Path(args.input_dir)
    output_dir = Path(args.output_dir)
    output_dir.mkdir(exist_ok=True)

    pdf_files = sorted(input_dir.glob("*.pdf"))
    print(f"Found {len(pdf_files)} PDF files\n")

    success = 0
    skipped = 0

    for pdf_file in pdf_files:
        output_path = output_dir / f"{pdf_file.stem}_dfsdm_regmap.pdf"
        try:
            result = extract_dfsdm_regmap(pdf_file, output_path)
            if result:
                success += 1
            else:
                print(f"  {pdf_file.name}: No DFSDM register map found")
                skipped += 1
        except Exception as e:
            print(f"  {pdf_file.name}: ERROR - {e}")
            skipped += 1

    print(f"\nDone: {success} extracted_mp, {skipped} skipped")


if __name__ == "__main__":
    main()
