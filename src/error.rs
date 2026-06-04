use std::path::PathBuf;
use thiserror::Error;

/// All errors that can occur in the peep CLI.
#[derive(Debug, Error)]
pub enum PeepError {
    /// Failed to open or decode an input image.
    #[error("failed to load image {path}")]
    ImageLoad {
        path: PathBuf,
        #[source]
        source: image::ImageError,
    },

    /// The two input images have different dimensions.
    #[error(
        "dimension mismatch: design is {width_a}x{height_a} but implementation is {width_b}x{height_b}"
    )]
    DimMismatch {
        width_a: u32,
        height_a: u32,
        width_b: u32,
        height_b: u32,
    },

    /// An error propagated from the `image-compare` crate.
    #[error("comparison failed: {0}")]
    Compare(#[from] image_compare::CompareError),

    /// Failed to write the diff image to the output path.
    #[error("failed to write diff image to {path}")]
    DiffWrite {
        path: std::path::PathBuf,
        #[source]
        source: image::ImageError,
    },
}
