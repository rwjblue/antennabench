use antennabench_core::v2::AttachmentReference;

use crate::v2::sha256_hex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleAttachment {
    pub reference: AttachmentReference,
    pub bytes: Vec<u8>,
}

impl BundleAttachment {
    pub fn new(
        bytes: Vec<u8>,
        media_type: impl Into<String>,
        encoding: Option<String>,
        container: Option<String>,
        source_locator: Option<String>,
    ) -> Self {
        let reference = AttachmentReference {
            sha256: sha256_hex(&bytes),
            byte_size: u64::try_from(bytes.len()).expect("usize fits in u64"),
            media_type: media_type.into(),
            encoding,
            container,
            source_locator,
        };
        Self { reference, bytes }
    }
}
