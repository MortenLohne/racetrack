use std::convert::TryInto;

// This is an implementation of GSPRT under a pentanominal model.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PentanomialResult {
    pub ww: usize,
    pub wd: usize,
    pub wl: usize,
    pub dd: usize,
    pub dl: usize,
    pub ll: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SprtParameters {
    lower_bound: f64,
    upper_bound: f64,
    elo0: f64,
    elo1: f64,
}

impl SprtParameters {
    pub fn new(elo0: f64, elo1: f64, alpha: f64, beta: f64) -> SprtParameters {
        let lower_bound = (beta / (1.0 - alpha)).ln();
        let upper_bound = ((1.0 - beta) / alpha).ln();
        SprtParameters{ lower_bound, upper_bound, elo0, elo1 }
    }
}

impl PentanomialResult {
    pub fn to_pdf(self: PentanomialResult) -> (f64, [f64; 5]) {
        let regularize = 1e-5;
        let penta = [
            self.ll as f64 + regularize,
            self.dl as f64 + regularize,
            self.dd as f64 + self.wl as f64 + regularize,
            self.wd as f64 + regularize,
            self.ww as f64 + regularize,
        ];
        let n: f64 = penta.iter().sum();
        (n, penta.iter().map(|x| x / n).collect::<Vec<_>>().try_into().unwrap())
    }

    pub fn to_mean_and_variance(self: PentanomialResult) -> (f64, f64, f64) {
        let scores = [0.0, 0.25, 0.5, 0.75, 1.0];
        let (n, pdf) = self.to_pdf();
        let mean: f64 = pdf.iter().zip(scores).map(|(p, s)| p * s).sum();
        let variance: f64 = pdf.iter().zip(scores).map(|(p, s)| p * (s - mean).powf(2.0)).sum();
        (n, mean, variance)
    }
}
