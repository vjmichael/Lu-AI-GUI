//! Pure typed image-reference admission for Lu-native inference requests.
//!
//! These values are *hints*, not rights to read files. The trusted desktop
//! host must resolve an image_ref against the active saved user turn, open
//! immutable bytes, verify the exact digest/length/type and decode the image.
//! Until that resolver is connected, the text-only bridge rejects images.
use std::collections::HashSet;

pub const MAX_IMAGES_PER_TURN: usize = 4;
pub const MAX_IMAGE_BYTES: u64 = 8 * 1024 * 1024;
pub const MAX_TURN_IMAGE_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageRef<'a> {
    pub image_ref: &'a str,
    pub mime_type: &'a str,
    pub sha256: &'a [u8],
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageRefError {
    TooManyImages,
    InvalidReference,
    DuplicateReference,
    UnsupportedMime,
    InvalidDigest,
    InvalidSize,
    ExcessiveTotal,
}

fn is_opaque_ref(reference: &str) -> bool {
    reference.len() <= 128
        && !reference.is_empty()
        && reference.as_bytes()[0].is_ascii_alphanumeric()
        && reference
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

pub fn validate_image_refs(images: &[ImageRef<'_>]) -> Result<(), ImageRefError> {
    if images.len() > MAX_IMAGES_PER_TURN {
        return Err(ImageRefError::TooManyImages);
    }
    let mut refs = HashSet::new();
    let mut total_bytes = 0u64;
    for image in images {
        if !is_opaque_ref(image.image_ref) {
            return Err(ImageRefError::InvalidReference);
        }
        if !refs.insert(image.image_ref) {
            return Err(ImageRefError::DuplicateReference);
        }
        if !matches!(image.mime_type, "image/jpeg" | "image/png") {
            return Err(ImageRefError::UnsupportedMime);
        }
        if image.sha256.len() != 32 || image.sha256.iter().all(|byte| *byte == 0) {
            return Err(ImageRefError::InvalidDigest);
        }
        if image.size_bytes == 0 || image.size_bytes > MAX_IMAGE_BYTES {
            return Err(ImageRefError::InvalidSize);
        }
        total_bytes = total_bytes
            .checked_add(image.size_bytes)
            .ok_or(ImageRefError::ExcessiveTotal)?;
        if total_bytes > MAX_TURN_IMAGE_BYTES {
            return Err(ImageRefError::ExcessiveTotal);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH: [u8; 32] = [0x4d; 32];
    fn image<'a>(reference: &'a str) -> ImageRef<'a> {
        ImageRef {
            image_ref: reference,
            mime_type: "image/png",
            sha256: &HASH,
            size_bytes: 1234,
        }
    }

    #[test]
    fn text_only_is_valid() {
        assert_eq!(validate_image_refs(&[]), Ok(()));
    }
    #[test]
    fn trusted_refs_are_valid() {
        assert_eq!(
            validate_image_refs(&[image("img-1"), image("img-2")]),
            Ok(())
        );
    }
    #[test]
    fn rejects_paths_urls_and_ref_injection() {
        for value in [
            "",
            "/tmp/secret.png",
            "C:\\images\\x.png",
            "https://site/img",
            "data:image/png;base64,...",
            "a/b",
            "a?b",
            "_abc",
            "..",
            "a..b",
        ] {
            assert_eq!(
                validate_image_refs(&[image(value)]),
                Err(ImageRefError::InvalidReference)
            );
        }
    }
    #[test]
    fn rejects_repeated_image_refs() {
        assert_eq!(
            validate_image_refs(&[image("same"), image("same")]),
            Err(ImageRefError::DuplicateReference)
        );
    }
    #[test]
    fn rejects_unsupported_mime_and_empty_digest() {
        let mut img = image("img-1");
        img.mime_type = "image/gif";
        assert_eq!(
            validate_image_refs(&[img]),
            Err(ImageRefError::UnsupportedMime)
        );
        img.mime_type = "image/jpeg";
        img.sha256 = &[0u8; 32];
        assert_eq!(
            validate_image_refs(&[img]),
            Err(ImageRefError::InvalidDigest)
        );
        img.sha256 = &[1u8; 31];
        assert_eq!(
            validate_image_refs(&[img]),
            Err(ImageRefError::InvalidDigest)
        );
    }
    #[test]
    fn rejects_zero_and_oversized_image() {
        let mut img = image("img-1");
        img.size_bytes = 0;
        assert_eq!(validate_image_refs(&[img]), Err(ImageRefError::InvalidSize));
        img.size_bytes = MAX_IMAGE_BYTES + 1;
        assert_eq!(validate_image_refs(&[img]), Err(ImageRefError::InvalidSize));
    }
    #[test]
    fn rejects_more_than_four_images() {
        let images = (0..5).map(|_| image("item")).collect::<Vec<_>>();
        assert_eq!(
            validate_image_refs(&images),
            Err(ImageRefError::TooManyImages)
        );
    }
    #[test]
    fn rejects_total_over_limit() {
        let mut images = [image("one"), image("two"), image("three")];
        for item in &mut images {
            item.size_bytes = MAX_IMAGE_BYTES;
        }
        assert_eq!(
            validate_image_refs(&images),
            Err(ImageRefError::ExcessiveTotal)
        );
    }
    #[test]
    fn limit_boundary_is_inclusive() {
        let mut images = [image("one"), image("two")];
        for item in &mut images {
            item.size_bytes = MAX_IMAGE_BYTES;
        }
        assert_eq!(validate_image_refs(&images), Ok(()));
    }
}
