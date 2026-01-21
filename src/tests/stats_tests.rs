use crate::stats::{PentanomialResult, ResultExt, TrinomialResult};

#[test]
fn penta_logistic_elo() {
    let examples = [
        (485, 1923, 2942, 1937, 594, (1.21, 5.11, 9.02)),
        (261, 739, 2683, 737, 253, (-5.00, -0.67, 3.66)),
        (63, 252, 385, 250, 74, (-7.38, 3.39, 14.17)),
        (527, 1007, 1932, 933, 511, (-9.16, -3.75, 1.66)),
        (175, 305, 694, 291, 157, (-14.57, -5.36, 3.85)),
    ];
    for (ll, dl, wl, wd, ww, (expected_lower, expected_mean, expected_upper)) in examples {
        let penta = PentanomialResult {
            ll,
            dl,
            wl,
            wd,
            ww,
            dd: 0,
        };
        let (lower, mean, upper) = penta.logistic_elo();
        assert!(lower.is_finite());
        assert!(mean.is_finite());
        assert!(upper.is_finite());
        assert!(f64::abs(lower - expected_lower) <= 0.01);
        assert!(f64::abs(mean - expected_mean) <= 0.01);
        assert!(f64::abs(upper - expected_upper) <= 0.01);
    }
}

#[test]
fn penta_normalized_elo() {
    let examples = [
        (485, 1923, 2942, 1937, 594, (1.68, 7.10, 12.53)),
        (261, 739, 2683, 737, 253, (-8.13, -1.09, 5.96)),
        (63, 252, 385, 250, 74, (-10.31, 4.74, 19.79)),
        (527, 1007, 1932, 933, 511, (-11.63, -4.76, 2.11)),
        (175, 305, 694, 291, 157, (-18.91, -6.96, 5.00)),
    ];
    for (ll, dl, wl, wd, ww, (expected_lower, expected_mean, expected_upper)) in examples {
        let penta = PentanomialResult {
            ll,
            dl,
            wl,
            wd,
            ww,
            dd: 0,
        };
        let (lower, mean, upper) = penta.normalized_elo();
        assert!(lower.is_finite());
        assert!(mean.is_finite());
        assert!(upper.is_finite());
        assert!(f64::abs(lower - expected_lower) <= 0.01);
        assert!(f64::abs(mean - expected_mean) <= 0.01);
        assert!(f64::abs(upper - expected_upper) <= 0.01);
    }
}
