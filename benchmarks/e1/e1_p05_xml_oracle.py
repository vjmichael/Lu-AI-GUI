"""Independent bounded OOXML content witness for E1-P05 evidence.

Not a general DOCX/XLSX parser: this fail-closed CPython check verifies the
benchmark's fixed expected contents separately from the native Rust reader.
"""
from pathlib import Path
import xml.etree.ElementTree as ET
import zipfile

WORD = "http://schemas.openxmlformats.org/wordprocessingml/2006/main"
SHEET = "http://schemas.openxmlformats.org/spreadsheetml/2006/main"
REL = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
PKG_REL = "http://schemas.openxmlformats.org/package/2006/relationships"
MAX_ARCHIVE_BYTES = 1024 * 1024
MAX_XML_PART_BYTES = 256 * 1024
MAX_ARCHIVE_PARTS = 16

def check_ooxml_exact(path: Path, kind: str) -> None:
    """Require exact fixed benchmark semantics, not just a valid ZIP header."""
    if kind not in ("docx", "xlsx") or not path.is_file():
        raise RuntimeError("Invalid independent OOXML witness input")
    if path.stat().st_size > MAX_ARCHIVE_BYTES:
        raise RuntimeError("OOXML archive exceeds benchmark bound")
    try:
        with zipfile.ZipFile(path) as archive:
            infos = archive.infolist()
            names = [info.filename for info in infos]
            if not names or len(names) > MAX_ARCHIVE_PARTS or len(names) != len(set(names)):
                raise RuntimeError("Duplicate or excessive OOXML ZIP parts")
            if any(info.is_dir() or info.file_size > MAX_XML_PART_BYTES or
                   info.filename.startswith("/") or "\\" in info.filename or
                   any(part in ("", ".", "..") for part in info.filename.split("/"))
                   for info in infos):
                raise RuntimeError("Unsafe or oversized OOXML ZIP entry")
            if sum(info.file_size for info in infos) > MAX_ARCHIVE_BYTES:
                raise RuntimeError("Excessive expanded OOXML payload")
            def part(name: str) -> ET.Element:
                if name not in names:
                    raise RuntimeError("Missing required OOXML part: " + name)
                raw = archive.read(name)
                if len(raw) > MAX_XML_PART_BYTES or b"<!DOCTYPE" in raw.upper():
                    raise RuntimeError("Unsafe OOXML XML content")
                try:
                    return ET.fromstring(raw)
                except ET.ParseError as exc:
                    raise RuntimeError("Malformed OOXML XML content") from exc
            # Follow the package root relation, not an unrelated orphan part.
            root_links = part("_rels/.rels")
            office = [r.attrib.get("Target") for r in root_links.findall(
                f"{{{PKG_REL}}}Relationship") if r.attrib.get("Type", "").endswith("/officeDocument")]
            if office != ["word/document.xml" if kind == "docx" else "xl/workbook.xml"]:
                raise RuntimeError("OOXML package root does not identify inspected document")
            part("[Content_Types].xml")
            if kind == "docx":
                document = part("word/document.xml")
                if document.tag != f"{{{WORD}}}document":
                    raise RuntimeError("Invalid Word document root")
                body = document.find(f"{{{WORD}}}body")
                if body is None:
                    raise RuntimeError("Missing Word body")
                paragraphs = ["".join(t.text or "" for t in p.iter(f"{{{WORD}}}t"))
                              for p in body.findall(f"{{{WORD}}}p")]
                if paragraphs != ["Lu E1 artifact custody verified"]:
                    raise RuntimeError("Word paragraph differs from approved benchmark")
            else:
                workbook = part("xl/workbook.xml")
                sheets = workbook.findall(f"{{{SHEET}}}sheets/{{{SHEET}}}sheet")
                if len(sheets) != 1 or sheets[0].attrib.get("name") != "E1":
                    raise RuntimeError("Unexpected workbook sheet identity")
                relation = sheets[0].attrib.get(f"{{{REL}}}id")
                links = part("xl/_rels/workbook.xml.rels")
                targets = [r.attrib.get("Target") for r in links.findall(
                    f"{{{PKG_REL}}}Relationship") if r.attrib.get("Id") == relation]
                if targets != ["worksheets/sheet1.xml"]:
                    raise RuntimeError("Workbook sheet relationship does not match")
                strings = part("xl/sharedStrings.xml")
                strings_list = ["".join(t.text or "" for t in si.iter(f"{{{SHEET}}}t"))
                                for si in strings.findall(f"{{{SHEET}}}si")]
                sheet = part("xl/worksheets/sheet1.xml")
                cells = sheet.findall(f"{{{SHEET}}}sheetData/{{{SHEET}}}row/{{{SHEET}}}c")
                actual = {}
                for c in cells:
                    addr = c.attrib.get("r")
                    if not addr or addr in actual:
                        raise RuntimeError("Missing or duplicate worksheet cell address")
                    v = c.find(f"{{{SHEET}}}v")
                    if v is None or v.text is None:
                        raise RuntimeError("Missing worksheet cell value")
                    if c.attrib.get("t") == "s":
                        try:
                            index = int(v.text)
                            value = strings_list[index] if index >= 0 else None
                        except (ValueError, IndexError) as exc:
                            raise RuntimeError("Invalid shared-string reference") from exc
                    elif c.attrib.get("t") == "n":
                        try:
                            value = float(v.text)
                        except ValueError as exc:
                            raise RuntimeError("Invalid numeric cell") from exc
                    else:
                        raise RuntimeError("Unexpected worksheet cell type")
                    actual[addr] = value
                if actual != {"A1": "Status", "B1": "Reviewed", "A2": "Frames", "B2": 48.0}:
                    raise RuntimeError("Worksheet cells differ from approved benchmark")
    except (zipfile.BadZipFile, EOFError, OSError) as exc:
        raise RuntimeError("Invalid physical OOXML benchmark artifact") from exc
