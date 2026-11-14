use cap_std::{ambient_authority,fs::Dir};
use lu_core::{
    native_pptx_create::{PresentationSlide,render_pptx,create_pptx_new,PptxCreateError},
    native_reader_office::read_pptx,
    native_office_revision::{OfficeRevisionEdit,render_office_revision},
};
use std::{fs,io::{Cursor,Read},sync::atomic::{AtomicU64,Ordering}};
use sha2::{Sha256,Digest};
use zip::ZipArchive;
static SEQ:AtomicU64=AtomicU64::new(0);
fn slides()->Vec<PresentationSlide>{
    vec![
        PresentationSlide{
            title:"Welcome & Introduction".into(),
            bullet_points:vec!["One <important> fact".into(),"Second point".into()],
        },
        PresentationSlide{
            title:"Closing".into(),
            bullet_points:vec!["Thank you!".into()],
        },
    ]
}
#[test]
fn powerpoint_package_has_valid_relationships_and_reader_roundtrip(){
    let bytes=render_pptx(&slides()).unwrap();
    let mut zip=ZipArchive::new(Cursor::new(&bytes)).unwrap();
    assert!(zip.by_name("ppt/presentation.xml").is_ok());
    assert!(zip.by_name("ppt/slides/slide1.xml").is_ok());
    assert!(zip.by_name("ppt/slides/slide2.xml").is_ok());
    let mut slide=String::new();
    zip.by_name("ppt/slides/slide1.xml").unwrap().read_to_string(&mut slide).unwrap();
    assert!(slide.contains("Welcome &amp; Introduction"));
    assert!(slide.contains("One &lt;important&gt; fact"));
    let result=read_pptx(&bytes,10000).unwrap();
    assert!(result.text.contains("Welcome & Introduction"));
    assert!(result.text.contains("Closing"));
}
#[test]
fn saved_presentation_can_be_revised_without_changing_other_slides(){
    let bytes=render_pptx(&slides()).unwrap();
    let old=format!("{:x}",Sha256::digest(&bytes));
    let revised=render_office_revision(&bytes,&old,&OfficeRevisionEdit::SlideRun{
        slide_number:1,text_index:0,expected_old:"Welcome & Introduction".into(),
        new_text:"Revised slide heading".into(),
    }).unwrap();
    let text=read_pptx(&revised.revised_bytes,10000).unwrap();
    assert!(text.text.contains("Revised slide heading"));
    assert!(text.text.contains("Closing"));
}
#[test]
fn pptx_create_does_not_overwrite_and_rejects_bad_names(){
    let root=std::env::temp_dir().join(format!("lu-pptx-{}-{}",
        std::process::id(),SEQ.fetch_add(1,Ordering::SeqCst)));
    fs::create_dir_all(&root).unwrap();
    let dir=Dir::open_ambient_dir(&root,ambient_authority()).unwrap();
    create_pptx_new(&dir,"slides.pptx","transaction-123456789",&slides()).unwrap();
    let original=fs::read(root.join("slides.pptx")).unwrap();
    assert!(create_pptx_new(&dir,"slides.pptx","transaction-987654321",
        &slides()).is_err());
    assert_eq!(fs::read(root.join("slides.pptx")).unwrap(),original);
    assert_eq!(create_pptx_new(&dir,"../escape.pptx",
        "transaction-abc123456",&slides()).unwrap_err(),
        PptxCreateError::InvalidName);
    assert_eq!(render_pptx(&[]).unwrap_err(),PptxCreateError::Empty);
    drop(dir);fs::remove_dir_all(root).unwrap();
}
