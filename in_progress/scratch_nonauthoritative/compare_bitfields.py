import argparse
import re
from pathlib import Path
from collections import defaultdict

import pandas as pd
import pdfplumber

# ============================================================
# Parser
# ============================================================
#
# Erkennt z.B.:
#
#   Bit 2 JOVRIE
#   Bits 23:16 AWDCH[7:0]
#   Bits 15:8 Reserved
#   Bit 7 Reserved
#   Bits 31:24 FOO
#
# "Reserved" wird exakt wie ein normales Bitfeld behandelt.
#

ROW_RE = re.compile(
    r"""
    \bBits?\s+
    (?P<reg_msb>\d+)
    (?:\s*:\s*(?P<reg_lsb>\d+))?
    \s+
    (?P<name>
        Reserved
        |
        [A-Za-z_][A-Za-z0-9_]*
    )
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


# ============================================================
# Hilfsfunktionen
# ============================================================


def bitset(msb, lsb):
    """Alle Bits eines Bereichs als Set."""
    if msb < lsb:
        msb, lsb = lsb, msb

    return set(range(lsb, msb + 1))


def range_text(msb, lsb):
    if msb == lsb:
        return str(msb)

    return f"{msb}:{lsb}"


def field_range_text(field_msb, field_lsb):
    if field_msb is None:
        return ""

    if field_msb == field_lsb:
        return str(field_msb)

    return f"{field_msb}:{field_lsb}"


def normalize_pdf_name(name):
    """
    Optional: macht aus langen TRM-Dateinamen einen kurzen Namen.

    Beispiel:
        RM0433-STM32G4.pdf -> RM0433-STM32G4

    Falls du später eigene Family-Namen möchtest, kann man
    diese Funktion entsprechend erweitern.
    """
    return Path(name).stem


# ============================================================
# PDF scannen
# ============================================================


def scan_pdfs(pdf_dir):
    records = []

    pdf_files = sorted(pdf_dir.glob("*.pdf"))

    if not pdf_files:
        print(f"Keine PDFs gefunden: {pdf_dir}")
        return records

    for pdf_path in pdf_files:

        print(f"Scanne {pdf_path.name} ...")

        try:

            with pdfplumber.open(pdf_path) as pdf:

                for page_no, page in enumerate(
                    pdf.pages,
                    start=1,
                ):

                    text = page.extract_text() or ""

                    for match in ROW_RE.finditer(text):

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
                            field_msb = int(field_msb)
                            field_lsb = int(field_lsb)

                        records.append(
                            {
                                "PDF": pdf_path.name,
                                "Family": normalize_pdf_name(pdf_path.name),
                                "Seite": page_no,
                                "Bitfeld": name,
                                "Register_MSB": reg_msb,
                                "Register_LSB": reg_lsb,
                                "Feld_MSB": field_msb,
                                "Feld_LSB": field_lsb,
                            }
                        )

        except Exception as e:

            print(f"  FEHLER in {pdf_path.name}: {e}")

    return records


# ============================================================
# Rohdaten
# ============================================================


def make_raw_dataframe(records):

    df = pd.DataFrame(records)

    if df.empty:
        return df

    df["Registerbits"] = df.apply(
        lambda x: range_text(
            x.Register_MSB,
            x.Register_LSB,
        ),
        axis=1,
    )

    df["Feldbits"] = df.apply(
        lambda x: field_range_text(
            x.Feld_MSB,
            x.Feld_LSB,
        ),
        axis=1,
    )

    return df


# ============================================================
# Matrix
# ============================================================


def make_matrix(df):

    if df.empty:
        return pd.DataFrame()

    fields = sorted(
        set(df["Bitfeld"]),
        key=str.lower,
    )

    pdfs = sorted(
        set(df["PDF"]),
        key=str.lower,
    )

    rows = []

    for field in fields:

        row = {"Bitfeld": field}

        for pdf in pdfs:

            subset = df[(df["Bitfeld"] == field) & (df["PDF"] == pdf)]

            values = set()

            for _, x in subset.iterrows():

                reg = range_text(
                    x.Register_MSB,
                    x.Register_LSB,
                )

                if pd.notna(x.Feld_MSB):

                    field_range = range_text(
                        int(x.Feld_MSB),
                        int(x.Feld_LSB),
                    )

                    values.add(f"{reg} -> [{field_range}]")

                else:

                    values.add(reg)

            row[pdf] = "; ".join(sorted(values))

        rows.append(row)

    return pd.DataFrame(rows)


# ============================================================
# Register-/Seiten-Gruppen
# ============================================================


def build_groups(df):
    """
    Gruppiert Einträge zunächst nach PDF + Seite.

    Das ist absichtlich konservativ.

    Ohne Tabellen-Camelot wissen wir nicht sicher,
    welches Register zu welcher Tabellenzeile gehört.

    Für die Analyse werden daher Bitbereiche betrachtet,
    die auf derselben Seite vorkommen.
    """

    groups = []

    for (pdf, page), group in df.groupby(["PDF", "Seite"]):

        groups.append(
            (
                pdf,
                page,
                group.copy(),
            )
        )

    return groups


# ============================================================
# Analyse eines Feldes
# ============================================================


def analyse_field(
    field_row,
    group,
):

    field_name = field_row["Bitfeld"]

    if field_name == "RESERVED":
        return None

    reg_msb = int(field_row["Register_MSB"])

    reg_lsb = int(field_row["Register_LSB"])

    field_bits = bitset(
        reg_msb,
        reg_lsb,
    )

    # --------------------------------------------------------
    # Alles, was in dieser Gruppe als RESERVED markiert ist
    # --------------------------------------------------------

    reserved_bits = set()

    for _, row in group.iterrows():

        if row["Bitfeld"] != "RESERVED":
            continue

        reserved_bits |= bitset(
            int(row["Register_MSB"]),
            int(row["Register_LSB"]),
        )

    # --------------------------------------------------------
    # Alles, was durch andere Felder belegt ist
    # --------------------------------------------------------

    used_by_other = set()

    for _, row in group.iterrows():

        if row["Bitfeld"] in (
            "RESERVED",
            field_name,
        ):
            continue

        used_by_other |= bitset(
            int(row["Register_MSB"]),
            int(row["Register_LSB"]),
        )

    # --------------------------------------------------------
    # Low byte
    # --------------------------------------------------------

    low_byte = {b for b in field_bits if b < 8}

    # Falls das Feld oberhalb des Low Bytes liegt:
    # betrachten wir die Bits darunter bis Bit 0.
    if not low_byte:

        lower_bits = set(
            range(
                0,
                min(field_bits) if field_bits else 0,
            )
        )

    else:

        lower_bits = set()

    # --------------------------------------------------------
    # Status für die darunterliegenden Bits
    # --------------------------------------------------------

    if not lower_bits:

        low_status = "NOT_APPLICABLE"

    else:

        reserved_lower = lower_bits & reserved_bits

        used_lower = lower_bits & used_by_other

        unknown_lower = lower_bits - reserved_lower - used_lower

        if reserved_lower == lower_bits:
            low_status = "RESERVED"

        elif used_lower:
            low_status = "USED"

        elif unknown_lower:
            low_status = "UNDOCUMENTED"

        else:
            low_status = "UNKNOWN"

    # --------------------------------------------------------
    # Feldbreite
    # --------------------------------------------------------

    field_width = abs(reg_msb - reg_lsb) + 1

    declared_field_width = None

    if pd.notna(field_row["Feld_MSB"]):

        declared_field_width = (
            abs(int(field_row["Feld_MSB"]) - int(field_row["Feld_LSB"])) + 1
        )

    # --------------------------------------------------------
    # Ergebnis
    # --------------------------------------------------------

    return {
        "PDF": field_row["PDF"],
        "Seite": field_row["Seite"],
        "Bitfeld": field_name,
        "Registerbits": range_text(
            reg_msb,
            reg_lsb,
        ),
        "Feldbits": (
            field_range_text(
                field_row["Feld_MSB"],
                field_row["Feld_LSB"],
            )
        ),
        "Registerbreite": field_width,
        "Feldbreite": (
            declared_field_width if declared_field_width is not None else ""
        ),
        "LowByte_Status": low_status,
        "Darunter": (
            range_text(
                max(field_bits) if field_bits else 0,
                0,
            )
            if lower_bits
            else ""
        ),
        "Reserved_Bits_darunter": (
            ",".join(
                str(x)
                for x in sorted(
                    reserved_lower,
                    reverse=True,
                )
            )
            if lower_bits
            else ""
        ),
        "Andere_Felder_darunter": (
            ",".join(
                str(x)
                for x in sorted(
                    used_lower,
                    reverse=True,
                )
            )
            if lower_bits
            else ""
        ),
        "Undokumentierte_Bits_darunter": (
            ",".join(
                str(x)
                for x in sorted(
                    unknown_lower,
                    reverse=True,
                )
            )
            if lower_bits
            else ""
        ),
    }


# ============================================================
# Analyse komplett
# ============================================================


def make_analysis(df):

    if df.empty:
        return pd.DataFrame()

    results = []

    for pdf, page, group in build_groups(df):

        for _, field in group.iterrows():

            result = analyse_field(
                field,
                group,
            )

            if result:
                results.append(result)

    return pd.DataFrame(results)


# ============================================================
# Cross-PDF Summary
# ============================================================


def make_summary(analysis):

    if analysis.empty:
        return pd.DataFrame()

    rows = []

    for field, group in analysis.groupby("Bitfeld"):

        statuses = sorted(set(group["LowByte_Status"]))

        rows.append(
            {
                "Bitfeld": field,
                "PDFs": "; ".join(
                    sorted(
                        group["PDF"].unique(),
                        key=str.lower,
                    )
                ),
                "Registerbits": "; ".join(sorted(set(group["Registerbits"]))),
                "Feldbits": "; ".join(sorted(set(x for x in group["Feldbits"] if x))),
                "LowByte_Status": "; ".join(statuses),
                "Anzahl_PDFs": (group["PDF"].nunique()),
                "Anzahl_Varianten": len(
                    group[
                        [
                            "Registerbits",
                            "Feldbits",
                        ]
                    ].drop_duplicates()
                ),
            }
        )

    return pd.DataFrame(rows)


# ============================================================
# Main
# ============================================================


def main():

    parser = argparse.ArgumentParser(
        description=("Vergleicht Bitfield-Definitionen " "über mehrere STM32-TRMs.")
    )

    parser.add_argument(
        "directory",
        nargs="?",
        default=".",
        help="Verzeichnis mit PDFs",
    )

    args = parser.parse_args()

    pdf_dir = Path(args.directory)

    # --------------------------------------------------------
    # Scan
    # --------------------------------------------------------

    records = scan_pdfs(pdf_dir)

    if not records:

        print("\nKeine passenden Bitfield-Zeilen gefunden.")

        return

    # --------------------------------------------------------
    # DataFrame
    # --------------------------------------------------------

    df = make_raw_dataframe(records)

    # --------------------------------------------------------
    # Rohdaten
    # --------------------------------------------------------

    df.to_csv(
        "bitfield_rows.csv",
        index=False,
        encoding="utf-8-sig",
    )

    # --------------------------------------------------------
    # Matrix
    # --------------------------------------------------------

    matrix = make_matrix(df)

    matrix.to_csv(
        "bitfield_matrix.csv",
        index=False,
        encoding="utf-8-sig",
    )

    # --------------------------------------------------------
    # Analyse
    # --------------------------------------------------------

    analysis = make_analysis(df)

    analysis.to_csv(
        "bitfield_analysis.csv",
        index=False,
        encoding="utf-8-sig",
    )

    # --------------------------------------------------------
    # Summary
    # --------------------------------------------------------

    summary = make_summary(analysis)

    summary.to_csv(
        "bitfield_summary.csv",
        index=False,
        encoding="utf-8-sig",
    )

    # --------------------------------------------------------
    # Ausgabe
    # --------------------------------------------------------

    print()
    print("=" * 80)
    print("FERTIG")
    print("=" * 80)
    print()

    print(f"Gefundene Tabellenzeilen: {len(df)}")

    print(f"Bitfelder: {df['Bitfeld'].nunique()}")

    print()
    print("Dateien:")
    print()
    print("  bitfield_matrix.csv")
    print("  bitfield_summary.csv")
    print("  bitfield_analysis.csv")
    print("  bitfield_rows.csv")


if __name__ == "__main__":
    main()
