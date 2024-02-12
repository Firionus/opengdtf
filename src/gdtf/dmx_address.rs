use std::{num::ParseIntError, str::FromStr};

use duplicate::duplicate_item;
use serde_with::{DeserializeFromStr, SerializeDisplay};

/// GDTF DMXAddress
///
/// The standard specifies either a signed or unsigned integer with an "absolute
/// DMX address" or a string format with "Universe.Address"
///
/// I'll assume that we start with address 1 in universe 1 and continue like
/// this (internal representation in braces):  
/// 1 (0) => "1.1"  
/// 2 (1) => "1.2"  
/// ...  
/// 512 (511) => "1.512"  
/// 513 (512) => "2.1"  
/// ...  
/// 1024 (1023) => "2.512"  
/// 1025 (1024) => "3.1"  
/// ...  
/// u32::MAX (u32::MAX - 1) => "8388608.511" (highest allowed value)
///
/// Care is taken to not leak the internal representation. External values are
/// available through method `get`.
///
/// Zero or negative values are somehow allowed in the standard, but even the
/// GDTF Builder refuses them. I'll take that as "we shouldn't allow that" and
/// will use NonZeroU32.
#[derive(
    Debug, SerializeDisplay, DeserializeFromStr, Default, PartialEq, Eq, derive_more::Display,
)]
#[display(fmt = "{}", "self.get()")]
pub struct DmxAddress(u32);

impl DmxAddress {
    fn get(&self) -> u32 {
        self.0 + 1
    }
}

#[allow(clippy::unnecessary_cast)]
#[duplicate_item(integer; [u16];[i16];[u32];[i32])]
impl TryFrom<integer> for DmxAddress {
    type Error = DmxAddressError;

    fn try_from(value: integer) -> Result<Self, Self::Error> {
        if value < 1 {
            Err(DmxAddressError::TooSmall(value as i32))
        } else {
            Ok(Self((value as u32) - 1))
        }
    }
}

impl FromStr for DmxAddress {
    type Err = DmxAddressError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut it = s.split('.');
        let first = it.next();
        let second = it.next();
        match (first, second) {
            (None, _) => Err(DmxAddressError::Unexpected),
            (Some(absolute_address), None) => absolute_address.parse::<u32>()?.try_into(),
            (Some(universe), Some(dmx_address)) => {
                let u = match universe.parse::<u32>()? {
                    u @ 1..=8388608 => u - 1,
                    u => Err(DmxAddressError::InvalidUniverse(u))?,
                };
                let a = match dmx_address.parse::<u32>()? {
                    a @ 1..=512 => a - 1,
                    a => Err(DmxAddressError::InvalidDmxAddress(a))?,
                };
                let internal = (u << 9) + a;
                match internal {
                    u32::MAX => Err(DmxAddressError::TooBig(internal)),
                    _ => Ok(Self(internal)),
                }
            }
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum DmxAddressError {
    #[error("absolute DMXAddress value {0} is smaller than 1")]
    TooSmall(i32),
    #[error("absolute DMXAddress value {0} is bigger than 2^32-2 = 4294967294")]
    TooBig(u32),
    #[error("parsing error: {0}")]
    ParseIntError(#[from] ParseIntError),
    #[error("invalid universe value {0}, only 1 to 8,388,608 is supported")]
    InvalidUniverse(u32),
    #[error("invalid DMX address {0}, only 1 to 512 is valid")]
    InvalidDmxAddress(u32),
    #[error("Unexpected DMXAddress state, this is a bug in opengdtf")]
    Unexpected,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parsing() -> Result<(), DmxAddressError> {
        assert_eq!("1".parse::<DmxAddress>()?, 1.try_into()?);
        assert_eq!("512".parse::<DmxAddress>()?, 512.try_into()?);
        assert_eq!("513".parse::<DmxAddress>()?, 513.try_into()?);
        assert_eq!("1023".parse::<DmxAddress>()?, 1023.try_into()?);
        assert_eq!("1024".parse::<DmxAddress>()?, 1024.try_into()?);
        assert_eq!("1025".parse::<DmxAddress>()?, 1025.try_into()?);

        assert_eq!("1.1".parse::<DmxAddress>()?, 1.try_into()?);
        assert_eq!("1.512".parse::<DmxAddress>()?, 512.try_into()?);
        assert_eq!("2.1".parse::<DmxAddress>()?, 513.try_into()?);
        assert_eq!("2.511".parse::<DmxAddress>()?, 1023.try_into()?);
        assert_eq!("2.512".parse::<DmxAddress>()?, 1024.try_into()?);
        assert_eq!("3.1".parse::<DmxAddress>()?, 1025.try_into()?);
        assert_eq!("8388608.511".parse::<DmxAddress>()?, u32::MAX.try_into()?);

        assert!("8388608.512".parse::<DmxAddress>().is_err());
        assert!("0.1".parse::<DmxAddress>().is_err());
        assert!("1.0".parse::<DmxAddress>().is_err());
        assert!("-1.0".parse::<DmxAddress>().is_err());
        assert!("1.-1".parse::<DmxAddress>().is_err());
        assert!("?".parse::<DmxAddress>().is_err());
        assert!("?.1".parse::<DmxAddress>().is_err());
        assert!("?".parse::<DmxAddress>().is_err());
        assert!("1.!".parse::<DmxAddress>().is_err());

        Ok(())
    }

    #[test]
    fn assert_default() {
        assert_eq!(DmxAddress::default(), 1.try_into().unwrap())
    }
}
