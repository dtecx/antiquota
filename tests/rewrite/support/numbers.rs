//! Compares JSON number values without rounding arbitrary-precision decimals.
use serde_json::Number;

pub fn equal(left: &Number, right: &Number) -> bool {
    if left == right {
        return true;
    }
    match (normalized(left), normalized(right)) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

fn normalized(number: &Number) -> Option<(bool, String, i128)> {
    let text = number.to_string();
    let (negative, text) = text
        .strip_prefix('-')
        .map_or((false, text.as_str()), |s| (true, s));
    let (mantissa, exponent) = text.split_once(['e', 'E']).unwrap_or((text, "0"));
    let fractional = mantissa.split_once('.').map_or(0, |(_, part)| part.len());
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let digits = digits.trim_start_matches('0');
    if digits.is_empty() {
        return Some((false, "0".into(), 0));
    }
    let trimmed = digits.trim_end_matches('0');
    let exponent = exponent
        .parse::<i128>()
        .ok()?
        .checked_sub(fractional as i128)?
        .checked_add((digits.len() - trimmed.len()) as i128)?;
    Some((negative, trimmed.into(), exponent))
}

#[cfg(test)]
mod tests {
    use super::equal;
    use serde_json::{Number, Value};
    fn number(text: &str) -> Number {
        serde_json::from_str::<Value>(text)
            .unwrap()
            .as_number()
            .unwrap()
            .clone()
    }
    #[test]
    fn equal_decimal_values_keep_sign_scale_and_exponents_exact() {
        for (left, right) in [
            ("0", "0.0"),
            ("-0.0", "0e12"),
            ("1e3", "1000.00"),
            ("-12.50e-2", "-0.125"),
            ("1e308", "10e307"),
            ("9007199254740993", "9007199254740993.000"),
        ] {
            assert!(equal(&number(left), &number(right)), "{left} != {right}");
        }
    }
    #[test]
    fn different_huge_integers_and_close_decimals_never_round_to_equal() {
        for (left, right) in [
            ("9007199254740993", "9007199254740992"),
            ("0.100000000000000000001", "0.100000000000000000002"),
            ("1e308", "1e307"),
            ("-1", "1"),
        ] {
            assert!(!equal(&number(left), &number(right)), "{left} == {right}");
        }
    }
}
