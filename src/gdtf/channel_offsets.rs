use std::str::FromStr;

use derive_more::IntoIterator;

/// DMX address offset of a channel from most to least significant byte.
///
/// Values go from 0 to 511. Empty indicates a virtual channel. The maximum
/// number of supported bytes per channel is 4.
#[derive(Default, Debug, IntoIterator, derive_more::Deref, derive_more::DerefMut)]
pub struct ChannelOffsets(Vec<u16>);

#[derive(Debug, thiserror::Error)]
pub enum OffsetError {
    #[error("invalid Offset Format")]
    Invalid,
    #[error("DXM address offsets must be between 1 and 512")]
    OutsideRange,
}

impl FromStr for ChannelOffsets {
    type Err = OffsetError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut out = Self::default();

        if let "None" | "" = s {
            // empty string is not allowed in GDTF 1.2, but some builder files use it
            return Ok(out);
        }

        for s in s.split(',') {
            let u: u16 = s.parse().map_err(|_| OffsetError::Invalid)?;
            if (1..=512).contains(&u) {
                out.0.push(u - 1);
            } else {
                return Err(OffsetError::OutsideRange);
            }
        }

        Ok(out)
    }
}

impl FromIterator<u16> for ChannelOffsets {
    fn from_iter<I: IntoIterator<Item = u16>>(iter: I) -> Self {
        Self(Vec::from_iter(iter))
    }
}
