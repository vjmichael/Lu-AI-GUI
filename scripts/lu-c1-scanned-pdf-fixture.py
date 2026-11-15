#!/usr/bin/env python3
"""Deterministic CI-only, image-only two-page PDF for Lu's ORIGINAL OCR test.

Both pages are raster image XObjects: no searchable PDF text, font objects,
synthetic model answer, task-completion receipt, or user-owned file is written.
This is a disposable test source, not a production PDF authoring utility.
"""
import hashlib
import io
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont
from reportlab.lib.utils import ImageReader
from reportlab.pdfgen import canvas

WIDTH, HEIGHT = 1600, 480
LINES = ("ALPHA SOURCE PAGE", "BRAVO SOURCE PAGE")
FONT = Path("/usr/share/fonts/truetype/dejavu/DejaVuSansMono-Bold.ttf")


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit("usage: lu-c1-scanned-pdf-fixture.py <new .pdf path>")
    output = Path(sys.argv[1])
    if output.suffix != ".pdf" or output.exists() or not output.parent.is_dir():
        raise SystemExit("C1_SCANNED_FIXTURE_REFUSED: output must be new PDF in existing directory")
    font = ImageFont.truetype(str(FONT), 105)
    result = io.BytesIO()
    document = canvas.Canvas(result, pagesize=(WIDTH, HEIGHT), pageCompression=1, invariant=1)
    for line in LINES:
        raster = Image.new("RGB", (WIDTH, HEIGHT), "white")
        ImageDraw.Draw(raster).text((90, 148), line, font=font, fill="black")
        document.drawImage(ImageReader(raster), 0, 0, width=WIDTH, height=HEIGHT)
        document.showPage()
    document.save()
    payload = result.getvalue()
    if not payload.startswith(b"%PDF-") or b"%%EOF" not in payload[-2048:]:
        raise SystemExit("C1_SCANNED_FIXTURE_REFUSED: invalid generated PDF")
    # Exclusive output prevents overwriting a user file or prior acceptance.
    with output.open("xb") as handle:
        handle.write(payload)
    print("C1_ORIGINAL_SCANNED_IMAGE_ONLY_PDF_SHA256=" + hashlib.sha256(payload).hexdigest())


if __name__ == "__main__":
    main()
