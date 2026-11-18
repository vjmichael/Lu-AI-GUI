"""Adversarial structural fixtures; never claimed as native-worker results."""
import io
from pathlib import Path
import tempfile
import unittest
import zipfile
from e1_p05_xml_oracle import check_ooxml_exact

ROOT_REL = ('<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
            '<Relationship Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="{target}"/></Relationships>')
CT = '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"/>'
WORD = 'http://schemas.openxmlformats.org/wordprocessingml/2006/main'
SHEET = 'http://schemas.openxmlformats.org/spreadsheetml/2006/main'
PKG = 'http://schemas.openxmlformats.org/package/2006/relationships'

def fixture(kind, changed=False):
    if kind == 'docx':
        files = {'[Content_Types].xml': CT,
            '_rels/.rels': ROOT_REL.format(target='word/document.xml'),
            'word/document.xml': f'<w:document xmlns:w="{WORD}"><w:body><w:p><w:r><w:t>' +
            ('Wrong text' if changed else 'Lu E1 artifact custody verified') +
            '</w:t></w:r></w:p></w:body></w:document>'}
    else:
        strings = ['Status', 'Reviewed', 'Frames']
        sheet = ('<worksheet xmlns="{ns}"><sheetData><row r="1">'
                 '<c r="A1" t="s"><v>0</v></c><c r="B1" t="s"><v>1</v></c>'
                 '</row><row r="2"><c r="A2" t="s"><v>2</v></c>'
                 '<c r="B2" t="n"><v>{n}</v></c></row></sheetData></worksheet>').format(
                     ns=SHEET,n='47' if changed else '48')
        files = {'[Content_Types].xml': CT,
            '_rels/.rels': ROOT_REL.format(target='xl/workbook.xml'),
            'xl/workbook.xml': f'<workbook xmlns="{SHEET}" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="E1" r:id="rId1"/></sheets></workbook>',
            'xl/_rels/workbook.xml.rels': f'<Relationships xmlns="{PKG}"><Relationship Id="rId1" Target="worksheets/sheet1.xml"/></Relationships>',
            'xl/worksheets/sheet1.xml': sheet,
            'xl/sharedStrings.xml': f'<sst xmlns="{SHEET}">'+''.join(
                f'<si><t>{v}</t></si>' for v in strings)+'</sst>'}
    buffer=io.BytesIO()
    with zipfile.ZipFile(buffer,'w') as archive:
        for name,content in files.items():archive.writestr(name,content)
    return buffer.getvalue()

class TestIndependentOOXMLWitness(unittest.TestCase):
    def test_approved_docx_and_xlsx(self):
        with tempfile.TemporaryDirectory() as d:
            for kind in ('docx','xlsx'):
                f=Path(d)/('test.'+kind)
                f.write_bytes(fixture(kind))
                check_ooxml_exact(f,kind)
    def test_valid_zip_with_wrong_content_rejected(self):
        with tempfile.TemporaryDirectory() as d:
            for kind in ('docx','xlsx'):
                f=Path(d)/('wrong.'+kind)
                f.write_bytes(fixture(kind,changed=True))
                with self.assertRaisesRegex(RuntimeError,'differ'):
                    check_ooxml_exact(f,kind)
    def test_fake_and_duplicate_part_rejected(self):
        with tempfile.TemporaryDirectory() as d:
            f=Path(d)/'bad.docx'
            f.write_bytes(b'PK\\x03\\x04 nonsense')
            with self.assertRaises(RuntimeError):check_ooxml_exact(f,'docx')
            buffer=io.BytesIO()
            with zipfile.ZipFile(buffer,'w') as archive:
                archive.writestr('word/document.xml','one')
                archive.writestr('word/document.xml','two')
            f.write_bytes(buffer.getvalue())
            with self.assertRaisesRegex(RuntimeError,'Duplicate'):
                check_ooxml_exact(f,'docx')
    def test_orphaned_xml_part_rejected(self):
        with tempfile.TemporaryDirectory() as d:
            f=Path(d)/'orphan.docx'
            raw=fixture('docx')
            b=io.BytesIO()
            with zipfile.ZipFile(io.BytesIO(raw)) as src,zipfile.ZipFile(b,'w') as dest:
                for name in src.namelist():
                    content=src.read(name)
                    if name=='_rels/.rels':
                        content=ROOT_REL.format(target='word/other.xml').encode()
                    dest.writestr(name,content)
            f.write_bytes(b.getvalue())
            with self.assertRaisesRegex(RuntimeError,'root'):
                check_ooxml_exact(f,'docx')

if __name__ == '__main__':unittest.main()
