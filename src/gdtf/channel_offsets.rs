use std::str::FromStr;

use derive_more::IntoIterator;

/// DMX address offsets of a channel from most to least significant byte.
///
/// Values go from 1 to 512. Empty indicates a virtual channel. The maximum
/// number of supported bytes per channel is 4. Duplicates are disallowed.
#[derive(Default, Debug, IntoIterator, derive_more::Deref, PartialEq, Clone)]
pub struct ChannelOffsets(Vec<u16>);

#[derive(Debug, thiserror::Error)]
pub enum OffsetError {
    #[error("invalid Offset Format")]
    Invalid,
    #[error("DMX address offsets must be between 1 and 512 (or 0 and 511 internally)")]
    OutsideRange,
    #[error("channels cannot have more than 4 bytes, this is a limitation of the implementation")]
    UnsupportedByteCount,
    #[error("duplicate channel offsets ${0}")]
    Duplicate(u16),
}

impl TryFrom<Vec<u16>> for ChannelOffsets {
    type Error = OffsetError;

    fn try_from(vec: Vec<u16>) -> Result<Self, Self::Error> {
        if vec.len() > 4 {
            Err(OffsetError::UnsupportedByteCount)?
        }

        for (i, v) in vec.iter().enumerate() {
            if !(1..=512).contains(v) {
                Err(OffsetError::OutsideRange)?
            }
            for (j, u) in vec.iter().enumerate() {
                if v == u && i != j {
                    Err(OffsetError::Duplicate(*v))?
                }
            }
        }

        Ok(Self(vec))
    }
}

impl FromStr for ChannelOffsets {
    type Err = OffsetError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut out = Vec::<u16>::new();

        if let "None" | "" = s {
            // empty string is not allowed in GDTF 1.2, but some builder files use it
            return out.try_into();
        }

        for s in s.split(',') {
            let u: u16 = s.parse().map_err(|_| OffsetError::Invalid)?;
            out.push(u);
        }

        out.try_into()
    }
}

impl ChannelOffsets {
    /// Adds `value` to all elements of the channel offset
    pub fn add_all(mut self, value: u16) -> Result<Self, OffsetError> {
        self.0.iter_mut().for_each(|v| *v += value);
        self.0.try_into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_creation() -> Result<(), OffsetError> {
        assert_eq!(ChannelOffsets::default().0, []);
        assert_eq!(ChannelOffsets::try_from(vec![])?, ChannelOffsets::default());
        assert_eq!(
            ChannelOffsets::from_str("1,4")?,
            ChannelOffsets::try_from(vec![1, 4])?
        );
        assert_eq!(
            ChannelOffsets::try_from(vec![1, 2, 3, 4])?,
            ChannelOffsets(vec![1, 2, 3, 4])
        );
        assert!(matches!(
            ChannelOffsets::try_from(vec![513]),
            Err(OffsetError::OutsideRange)
        ));
        assert!(matches!(
            ChannelOffsets::try_from(vec![0]),
            Err(OffsetError::OutsideRange)
        ));
        assert!(matches!(
            ChannelOffsets::try_from(vec![1, 2, 3, 4, 5]),
            Err(OffsetError::UnsupportedByteCount)
        ));
        assert!(matches!(
            ChannelOffsets::try_from(vec![1, 1]),
            Err(OffsetError::Duplicate(1))
        ));
        assert!(matches!(
            ChannelOffsets::from_str("4,4"),
            Err(OffsetError::Duplicate(4))
        ));
        Ok(())
    }
}
