use std::{cmp::Ordering, fmt::Display};

use bpci::{Interval, NSuccessesSample, WilsonScore};
use rand::{Rng, SeedableRng};
use rand_distr::{Distribution, Normal};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct MatchScore {
    pub wins: u64,
    pub draws: u64,
    pub losses: u64,
}

impl MatchScore {
    pub fn score(self) -> f32 {
        let num_games = self.num_games() as f32;
        (self.wins as f32 + (self.draws as f32 / 2.0)) / num_games
    }

    pub fn num_games(self) -> u64 {
        self.wins + self.draws + self.losses
    }
}

impl Display for MatchScore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "+{}-{}={}", self.wins, self.losses, self.draws)
    }
}

pub struct WilsonDistribution {
    normal_dist: Normal<f64>,
    wilson_sampler: NSuccessesSample<u32>,
}

impl WilsonDistribution {
    pub fn new(n_games: u32, n_draws: u32) -> Self {
        let normal_dist = Normal::new(0.0, 1.0).unwrap();
        let wilson_sampler: NSuccessesSample<u32> =
            NSuccessesSample::new(n_games, n_draws).unwrap();

        WilsonDistribution {
            normal_dist,
            wilson_sampler,
        }
    }
}

impl Distribution<f64> for WilsonDistribution {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> f64 {
        let z: f64 = self.normal_dist.sample(rng);
        let interval = self.wilson_sampler.wilson_score_with_cc(z.abs());
        if z.is_sign_positive() {
            interval.upper()
        } else {
            interval.lower()
        }
    }
}

pub struct FullWinstonSimulation {
    results: Vec<f32>,
}

const NUM_SIMULATIONS: u64 = 100000;

impl FullWinstonSimulation {
    pub fn run_simulation(score: MatchScore) -> Self {
        // assert_ne!(score.num_games(), 0);
        let mut results = vec![];

        let mut rng = rand::rngs::SmallRng::seed_from_u64(0);

        let wilson_sampler_draw: WilsonDistribution =
            WilsonDistribution::new(score.num_games() as u32, score.draws as u32);
        let wilson_sampler_win: WilsonDistribution =
            WilsonDistribution::new((score.wins + score.losses) as u32, score.wins as u32);

        for _ in 0..=NUM_SIMULATIONS {
            let draw_p = wilson_sampler_draw.sample(&mut rng) as f32;
            let win_p = if (score.wins + score.losses) != 0 {
                wilson_sampler_win.sample(&mut rng) as f32
            } else {
                rng.gen()
            };

            let num_draws = draw_p * score.num_games() as f32;
            let num_wins = win_p * (score.num_games() as f32 - num_draws);

            results.push((0.5 * num_draws + num_wins) / score.num_games() as f32);
        }
        results.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));

        FullWinstonSimulation { results }
    }

    pub fn result_for_p(&self, p: f32) -> f32 {
        self.results[(p * self.results.len() as f32) as usize]
    }
}

pub fn to_elo(p: f32) -> Option<i32> {
    let elo = -400.0 * ((1.0 - p) / p).log10();
    if elo.is_finite() {
        Some(elo as i32)
    } else {
        None
    }
}
pub fn to_elo_string(p: f32) -> String {
    if p <= 0.0 {
        "-INF".to_string()
    } else if p >= 1.0 {
        "+INF".to_string()
    } else if p.is_nan() {
        "N/A".to_string()
    } else {
        format!("{:+}", to_elo(p).unwrap())
    }
}
