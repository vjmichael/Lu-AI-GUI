//! Real P0-WIN-CHAT GGUF+MMProj visual challenges (not a mock model).
//! An actual pinned llama.cpp CPU server must be started separately.
use lu_core::{
    multimodal_http::generate_image_chat_stream,
    multimodal_saved_turn::{PinnedImage,SavedMultimodalTurn},
};
use image::{DynamicImage,ImageFormat,Rgb,RgbImage};
use sha2::{Digest,Sha256};
use std::{io::Cursor,time::Duration};

fn challenge(red:bool)->PinnedImage {
    let pixel=if red{Rgb([245,9,9])}else{Rgb([9,9,245])};
    let image=RgbImage::from_pixel(256,256,pixel);
    let mut bytes=Vec::new();
    DynamicImage::ImageRgb8(image).write_to(
        &mut Cursor::new(&mut bytes),ImageFormat::Png
    ).expect("create original in-memory PNG");
    let digest:[u8;32]=Sha256::digest(&bytes).into();
    PinnedImage{
        image_ref:if red{"img-ci-red"}else{"img-ci-blue"}.into(),
        mime_type:"image/png",sha256:digest,bytes
    }
}
#[test]
#[ignore="requires checksum-pinned real localhost GGUF and MMProj"]
fn actual_local_mmproj_distinguishes_two_independent_pixel_images(){
    let port:u16=std::env::var("LU_P0_MM_PORT")
        .expect("real native engine port required").parse().unwrap();
    let api_key=std::env::var("LU_P0_MM_API_KEY")
        .expect("real engine API token required");
    assert_eq!(api_key.len(),64);
    for (label,red) in [("red",true),("blue",false)]{
        let image=challenge(red);
        let sha=format!("{:x}",Sha256::digest(&image.bytes));
        let turn=SavedMultimodalTurn{
            prompt:"What is the dominant color of the provided image? Respond with exactly one color word only.".into(),
            images:vec![image],
        };
        let mut output=String::new();
        generate_image_chat_stream(
            port,&api_key,&turn,96,Duration::from_secs(180),||false,
            |part|{
                if output.len()+part.len()>2048 {
                    return Err("original multimodal output too large".into())
                }
                output.push_str(&part);Ok(())
            },
        ).expect("genuine GGUF+MMProj request must stream visible answer");
        println!("P0_MM_REAL image_sha256={sha} expected={label} model_text={:?}",
            output.chars().take(300).collect::<String>());
        let words:Vec<String>=output.split(|c:char|!c.is_ascii_alphabetic())
            .filter(|s|!s.is_empty()).map(str::to_ascii_lowercase).collect();
        assert!(words.iter().any(|word|word==label),
            "real model failed to recognize source image color");
        let opposite=if red{"blue"}else{"red"};
        assert!(!words.iter().any(|word|word==opposite),
            "real model confused independently generated colors");
    }
}
