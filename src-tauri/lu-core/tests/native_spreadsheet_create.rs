use cap_std::{ambient_authority,fs::Dir};
use lu_core::native_spreadsheet_create::{
    CellValue,SpreadsheetCell,SpreadsheetWriteError,create_spreadsheet_new_in_dir,
    render_csv,render_xlsx,
};
use std::{fs,io::Read,sync::atomic::{AtomicU64,Ordering}};
static COUNT:AtomicU64=AtomicU64::new(0);
fn cell(row:u32,column:u16,value:CellValue)->SpreadsheetCell {
    SpreadsheetCell{row,column,value}
}
fn fixture()->(std::path::PathBuf,Dir){
    let p=std::env::temp_dir().join(format!(
        "lu-sheet-{}-{}",std::process::id(),COUNT.fetch_add(1,Ordering::SeqCst)
    ));
    fs::create_dir(&p).unwrap();
    (p.clone(),Dir::open_ambient_dir(&p,ambient_authority()).unwrap())
}
#[test]
fn xlsx_is_a_real_zip_package_with_typed_xml_and_escaped_strings(){
    let workbook=render_xlsx("Budget & Taxes",&[
        cell(1,1,CellValue::Text("Item".into())),
        cell(1,2,CellValue::Text("Total".into())),
        cell(2,1,CellValue::Text("<Food> & rent".into())),
        cell(2,2,CellValue::Number(123.5)),
        cell(3,2,CellValue::Formula("SUM(B2:B2)".into())),
    ]).unwrap();
    let mut z=zip::ZipArchive::new(std::io::Cursor::new(workbook)).unwrap();
    let mut sheet=String::new();
    z.by_name("xl/worksheets/sheet1.xml").unwrap().read_to_string(&mut sheet).unwrap();
    assert!(sheet.contains(r#"<c r="B2" t="n"><v>123.5</v></c>"#));
    assert!(sheet.contains(r#"<c r="B3"><f>SUM(B2:B2)</f></c>"#));
    let mut strings=String::new();
    z.by_name("xl/sharedStrings.xml").unwrap().read_to_string(&mut strings).unwrap();
    assert!(strings.contains("&lt;Food&gt; &amp; rent"));
    let mut wb=String::new();
    z.by_name("xl/workbook.xml").unwrap().read_to_string(&mut wb).unwrap();
    assert!(wb.contains("Budget &amp; Taxes"));
}
#[test]
fn csv_text_does_not_execute_formula_injection(){
    let bytes=render_csv(&[
        cell(1,1,CellValue::Text("=1+1".into())),
        cell(1,2,CellValue::Text("normal \"quoted\"".into())),
        cell(2,1,CellValue::Formula("SUM(A1:A1)".into())),
    ]).unwrap();
    let s=String::from_utf8(bytes).unwrap();
    assert!(s.contains("\"'=1+1\""));
    assert!(s.contains("\"normal \"\"quoted\"\"\""));
    assert!(s.contains("\"'=SUM(A1:A1)\""));
}
#[test]
fn bad_ranges_duplicate_cells_unsafe_sheet_names_and_formulas_fail(){
    assert_eq!(render_xlsx("bad/name",&[]).unwrap_err(),SpreadsheetWriteError::InvalidSheetName);
    assert_eq!(render_xlsx("Sheet",&[
        cell(1,1,CellValue::Text("x".into())),
        cell(1,1,CellValue::Text("y".into())),
    ]).unwrap_err(),SpreadsheetWriteError::DuplicateCell);
    assert_eq!(render_xlsx("Sheet",&[
        cell(0,1,CellValue::Text("x".into())),
    ]).unwrap_err(),SpreadsheetWriteError::InvalidCell);
    assert_eq!(render_xlsx("Sheet",&[
        cell(1,1,CellValue::Formula("WEBSERVICE(A1)".into())),
    ]).unwrap_err(),SpreadsheetWriteError::InvalidFormula);
    assert_eq!(render_xlsx("Sheet",&[
        cell(1,1,CellValue::Number(f64::NAN)),
    ]).unwrap_err(),SpreadsheetWriteError::InvalidNumber);
}
#[test]
fn publish_is_create_only_never_overwrites(){
    let (root,dir)=fixture();
    let cells=[cell(1,1,CellValue::Text("Hello".into()))];
    let first=create_spreadsheet_new_in_dir(
        &dir,"report.xlsx","transaction-123456789","Report",&cells
    ).unwrap();
    assert_eq!(first.target,"report.xlsx");
    let original=fs::read(root.join("report.xlsx")).unwrap();
    assert!(!original.is_empty());
    assert!(create_spreadsheet_new_in_dir(
        &dir,"report.xlsx","transaction-987654321","Report",&cells
    ).is_err());
    assert_eq!(fs::read(root.join("report.xlsx")).unwrap(),original);
    drop(dir);fs::remove_dir_all(root).unwrap();
}

#[test]
fn new_workbook_roundtrips_through_real_universal_reader(){
    let bytes=render_xlsx("Income",&[
        cell(1,1,CellValue::Text("Income & Costs".into())),
        cell(2,2,CellValue::Number(301.25)),
        cell(3,2,CellValue::Formula("SUM(B2:B2)".into())),
    ]).unwrap();
    let decoded=lu_core::native_reader_office::read_xlsx(&bytes,4096).unwrap();
    assert!(decoded.text.contains("[Sheet: Income]"));
    assert!(decoded.text.contains("A1=Income & Costs"));
    assert!(decoded.text.contains("B2=301.25"));
    assert!(decoded.text.contains("SUM(B2:B2)"));
}
