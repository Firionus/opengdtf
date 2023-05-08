use std::str::FromStr;

use roxmltree::Node;
use thiserror::Error;

use crate::{Problem, Problems};

#[derive(Error, Debug)]
pub enum DmxValueError {
    #[error("DMXValue does not contain exactly one delimiting slash")]
    WrongSlashCount,
    #[error("empty DMXValue")]
    Empty,
    #[error("value is not a valid u32; {0}")]
    InvalidValue(Box<dyn std::error::Error>),
    #[error("byte count is not a valid u8; {0}")]
    InvalidBytes(Box<dyn std::error::Error>),
    #[error("DMXValue bigger than maximum value {0} of given bytes")]
    ValueTooBig(u32),
    #[error("DMXValues are only supported with up to 4 bytes")]
    UnsupportedByteCount,
}

pub fn bytes_max_value(bytes: u8) -> u32 {
    if bytes == 0 {
        return 0;
    }
    u32::MAX >> (32 - 8 * bytes)
}

pub fn parse_dmx(s: &str, out_bytes: u8) -> Result<u32, DmxValueError> {
    if out_bytes > 4 {
        return Err(DmxValueError::UnsupportedByteCount);
    } else if out_bytes == 0 {
        return Ok(0);
    }

    let mut parts = s.split('/');
    let in_value = parts.next().ok_or(DmxValueError::Empty)?;
    let bytes_and_mode = parts.next().ok_or(DmxValueError::WrongSlashCount)?;
    if parts.next().is_some() {
        return Err(DmxValueError::WrongSlashCount);
    }

    let in_value: u32 = in_value
        .parse()
        .map_err(|e| DmxValueError::InvalidValue(Box::new(e)))?;

    let mut in_bytes = bytes_and_mode.chars();
    let shift_mode = if bytes_and_mode.ends_with('s') {
        in_bytes.next_back();
        true
    } else {
        false
    };
    let in_bytes = in_bytes.as_str();

    let in_bytes: u8 = in_bytes
        .parse()
        .map_err(|e| DmxValueError::InvalidBytes(Box::new(e)))?;

    if in_bytes > 4 {
        return Err(DmxValueError::UnsupportedByteCount);
    }

    let maximum_value = bytes_max_value(in_bytes);
    if in_value > maximum_value {
        return Err(DmxValueError::ValueTooBig(maximum_value));
    }

    let byte_diff = (out_bytes as i8) - (in_bytes as i8);

    let out_value = if byte_diff <= 0 {
        // downconvert by truncation
        in_value >> (-8 * byte_diff)
    } else if shift_mode {
        // upconvert by shift (fill with zeros)
        in_value << (8 * byte_diff)
    } else {
        // upconvert in periodic mode
        // shift with whole input repeated
        let full_shifts = byte_diff / (in_bytes as i8);
        let partial_shifts = byte_diff % (in_bytes as i8);
        let drop_for_partial = (in_bytes as i8) - partial_shifts;
        let mut out = in_value;
        for _ in 0..full_shifts {
            out = (out << (8 * in_bytes)) | in_value;
        }
        (out << (8 * partial_shifts)) | (in_value >> (8 * drop_for_partial))
    };

    Ok(out_value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zeros() {
        // // zero
        for in_bytes in 1..=4 {
            for out_bytes in 1..=4 {
                assert_eq!(parse_dmx(&format!("0/{in_bytes}"), out_bytes).unwrap(), 0);
            }
        }
    }

    #[test]
    fn identical_in_and_out_bytes() {
        for bytes in 1..=4 {
            assert_eq!(parse_dmx(&format!("1/{bytes}"), bytes).unwrap(), 1);
            let max_val = u32::MAX >> (32 - 8 * bytes);
            assert_eq!(
                parse_dmx(&format!("{max_val}/{bytes}"), bytes).unwrap(),
                max_val
            );
        }
    }

    #[test]
    fn din_spec_cases() {
        assert_eq!(parse_dmx("255/1", 2).unwrap(), 65535);
        assert_eq!(parse_dmx("255/1s", 2).unwrap(), 65280);
    }

    #[test]
    fn downconversion() {
        assert_eq!(parse_dmx("3419130827/4", 3).unwrap(), 13355979);
        assert_eq!(parse_dmx("3419130827/4", 2).unwrap(), 52171);
        assert_eq!(parse_dmx("3419130827/4", 1).unwrap(), 203);
        // shift should have not impact
        assert_eq!(parse_dmx("3419130827/4s", 3).unwrap(), 13355979);
        assert_eq!(parse_dmx("3419130827/4s", 2).unwrap(), 52171);
        assert_eq!(parse_dmx("3419130827/4s", 1).unwrap(), 203);
    }

    #[test]
    fn periodic_upconversion() {
        assert_eq!(parse_dmx("42/1", 2).unwrap(), 10794);
        assert_eq!(parse_dmx("42/1", 3).unwrap(), 2763306);
        assert_eq!(parse_dmx("42/1", 4).unwrap(), 707406378);

        assert_eq!(parse_dmx("42423/2", 3).unwrap(), 10860453);
        assert_eq!(parse_dmx("42423/2", 4).unwrap(), 2780276151);
    }

    #[test]
    fn shifting_upconversion() {
        assert_eq!(parse_dmx("234/1s", 2).unwrap(), 59904);
        assert_eq!(parse_dmx("234/1s", 3).unwrap(), 15335424);
        assert_eq!(parse_dmx("234/1s", 4).unwrap(), 3925868544);
    }

    #[test]
    fn zero_bytes() {
        assert!(matches!(
            parse_dmx("42/0", 1),
            Err(DmxValueError::ValueTooBig(..))
        ));
        assert_eq!(parse_dmx("42/1", 0).unwrap(), 0);
        assert_eq!(parse_dmx("0/4", 0).unwrap(), 0);
    }
}
