//! Deliberately malformed artifact that has a correct *transport digest*.
//! The independent DOCX/XLSX content parser MUST still reject publication.
use std::{io::{self,Write},ffi::OsStr};
use sha2::{Digest,Sha256};
use lu_core::native_app_execution::E1_VERSION_MARKER_BYTES;
fn main(){
    let mut args=std::env::args_os().skip(1);
    if args.next().as_deref()!=Some(OsStr::new("--lu-e1-artifact-one-shot")){
        std::process::exit(2)
    }
    let kind=match args.next().and_then(|a|a.into_string().ok()).as_deref(){
        Some("docx")=>"docx",
        Some("xlsx")=>"xlsx",
        _=>std::process::exit(2),
    };
    if args.next().is_some(){std::process::exit(2)}
    let bytes=b"definitely not an OOXML archive";
    let mut out=io::stdout().lock();
    let n=E1_VERSION_MARKER_BYTES.len();
    writeln!(out,"P\t0\t{n}").unwrap();
    writeln!(out,"P\t{n}\t{n}").unwrap();
    writeln!(out,"D\t{:x}",Sha256::digest(E1_VERSION_MARKER_BYTES)).unwrap();
    writeln!(out,"A\t{kind}\t{}\t{:x}",bytes.len(),Sha256::digest(bytes)).unwrap();
    out.write_all(bytes).unwrap();
    out.flush().unwrap();
}
