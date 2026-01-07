use crate::stats::{PentanomialResult, ResultExt};

// This is an implementation of GSPRT under a pentanomial model.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SprtParameters {
    lower_bound: f64,
    upper_bound: f64,
    elo0: f64,
    elo1: f64,
    t0: f64,
    t1: f64,
}

impl SprtParameters {
    pub fn new(elo0: f64, elo1: f64, alpha: f64, beta: f64) -> SprtParameters {
        let c_et = 800.0 / f64::ln(10.0);
        let lower_bound = f64::ln(beta / (1.0 - alpha));
        let upper_bound = f64::ln((1.0 - beta) / alpha);
        let t0 = elo0 / c_et;
        let t1 = elo1 / c_et;
        SprtParameters {
            lower_bound,
            upper_bound,
            elo0,
            elo1,
            t0,
            t1,
        }
    }

    pub fn llr_bounds(self: SprtParameters) -> (f64, f64) {
        (self.lower_bound, self.upper_bound)
    }

    pub fn elo_bounds(self: SprtParameters) -> (f64, f64) {
        (self.elo0, self.elo1)
    }

    // Approximate formula for the log-likelihood ratio for the given pentanomial result.
    // See section 4.2 of https://archive.org/details/fishtest_mathematics/normalized_elo_practical/
    // Many thanks to Michel Van den Bergh.
    pub fn llr(self: SprtParameters, penta: PentanomialResult) -> f64 {
        let n = penta.count() as f64;
        let mean = penta.score();
        let variance = penta.variance();
        let sigma = (2.0 * variance).sqrt();
        let t = (mean - 0.5) / sigma;
        let a = 1.0 + (t - self.t0).powf(2.0);
        let b = 1.0 + (t - self.t1).powf(2.0);
        n * f64::ln(a / b)
    }
}
