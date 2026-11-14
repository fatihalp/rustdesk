use sha2::{Digest, Sha256};
use std::io::{self, Read, Seek, SeekFrom};

const SHA256_HEX_LENGTH: usize = 64;
const UPDATE_HASH_BUFFER_SIZE: usize = 8192;

#[derive(Debug)]
pub(crate) enum Sha256VerificationError {
    InvalidExpected,
    Mismatch {
        expected_sha256: String,
        actual_sha256: String,
    },
    Io(io::Error),
}

impl From<io::Error> for Sha256VerificationError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

pub(crate) fn sha256_reader_hex<R: Read>(reader: &mut R) -> io::Result<String> {
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; UPDATE_HASH_BUFFER_SIZE];
    loop {
        let count = match reader.read(&mut buffer) {
            Ok(count) => count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub(crate) fn verify_sha256_reader<R: Read + Seek>(
    reader: &mut R,
    expected_sha256: &str,
) -> Result<(), Sha256VerificationError> {
    let expected_sha256 = expected_sha256.trim().to_ascii_lowercase();
    if expected_sha256.len() != SHA256_HEX_LENGTH
        || !expected_sha256.chars().all(|c| c.is_ascii_hexdigit())
    {
        return Err(Sha256VerificationError::InvalidExpected);
    }
    reader.seek(SeekFrom::Start(0))?;
    let actual_sha256 = match sha256_reader_hex(reader) {
        Ok(actual_sha256) => actual_sha256,
        Err(error) => {
            reader.seek(SeekFrom::Start(0))?;
            return Err(error.into());
        }
    };
    reader.seek(SeekFrom::Start(0))?;
    if actual_sha256 != expected_sha256 {
        return Err(Sha256VerificationError::Mismatch {
            expected_sha256,
            actual_sha256,
        });
    }
    Ok(())
}
