//! Conservative capability routing for native image understanding.
//! Model labels, a GGUF header or nearby MMProj file are insufficient.
//! Host-verified backend admission + an actual successful image probe are
//! required before claiming a vision-enabled inference session.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum VisionAvailability{
    VerifiedNative,
    Unavailable,
    Unverified,
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct VisionEvidence {
    pub backend_advertises_images:bool,
    pub model_projector_admitted:bool,
    pub live_image_probe_passed:bool,
}
impl VisionEvidence {
    pub fn availability(self)->VisionAvailability{
        if self.backend_advertises_images && self.model_projector_admitted
            && self.live_image_probe_passed{
            VisionAvailability::VerifiedNative
        }else if !self.backend_advertises_images{
            VisionAvailability::Unavailable
        }else{
            VisionAvailability::Unverified
        }
    }
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum ImageIntent {
    ReadText,
    UnderstandImage,
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum ImageRoute{
    NativeVision,
    ReaderOcrAdapter,
    CapabilityUnavailable,
}
/// The OCR fallback is ONLY for a text-reading request. It cannot be sold
/// as a generic photograph captioning or visual reasoning capability.
pub fn choose_image_route(intent:ImageIntent,evidence:VisionEvidence,
    ocr_adapter_available:bool)->ImageRoute{
    if evidence.availability()==VisionAvailability::VerifiedNative{
        return ImageRoute::NativeVision;
    }
    if intent==ImageIntent::ReadText && ocr_adapter_available{
        return ImageRoute::ReaderOcrAdapter;
    }
    ImageRoute::CapabilityUnavailable
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn projector_alone_never_claims_vision(){
        let evidence=VisionEvidence{
            backend_advertises_images:true,
            model_projector_admitted:true,
            live_image_probe_passed:false,
        };
        assert_eq!(evidence.availability(),VisionAvailability::Unverified);
        assert_eq!(choose_image_route(ImageIntent::UnderstandImage,evidence,true),
            ImageRoute::CapabilityUnavailable);
        assert_eq!(choose_image_route(ImageIntent::ReadText,evidence,true),
            ImageRoute::ReaderOcrAdapter);
    }
    #[test]
    fn verified_native_vision_is_preferred_for_both_intents(){
        let evidence=VisionEvidence{
            backend_advertises_images:true,
            model_projector_admitted:true,
            live_image_probe_passed:true,
        };
        for intent in [ImageIntent::ReadText,ImageIntent::UnderstandImage]{
            assert_eq!(choose_image_route(intent,evidence,true),ImageRoute::NativeVision);
        }
    }
    #[test]
    fn no_vision_no_ocr_yields_explicit_unavailable(){
        let evidence=VisionEvidence{
            backend_advertises_images:false,
            model_projector_admitted:false,
            live_image_probe_passed:false,
        };
        assert_eq!(choose_image_route(ImageIntent::ReadText,evidence,false),
            ImageRoute::CapabilityUnavailable);
    }
}
