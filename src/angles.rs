#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AngleMode {
    #[default]
    Degrees,
    Radians,
}
/// sin of an angle in degrees, exact for multiples of 30 (and so for 90, 180, 270).
pub fn sin_deg(x: f64) -> f64 {
    let r = x.rem_euclid(360.0);
    let (sign, a) = if r <= 90.0 {
        (1.0, r)
    } else if r <= 180.0 {
        (1.0, 180.0 - r)
    } else if r <= 270.0 {
        (-1.0, r - 180.0)
    } else {
        (-1.0, 360.0 - r)
    };
    // a is now in [0, 90]
    let v = if a == 0.0 {
        0.0
    } else if a == 30.0 {
        0.5
    } else if a == 90.0 {
        1.0
    } else {
        a.to_radians().sin()
    };
    sign * v
}

/// cos of an angle in degrees: cos(x) = sin(x + 90).
pub fn cos_deg(x: f64) -> f64 {
    sin_deg(x + 90.0)
}

/// tan of an angle in degrees. tan(90) gives +inf and tan(45) gives exactly 1.
pub fn tan_deg(x: f64) -> f64 {
    let r = x.rem_euclid(180.0);
    if r == 0.0 {
        0.0
    } else if r == 90.0 {
        f64::INFINITY
    } else if r == 45.0 {
        1.0
    } else if r == 135.0 {
        -1.0
    } else {
        r.to_radians().tan()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_exact_degree_values() {
        assert_eq!(sin_deg(0.0), 0.0);
        assert_eq!(sin_deg(30.0), 0.5);
        assert_eq!(sin_deg(90.0), 1.0);
        assert_eq!(sin_deg(180.0), 0.0);
        assert_eq!(sin_deg(270.0), -1.0);
        assert_eq!(cos_deg(90.0), 0.0);
        assert_eq!(cos_deg(60.0), 0.5);
        assert_eq!(cos_deg(180.0), -1.0);
        assert_eq!(sin_deg(-30.0), -0.5);
        assert_eq!(sin_deg(390.0), 0.5); // wraps
        assert_eq!(tan_deg(45.0), 1.0);
        assert!(tan_deg(90.0).is_infinite());
    }
}
