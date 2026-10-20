use crate::{AppError, AppResult};

pub fn size_gb_to_mib(raw: &str) -> AppResult<u64> {
    if raw.is_empty() {
        return Err(AppError::new("Size must be a positive number of GB."));
    }

    let mut parts = raw.split('.');
    let whole = parts.next().unwrap_or_default();
    let fraction = parts.next();
    if parts.next().is_some() {
        return Err(AppError::new("Size must be a positive number of GB."));
    }

    if whole.is_empty() && fraction.is_none() {
        return Err(AppError::new("Size must be a positive number of GB."));
    }

    let whole_valid = !whole.is_empty() && whole.chars().all(|ch| ch.is_ascii_digit());
    let fraction_valid = fraction
        .map(|value| !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()))
        .unwrap_or(true);

    let valid = match (whole.is_empty(), fraction) {
        (false, None) => whole_valid,
        (false, Some(_)) => whole_valid && fraction_valid,
        (true, Some(_)) => fraction_valid,
        (true, None) => false,
    };

    if !valid {
        return Err(AppError::new("Size must be a positive number of GB."));
    }

    let whole_value = if whole.is_empty() {
        0u128
    } else {
        whole
            .parse::<u128>()
            .map_err(|_| AppError::new("Size must be a positive number of GB."))?
    };
    let mut size_mib = whole_value
        .checked_mul(1024)
        .ok_or_else(|| AppError::new("Size is too large."))?;

    if let Some(fraction) = fraction {
        let denominator = 10u128
            .checked_pow(fraction.len() as u32)
            .ok_or_else(|| AppError::new("Size is too large."))?;
        let fraction_value = fraction
            .parse::<u128>()
            .map_err(|_| AppError::new("Size must be a positive number of GB."))?;
        size_mib = size_mib
            .checked_add(
                fraction_value
                    .checked_mul(1024)
                    .ok_or_else(|| AppError::new("Size is too large."))?
                    / denominator,
            )
            .ok_or_else(|| AppError::new("Size is too large."))?;
    }

    if size_mib == 0 {
        return Err(AppError::new("Size must be greater than zero."));
    }

    u64::try_from(size_mib).map_err(|_| AppError::new("Size is too large."))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sizes_are_parsed_without_float_rounding() {
        for (text, expected) in [
            ("1", 1024),
            ("1.5", 1536),
            (".5", 512),
            ("0.03125", 32),
            ("0.015625", 16),
        ] {
            assert_eq!(size_gb_to_mib(text).unwrap(), expected);
        }
        for text in [
            "",
            "0",
            "-1",
            "1e3",
            "NaN",
            "inf",
            "1.2.3",
            "1.",
            ".",
            "0.000001",
            "18446744073709551616",
            "0.9999999999999999999999999999999999999999",
        ] {
            assert!(size_gb_to_mib(text).is_err(), "accepted {text}");
        }
    }
}
