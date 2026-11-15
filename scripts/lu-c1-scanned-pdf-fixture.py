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
from reportlab import rl_config
from reportlab.lib.utils import ImageReader
from reportlab.pdfgen import canvas

# The native Reader accepts Flate-compressed bounded classic streams, not
# ASCII85-wrapped Flate. A real image-only PDF must first return its
# intentional UNSUPPORTED classification, not an invalid-decoder error.
rl_config.useA85 = 0
WIDTH, HEIGHT = 1300, 400
LINES = ("ALPHA SOURCE PAGE", "BRAVO SOURCE PAGE")
FONT = Path("/usr/share/fonts/truetype/dejavu/DejaVuSansMono-Bold.ttf")


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit("usage: lu-c1-scanned-pdf-fixture.py <new .pdf path>")
    output = Path(sys.argv[1])
    if output.suffix != ".pdf" or output.exists() or not output.parent.is_dir():
        raise SystemExit("C1_SCANNED_FIXTURE_REFUSED: output must be new PDF in existing directory")
    font = ImageFont.truetype(str(FONT), 90)
    result = io.BytesIO()
    document = canvas.Canvas(result, pagesize=(WIDTH, HEIGHT), pageCompression=1, invariant=1)
    for line in LINES:
        raster = Image.new("RGB", (WIDTH, HEIGHT), "white")
        ImageDraw.Draw(raster).text((70, 120), line, font=font, fill="black")
        document.drawImage(ImageReader(raster), 0, 0, width=WIDTH, height=HEIGHT)
        document.showPage()
    document.save()
    payload = result.getvalue()
    if not payload.startswith(b"%PDF-") or b"%%EOF" not in payload[-2048:]:
        raise SystemExit("C1_SCANNED_FIXTURE_REFUSED: invalid generated PDF")
    # The conservative native Reader decodes each Flate image stream to a
    # bounded RGB raster, even though text extraction must return UNSUPPORTED.
    if WIDTH * HEIGHT * 3 > 2 * 1024 * 1024:
        raise SystemExit("C1_SCANNED_FIXTURE_REFUSED: decoded raster too large")
    if b"ASCII85Decode" in payload:
        raise SystemExit("C1_SCANNED_FIXTURE_REFUSED: unsupported ASCII85 wrapper")
    # Exclusive output prevents overwriting a user file or prior acceptance.
    with output.open("xb") as handle:
        handle.write(payload)
    print("C1_ORIGINAL_SCANNED_IMAGE_ONLY_PDF_SHA256=" + hashlib.sha256(payload).hexdigest())


if __name__ == "__main__":
    main()
