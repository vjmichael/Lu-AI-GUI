//! P06: one-way, in-memory local WGC BGRA -> admitted PNG boundary.
//! Designed for a separately OS-attested and user-approved WGC frame.
//! NO filesystem path, socket, remote URL, Tauri command or network worker.
//! This is an image-admission primitive, not a capture or authorization grant.
use base64::{engine::general_purpose::STANDARD, Engine as _};
use image::{codecs::png::PngEncoder, ColorType, ImageEncoder};
use crate::{
    multimodal_saved_turn::{admit_data_url,PinnedImage,MAX_IMAGE_BYTES},
    native_observe_egress::{DataProvenance,Destination},
};

pub const MAX_VISUAL_WIDTH:u32=1280;
pub const MAX_VISUAL_HEIGHT:u32=720;
const MAX_NATIVE_EDGE:u32=8192;
const MAX_NATIVE_PIXELS:u64=33_177_600;

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum LocalVisualError {
    MissingOsConsent,
    InvalidGeometry,
    UnboundedSource,
    PngEncode,
    RejectedImage,
    EgressDenied,
}
/// Never expose the WGC frame via a model-visible filename or a WebView data
/// URL. The containing host must bind this image to its chat and consent
/// session and enforce OBSERVE_LEARN provenance for subsequent generations.
#[derive(Debug,Clone)]
pub struct LocalObservedImage {
    pub pinned:PinnedImage,
    pub provenance:DataProvenance,
    pub source_width:u32,
    pub source_height:u32,
    pub rendered_width:u32,
    pub rendered_height:u32,
}
/// Only a native host adapter with separately verified active OS consent
/// should call this with a mapped WGC BGRA buffer. Consent is an explicit
/// parameter, not inferred from an untrusted image or model instruction.
pub fn admit_wgc_frame_locally(
    source:&[u8],width:u32,height:u32,row_pitch:u32,
    consent_active_and_os_source_attested:bool
)->Result<LocalObservedImage,LocalVisualError>{
    if !consent_active_and_os_source_attested{
        return Err(LocalVisualError::MissingOsConsent);
    }
    if width==0||height==0||width>MAX_NATIVE_EDGE||height>MAX_NATIVE_EDGE
        ||u64::from(width)*u64::from(height)>MAX_NATIVE_PIXELS{
        return Err(LocalVisualError::UnboundedSource);
    }
    let min_pitch=width.checked_mul(4).ok_or(LocalVisualError::InvalidGeometry)?;
    if row_pitch<min_pitch||row_pitch>min_pitch.saturating_add(4096){
        return Err(LocalVisualError::InvalidGeometry);
    }
    let src_len=usize::try_from(row_pitch).ok()
        .and_then(|pitch|pitch.checked_mul(height as usize))
        .ok_or(LocalVisualError::InvalidGeometry)?;
    if source.len()!=src_len{
        return Err(LocalVisualError::InvalidGeometry);
    }
    // Integer math preserves aspect ratio without floating-point
    // precision loss, and a smaller RGB image limits both RAM and PNG size.
    let (rendered_width,rendered_height)=if width<=MAX_VISUAL_WIDTH&&height<=MAX_VISUAL_HEIGHT{
        (width,height)
    }else if u64::from(width)*u64::from(MAX_VISUAL_HEIGHT)>
        u64::from(height)*u64::from(MAX_VISUAL_WIDTH){
        (MAX_VISUAL_WIDTH,
         (u64::from(height)*u64::from(MAX_VISUAL_WIDTH)/u64::from(width)).max(1) as u32)
    }else{
        ((u64::from(width)*u64::from(MAX_VISUAL_HEIGHT)/u64::from(height)).max(1) as u32,
         MAX_VISUAL_HEIGHT)
    };
    let output_len=usize::try_from(rendered_width).ok()
        .and_then(|w|w.checked_mul(rendered_height as usize))
        .and_then(|px|px.checked_mul(4))
        .ok_or(LocalVisualError::InvalidGeometry)?;
    let mut rgba=vec![0u8;output_len];
    let pitch=row_pitch as usize;
    for y in 0..rendered_height{
        let sy=(u64::from(y)*u64::from(height)/u64::from(rendered_height)) as usize;
        for x in 0..rendered_width{
            let sx=(u64::from(x)*u64::from(width)/u64::from(rendered_width)) as usize;
            let from=sy*pitch+sx*4;
            let to=((y as usize*rendered_width as usize)+(x as usize))*4;
            rgba[to]=source[from+2];
            rgba[to+1]=source[from+1];
            rgba[to+2]=source[from];
            rgba[to+3]=source[from+3];
        }
    }
    let mut encoded=Vec::new();
    let encode_result=PngEncoder::new(&mut encoded)
        .write_image(&rgba,rendered_width,rendered_height,ColorType::Rgba8.into());
    // The intermediate heap representation is never returned, logged,
    // serialized, passed to a child process, or retained after encoding.
    rgba.fill(0);
    encode_result.map_err(|_|LocalVisualError::PngEncode)?;
    if encoded.is_empty()||encoded.len()>MAX_IMAGE_BYTES{
        encoded.fill(0);
        return Err(LocalVisualError::RejectedImage);
    }
    // Reuse Lu's actual PNG parser/dimension/bytes canonical-admission
    // contract, not an image invented by a model or WebView.
    let payload=format!("data:image/png;base64,{}",STANDARD.encode(&encoded));
    encoded.fill(0);
    let pinned=admit_data_url(&payload).map_err(|_|LocalVisualError::RejectedImage)?;
    let provenance=DataProvenance::OBSERVE_LEARN;
    provenance.gate(Destination::LocalInference)
        .map_err(|_|LocalVisualError::EgressDenied)?;
    if provenance.gate(Destination::RemoteModel).is_ok()
        ||provenance.gate(Destination::NetworkSocket).is_ok(){
        return Err(LocalVisualError::EgressDenied);
    }
    Ok(LocalObservedImage{
        pinned,provenance,source_width:width,source_height:height,
        rendered_width,rendered_height,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::ImageReader;
    use std::io::Cursor;

    #[test]
    fn approved_local_frame_is_png_and_keeps_observation_egress_taint(){
        // Two BGRA pixels, then nonzero D3D pitch padding which must never
        // become image pixels or affect the canonical admitted digest.
        let a=[1u8,2,3,255,10,20,30,255,91,92,93,94];
        let mut b=a;
        b[8..12].copy_from_slice(&[9,8,7,6]);
        let first=admit_wgc_frame_locally(&a,2,1,12,true).unwrap();
        let second=admit_wgc_frame_locally(&b,2,1,12,true).unwrap();
        assert_eq!(first.pinned.image_ref,second.pinned.image_ref);
        assert_eq!(first.provenance,DataProvenance::OBSERVE_LEARN);
        assert!(first.provenance.gate(Destination::RemoteModel).is_err());
        assert!(first.provenance.gate(Destination::LocalInference).is_ok());
        assert_eq!(first.rendered_width,2);
        assert_eq!(first.rendered_height,1);
        let decoded=ImageReader::with_format(Cursor::new(&first.pinned.bytes),image::ImageFormat::Png)
            .decode().unwrap().to_rgba8();
        assert_eq!(decoded.get_pixel(0,0).0,[3,2,1,255]);
        assert_eq!(decoded.get_pixel(1,0).0,[30,20,10,255]);
    }

    #[test]
    fn inactive_consent_or_unverified_source_never_returns_pixels(){
        assert_eq!(admit_wgc_frame_locally(&[1,2,3,255],1,1,4,false).unwrap_err(),
            LocalVisualError::MissingOsConsent);
        assert!(admit_wgc_frame_locally(&[1,2,3,255],1,1,4,true).is_ok());
    }

    #[test]
    fn invalid_stride_geometry_or_excessive_images_refuse_admission(){
        assert_eq!(admit_wgc_frame_locally(&[],1,1,4,true).unwrap_err(),
            LocalVisualError::InvalidGeometry);
        assert_eq!(admit_wgc_frame_locally(&[1,2,3,255],1,1,3,true).unwrap_err(),
            LocalVisualError::InvalidGeometry);
        assert_eq!(admit_wgc_frame_locally(&[1,2,3,255],0,1,4,true).unwrap_err(),
            LocalVisualError::UnboundedSource);
        assert_eq!(admit_wgc_frame_locally(&[],8192,8192,32768,true).unwrap_err(),
            LocalVisualError::UnboundedSource);
        assert_eq!(admit_wgc_frame_locally(&[],1,1,8192,true).unwrap_err(),
            LocalVisualError::InvalidGeometry);
    }

    #[test]
    fn oversized_frame_is_aspect_preserved_and_bounded_to_720p(){
        let width=1920u32;
        let height=1080u32;
        let original=vec![255u8;(width*height*4) as usize];
        let frame=admit_wgc_frame_locally(&original,width,height,width*4,true).unwrap();
        assert_eq!((frame.rendered_width,frame.rendered_height),(1280,720));
        assert!(frame.pinned.bytes.len()<MAX_IMAGE_BYTES);
    }
}
