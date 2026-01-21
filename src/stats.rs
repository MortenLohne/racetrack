// 97.5th percentile point of the normal distribution.
// This is used in computing 95% confidence intervals.
const NORM_PPF_0_975: f64 = 1.959963984540054;

#[derive(Copy, Clone, Debug, Default)]
pub struct TrinomialResult {
    pub w: u64,
    pub d: u64,
    pub l: u64,
}

#[derive(Copy, Clone, Debug, Default)]
pub struct PentanomialResult {
    pub ll: u64,
    pub dl: u64,
    pub dd: u64,
    pub wl: u64,
    pub wd: u64,
    pub ww: u64,
}

pub trait ResultExt {
    fn scores_map() -> Vec<f64>;

    fn to_vec(&self) -> Vec<u64>;

    fn count(&self) -> u64 {
        self.to_vec().iter().sum()
    }

    fn probability_distribution(&self) -> Vec<f64> {
        let v = self.to_vec();
        let n = self.count() as f64;
        let zeros = v.iter().filter(|&x| *x == 0).count();
        let regularisation = if zeros > 0 { 0.001 / zeros as f64 } else { 0.0 };
        v.iter()
            .map(|&x| (x as f64 + regularisation) / (n + regularisation * v.len() as f64))
            .collect()
    }

    fn score(&self) -> f64 {
        let pdf = self.probability_distribution();
        pdf.iter().zip(Self::scores_map()).map(|(p, s)| p * s).sum()
    }

    fn variance(&self) -> f64 {
        let pdf = self.probability_distribution();
        let mean = self.score();
        let variance: f64 = pdf
            .iter()
            .zip(Self::scores_map())
            .map(|(p, s)| p * (s - mean).powf(2.0))
            .sum();
        variance
    }

    // 95% confidence interval for score
    fn score_confidence_interval(&self) -> (f64, f64, f64) {
        let count = self.count() as f64;
        let score = self.score();
        let variance = self.variance();
        let per_count_variance = variance / count;
        let score_lower = score - NORM_PPF_0_975 * per_count_variance.sqrt();
        let score_upper = score + NORM_PPF_0_975 * per_count_variance.sqrt();

        (score_lower, score, score_upper)
    }

    // 95% confidence interval for Elo
    fn logistic_elo(&self) -> (f64, f64, f64) {
        let (score_lower, score, score_upper) = self.score_confidence_interval();

        let elo_lower = logistic_elo(score_lower);
        let elo = logistic_elo(score);
        let elo_upper = logistic_elo(score_upper);

        (elo_lower, elo, elo_upper)
    }

    // 95% confidence interval for nElo
    fn normalized_elo(&self) -> (f64, f64, f64) {
        let variance = self.variance();
        let (score_lower, score, score_upper) = self.score_confidence_interval();

        let elo_lower = normalized_elo(score_lower, variance);
        let elo = normalized_elo(score, variance);
        let elo_upper = normalized_elo(score_upper, variance);

        (elo_lower, elo, elo_upper)
    }
}

impl ResultExt for TrinomialResult {
    fn scores_map() -> Vec<f64> {
        vec![0.0, 0.5, 1.0]
    }

    fn to_vec(&self) -> Vec<u64> {
        vec![self.l, self.d, self.w]
    }
}

impl ResultExt for PentanomialResult {
    fn scores_map() -> Vec<f64> {
        vec![0.0, 0.25, 0.5, 0.75, 1.0]
    }

    fn to_vec(&self) -> Vec<u64> {
        vec![self.ll, self.dl, self.dd + self.wl, self.wd, self.ww]
    }
}

impl std::fmt::Display for TrinomialResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "+{}-{}={}", self.w, self.l, self.d)
    }
}

impl std::fmt::Display for PentanomialResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{}, {}, {}, {}, {}]",
            self.ll,
            self.dl,
            self.dd + self.wl,
            self.wd,
            self.ww
        )
    }
}

pub fn elo_to_string(p: f64) -> String {
    if p.is_infinite() && p.is_sign_negative() {
        "-INF".to_string()
    } else if p.is_infinite() && p.is_sign_positive() {
        "+INF".to_string()
    } else if p.is_nan() {
        "N/A".to_string()
    } else {
        format!("{:+.2}", p)
    }
}

fn logistic_elo(score: f64) -> f64 {
    let score = score.clamp(1e-6, 1.0 - 1e-6);
    -400.0 * (1.0 / score - 1.0).log10()
}

// References:
// - Michel Van den Bergh. Normalized Elo, https://cantate.be/Fishtest/normalized_elo.pdf
// - Michel Van den Bergh. Comments On Normalized Elo, https://cantate.be/Fishtest/normalized_elo_practical.pdf
fn normalized_elo(score: f64, variance: f64) -> f64 {
    (score - 0.5) / (2.0 * variance).sqrt() * (800.0 / f64::ln(10.0))
}
