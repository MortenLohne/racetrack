use crate::simulation::{FullWinstonSimulation, MatchScore};

#[test]
fn single_win_test() {
    let score = MatchScore {
        wins: 1,
        draws: 0,
        losses: 0,
    };
    let simulation_result = FullWinstonSimulation::run_simulation(score);
    let lower_bound = simulation_result.result_for_p(0.025);
    // If you run 40 1-game tournaments between engines with 10% win chance for the weaker engine,
    // 4 tournaments will result in a win for the weaker engines
    assert!(
        lower_bound > 0.01 && lower_bound < 0.1,
        "Lower bound of true winning probability was {:.1}% after 1 winning game",
        100.0 * lower_bound
    );
}

#[test]
fn ten_wins_test() {
    let score = MatchScore {
        wins: 10,
        draws: 0,
        losses: 0,
    };
    let simulation_result = FullWinstonSimulation::run_simulation(score);
    let lower_bound = simulation_result.result_for_p(0.025);
    // The binomial probability for 10 trials with 10 successes with a 69% probability, is roughly 2.5%
    assert!(
        lower_bound > 0.62 && lower_bound < 0.76,
        "Lower bound of true winning probability was {:.1}% after 10 winning games",
        100.0 * lower_bound
    );
}

#[test]
fn expected_score_near_the_center_of_the_distribution_test() {
    for wins in 0..10 {
        for draws in 0..10 {
            for losses in 0..10 {
                if draws == 0 && (wins == 0 || losses == 0) {
                    continue;
                }
                let score = MatchScore {
                    wins,
                    draws,
                    losses,
                };
                let simulation_result = FullWinstonSimulation::run_simulation(score);
                let expected_score = score.score();

                let low_bound = simulation_result.result_for_p(0.2);
                let high_bound = simulation_result.result_for_p(0.8);

                assert!(
                    low_bound <= expected_score,
                    "Score {:?} had expected score {:.1}%, 20th percentile {:.1}%, 80th percentile {:.1}%",
                    score,
                    expected_score * 100.0,
                    low_bound * 100.0,
                    high_bound * 100.0
                );
                assert!(high_bound >= expected_score);
            }
        }
    }
}
