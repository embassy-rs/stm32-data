#!/usr/bin/env python3
"""
Extract DFSDM register maps from STM32 Reference Manuals.

Usage:
    python extract_dfsdm.py [--input-dir DIR] [--output-dir DIR]
"""

import fitz  # PyMuPDF
import argparse
from pathlib import Path


def extract_dfsdm(pdf_path: Path, output_path: Path) -> Path | None:
    """Extract DFSDM chapter from a single RM PDF."""
    doc = fitz.open(pdf_path)
    toc = doc.get_toc()

    # Find main DFSDM chapter (level 1)
    main_chapter = None
    for level, title, page in toc:
        if level == 1 and "DFSDM" in title.upper():
            main_chapter = (level, title, page)
            break

    if not main_chapter:
        doc.close()
        return None

    start_page = main_chapter[2] - 1  # 0-indexed

    # Find end: next level-1 entry after DFSDM chapter
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
    print(f"  {pdf_path.name}: {num_pages} pages extracted")
    return output_path


def main():
    parser = argparse.ArgumentParser(
        description="Extract DFSDM register maps from STM32 RMs"
    )
    parser.add_argument(
        "--input-dir", default="./trms", help="Input directory with RM PDFs"
    )
    parser.add_argument(
        "--output-dir", default="./extracted_chapters", help="Output directory"
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
        output_path = output_dir / f"{pdf_file.stem}_dfsdm.pdf"
        try:
            result = extract_dfsdm(pdf_file, output_path)
            if result:
                success += 1
            else:
                print(f"  {pdf_file.name}: No DFSDM chapter found")
                skipped += 1
        except Exception as e:
            print(f"  {pdf_file.name}: ERROR - {e}")
            skipped += 1

    print(f"\nDone: {success} extracted, {skipped} skipped")


if __name__ == "__main__":
    main()
