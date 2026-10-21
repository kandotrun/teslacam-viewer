//! Camera window states and the mapping from AVFoundation errors to them.
//!
//! Pure logic: errors are copied out of `NSError` into [`MediaError`] first, so the mapping is testable
//! without media.

/// Why a clip file cannot be shown (the terminal camera window states of DESIGN.md "Camera window").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileFault {
    /// The file cannot be read (removed drive, permissions, I/O, anything unclassified):
    /// `映像を読み込めません`.
    Unavailable,
    /// The file is damaged or cannot be decoded: `映像が破損しています`.
    Corrupt,
    /// The container or codec is not supported: `未対応の映像形式です`.
    Unsupported,
}

/// The state a camera window shows at one point of the event timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CameraStatus {
    /// The clip is being opened: `読み込み中`.
    Loading,
    /// Frames can be presented.
    Ready,
    /// No clip covers this time for this camera: `記録がありません`.
    Missing,
    /// See [`FileFault::Unavailable`].
    Unavailable,
    /// See [`FileFault::Corrupt`].
    Corrupt,
    /// See [`FileFault::Unsupported`].
    Unsupported,
}

impl From<FileFault> for CameraStatus {
    fn from(fault: FileFault) -> Self {
        match fault {
            FileFault::Unavailable => CameraStatus::Unavailable,
            FileFault::Corrupt => CameraStatus::Corrupt,
            FileFault::Unsupported => CameraStatus::Unsupported,
        }
    }
}

impl CameraStatus {
    /// True for the states that will not change without a new file (`missing` and the faults).
    pub fn is_terminal(self) -> bool {
        !matches!(self, CameraStatus::Loading | CameraStatus::Ready)
    }
}

/// A plain-data copy of an `NSError` and its underlying error chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaError {
    /// `NSError.domain`, e.g. `AVFoundationErrorDomain`.
    pub domain: String,
    /// `NSError.code`.
    pub code: i64,
    /// `NSError.localizedDescription` (for logs and reports; never shown verbatim in the UI).
    pub description: String,
    /// `NSUnderlyingErrorKey`, when present.
    pub underlying: Option<Box<MediaError>>,
}

impl MediaError {
    /// An error without an underlying error.
    pub fn new(domain: impl Into<String>, code: i64, description: impl Into<String>) -> Self {
        MediaError {
            domain: domain.into(),
            code,
            description: description.into(),
            underlying: None,
        }
    }

    /// Adds an underlying error.
    pub fn with_underlying(mut self, underlying: MediaError) -> Self {
        self.underlying = Some(Box::new(underlying));
        self
    }

    /// The error and its underlying errors, outermost first.
    pub fn chain(&self) -> impl Iterator<Item = &MediaError> {
        std::iter::successors(Some(self), |error| error.underlying.as_deref())
    }
}

impl std::fmt::Display for MediaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}: {}", self.domain, self.code, self.description)?;
        if let Some(underlying) = &self.underlying {
            write!(f, " (underlying: {underlying})")?;
        }
        Ok(())
    }
}

impl std::error::Error for MediaError {}

/// `AVFoundationErrorDomain`.
pub const AV_FOUNDATION_DOMAIN: &str = "AVFoundationErrorDomain";
/// `NSOSStatusErrorDomain`.
pub const OS_STATUS_DOMAIN: &str = "NSOSStatusErrorDomain";

/// AVFoundation codes that mean the media data itself is damaged.
const AV_CORRUPT: [i64; 3] = [
    -11821, // AVErrorDecodeFailed
    -11855, // AVErrorUndecodableMediaData
    -11849, // AVErrorFailedToLoadMediaData (sample data missing or unreadable inside a readable file)
];

/// AVFoundation codes that mean the container or codec is not supported.
const AV_UNSUPPORTED: [i64; 5] = [
    -11828, // AVErrorFileFormatNotRecognized
    -11829, // AVErrorFileFailedToParse
    -11853, // AVErrorFailedToParse
    -11833, // AVErrorDecoderNotFound
    -11838, // AVErrorOperationNotSupportedForAsset
];

/// OSStatus codes (VideoToolbox / media toolbox) that mean damaged media data.
const OS_STATUS_CORRUPT: [i64; 4] = [
    -12909, // kVTVideoDecoderBadDataErr
    -12911, // kVTVideoDecoderMalfunctionErr
    -12848, // the movie reader found an inconsistent or truncated sample table
    -8969,  // codecBadDataErr
];

/// OSStatus codes that mean an unknown container or codec.
const OS_STATUS_UNSUPPORTED: [i64; 3] = [
    -12847, // the format reader does not recognise the container
    -12906, // kVTCouldNotFindVideoDecoderErr
    -12910, // kVTVideoDecoderUnsupportedDataFormatErr
];

fn classify_one(error: &MediaError) -> Option<FileFault> {
    let code = error.code;
    match error.domain.as_str() {
        AV_FOUNDATION_DOMAIN if AV_CORRUPT.contains(&code) => Some(FileFault::Corrupt),
        AV_FOUNDATION_DOMAIN if AV_UNSUPPORTED.contains(&code) => Some(FileFault::Unsupported),
        OS_STATUS_DOMAIN if OS_STATUS_CORRUPT.contains(&code) => Some(FileFault::Corrupt),
        OS_STATUS_DOMAIN if OS_STATUS_UNSUPPORTED.contains(&code) => Some(FileFault::Unsupported),
        _ => None,
    }
}

/// Maps an error to the camera window state (spec §2.10.4, Mac guidance): decode failures are
/// `corrupt`; unrecognised formats, parse failures and missing decoders are `unsupported`;
/// file-system errors and everything else are `unavailable`.
///
/// The outermost error that can be classified wins, so a generic AVFoundation error wrapping a
/// VideoToolbox decode error is `corrupt`. File-system errors anywhere in the chain win over
/// everything, because a file that vanished is not damaged.
pub fn classify_error(error: &MediaError) -> FileFault {
    if error.chain().any(is_file_system_error) {
        return FileFault::Unavailable;
    }
    error
        .chain()
        .find_map(classify_one)
        .unwrap_or(FileFault::Unavailable)
}

fn is_file_system_error(error: &MediaError) -> bool {
    match error.domain.as_str() {
        "NSPOSIXErrorDomain" => true,
        // NSFileNoSuchFileError (4), NSFileReadUnknownError (256) ... NSFileReadNoSuchFileError (260),
        // NSFileReadNoPermissionError (257) and the other file read errors (256..=264).
        "NSCocoaErrorDomain" => error.code == 4 || (256..=264).contains(&error.code),
        // NSURLErrorFileDoesNotExist, NSURLErrorNoPermissionsToReadFile, ...
        "NSURLErrorDomain" => matches!(error.code, -1100 | -1102),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn av(code: i64) -> MediaError {
        MediaError::new(AV_FOUNDATION_DOMAIN, code, "x")
    }

    #[test]
    fn decode_failures_are_corrupt() {
        assert_eq!(classify_error(&av(-11821)), FileFault::Corrupt);
        assert_eq!(classify_error(&av(-11855)), FileFault::Corrupt);
    }

    #[test]
    fn unrecognised_formats_and_parse_failures_are_unsupported() {
        for code in [-11828, -11829, -11853, -11833] {
            assert_eq!(
                classify_error(&av(code)),
                FileFault::Unsupported,
                "code {code}"
            );
        }
    }

    #[test]
    fn unknown_errors_use_the_underlying_cause() {
        let wrapped =
            av(-11800).with_underlying(MediaError::new(OS_STATUS_DOMAIN, -12909, "bad data"));
        assert_eq!(classify_error(&wrapped), FileFault::Corrupt);
        let unknown = av(-11800).with_underlying(MediaError::new(OS_STATUS_DOMAIN, -1, "?"));
        assert_eq!(classify_error(&unknown), FileFault::Unavailable);
    }

    #[test]
    fn file_system_errors_are_unavailable_even_when_wrapped() {
        let gone = av(-11829).with_underlying(MediaError::new("NSPOSIXErrorDomain", 2, "ENOENT"));
        assert_eq!(classify_error(&gone), FileFault::Unavailable);
        assert_eq!(
            classify_error(&MediaError::new("NSCocoaErrorDomain", 260, "no file")),
            FileFault::Unavailable
        );
        assert_eq!(
            classify_error(&MediaError::new("NSCocoaErrorDomain", 257, "denied")),
            FileFault::Unavailable
        );
    }

    #[test]
    fn anything_else_is_unavailable() {
        assert_eq!(classify_error(&av(-11800)), FileFault::Unavailable);
        assert_eq!(
            classify_error(&MediaError::new("Other", 1, "?")),
            FileFault::Unavailable
        );
    }

    #[test]
    fn faults_map_to_camera_states() {
        assert_eq!(
            CameraStatus::from(FileFault::Corrupt),
            CameraStatus::Corrupt
        );
        assert_eq!(
            CameraStatus::from(FileFault::Unsupported),
            CameraStatus::Unsupported
        );
        assert_eq!(
            CameraStatus::from(FileFault::Unavailable),
            CameraStatus::Unavailable
        );
        assert!(CameraStatus::Missing.is_terminal());
        assert!(!CameraStatus::Loading.is_terminal());
    }

    #[test]
    fn display_includes_the_chain() {
        let error = av(-11800).with_underlying(MediaError::new(OS_STATUS_DOMAIN, -12909, "bad"));
        assert_eq!(
            error.to_string(),
            "AVFoundationErrorDomain -11800: x (underlying: NSOSStatusErrorDomain -12909: bad)"
        );
    }
}
