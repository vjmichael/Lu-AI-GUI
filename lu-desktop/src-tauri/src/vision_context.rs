//! WAVE4-VISION-01: host-only observed capability status and early image
//! admission. MMProj presence is not proof of native visual understanding.
//! The actual model/backend still validates image inference independently.
use lu_core::vision_routing::{VisionEvidence,VisionAvailability};

pub(crate) fn session_evidence(
    loaded:bool,projector_admitted:bool
)->VisionEvidence{
    VisionEvidence{
        // The current installed llama HTTP adapter supports image requests
        // with a pinned projector; it does not prove semantic image output.
        backend_advertises_images:loaded,
        model_projector_admitted:loaded&&projector_admitted,
        live_image_probe_passed:false,
    }
}
pub(crate) fn image_preflight(
    loaded:bool,projector_admitted:bool
)->Result<VisionAvailability,&'static str>{
    if !loaded{return Err("Load a local model before sending an image");}
    if !projector_admitted{
        return Err("Image input requires an admitted MMProj for the loaded model");
    }
    // Successful model loading alone is never advertised as verified vision.
    Ok(session_evidence(loaded,projector_admitted).availability())
}
pub(crate) fn availability_label(evidence:VisionEvidence)->&'static str{
    match evidence.availability(){
        VisionAvailability::VerifiedNative=>"verified",
        VisionAvailability::Unverified=>"unverified",
        VisionAvailability::Unavailable=>"unavailable",
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn rejects_image_without_model_or_admitted_projector(){
        assert!(image_preflight(false,false).is_err());
        assert!(image_preflight(true,false).is_err());
    }
    #[test]
    fn projector_never_substitutes_for_real_image_probe(){
        assert_eq!(image_preflight(true,true).unwrap(),VisionAvailability::Unverified);
        assert_eq!(availability_label(session_evidence(true,true)),"unverified");
        assert_eq!(availability_label(session_evidence(false,true)),"unavailable");
        let verified=VisionEvidence{
            backend_advertises_images:true,
            model_projector_admitted:true,
            live_image_probe_passed:true,
        };
        assert_eq!(availability_label(verified),"verified");
    }
}
