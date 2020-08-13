use board_game_traits::board::Board as BoardTrait;
use std::collections::HashSet;
use taik::board::{Board, Move, Role, Square, BOARD_SIZE};
use taik::mcts;
use taik::mcts::MctsSetting;
use rayon::prelude::*;

type Opening = Vec<Move>;

fn evaluate_opening(opening: Opening) -> (Move, f32) {
    let mut board = Board::default();
    for mv in opening {
        board.do_move(mv);
    }
    mcts::mcts(board, 100_000)
}

pub fn print_opening_evals(openings: Vec<Opening>) {
    let results: Vec<_> = openings.into_par_iter()
        .map(|opening| (opening.clone(), evaluate_opening(opening)))
        .collect();
    for (opening, (best_move, score)) in results {
        let opening_move_strings = opening.iter().map(|mv| mv.to_string()).collect::<Vec<_>>();
        println!("{}; wp {:.3}; bm {}", opening_move_strings.join(" "), score, best_move);
    }
}

#[allow(unused)]
pub fn all_flatstone_n_ply_openings(n: usize) -> Vec<Opening> {
    if n == 0 {
        vec![vec![]]
    } else {
        let lower_openings = all_flatstone_n_ply_openings(n - 1);

        let mut positions: HashSet<Board> = HashSet::new();
        let mut openings = Vec::new();

        for lower_opening in lower_openings {
            let mut board = Board::start_board();
            for mv in lower_opening.iter() {
                board.do_move(mv.clone());
            }

            let mut moves = vec![];
            board.generate_moves(&mut moves);
            moves.retain(|mv| matches!(mv, Move::Place(Role::Flat, _)));
            moves.sort_by(|move1, move2| match (move1, move2) {
                (Move::Place(_, sq1), Move::Place(_, sq2)) => sq1
                    .rank()
                    .cmp(&sq2.rank())
                    .reverse()
                    .then(sq1.file().cmp(&sq2.file())),
                _ => unreachable!(),
            });
            for mv in moves {
                let reverse_move = board.do_move(mv.clone());
                if rotations_and_symmetries(&board)
                    .iter()
                    .all(|symmetry| !positions.contains(&symmetry))
                {
                    positions.insert(board.clone());
                    let mut opening = lower_opening.clone();
                    opening.push(mv);
                    openings.push(opening);
                }
                board.reverse_move(reverse_move);
            }
        }
        openings
    }
}

fn generate_openings(openings: &[Vec<Move>]) -> Vec<Vec<Move>> {
    let mut good_openings: Vec<_> = vec![];
    for opening in openings.iter() {
        let mut position = Board::start_board();
        for mv in opening.iter() {
            position.do_move(mv.clone());
        }

        let mut tree = mcts::Tree::new_root();
        let mut simple_moves = vec![];
        let mut moves = vec![];

        let settings = MctsSetting::default();

        for _ in 0..20_000_000 {
            tree.select(
                &mut position.clone(),
                &settings,
                &mut simple_moves,
                &mut moves,
            );
        }
        println!("Analysis for opening {:?}", opening);
        tree.print_info(&MctsSetting::default());

        let alternative_moves: Vec<_> = tree
            .children
            .iter()
            .map(|(child, mv)| (mv.clone(), child.visits, child.mean_action_value))
            .filter(|(mv, visits, _)| *visits > 50_000 && !matches!(mv, Move::Place(Role::Cap, _)))
            .collect();

        if alternative_moves.len() > 1 {
            for (mv, _, _) in alternative_moves.iter() {
                let mut good_opening = opening.clone();
                good_opening.push(mv.clone());
                println!("Added opening {:?}", good_opening);
                good_openings.push(good_opening)
            }
        } else {
            println!("Added opening {:?}", opening);
            good_openings.push(opening.clone());
        }
    }

    print!("[");
    for opening in good_openings.iter() {
        if opening.len() == 2 {
            print!("[\"{}\", \"{}\"], ", opening[0], opening[1]);
        } else {
            print!(
                "[\"{}\", \"{}\", \"{}\"], ",
                opening[0], opening[1], opening[2]
            );
        }
    }
    println!("]");

    good_openings
}

pub fn flip_board_y(board: &Board) -> Board {
    let mut new_board = board.clone();
    for x in 0..BOARD_SIZE as u8 {
        for y in 0..BOARD_SIZE as u8 {
            new_board[Square(y * BOARD_SIZE as u8 + x)] =
                board[Square((BOARD_SIZE as u8 - y - 1) * BOARD_SIZE as u8 + x)].clone();
        }
    }
    new_board.update_group_connectedness();
    new_board
}

pub fn flip_board_x(board: &Board) -> Board {
    let mut new_board = board.clone();
    for x in 0..BOARD_SIZE as u8 {
        for y in 0..BOARD_SIZE as u8 {
            new_board[Square(y * BOARD_SIZE as u8 + x)] =
                board[Square(y * BOARD_SIZE as u8 + (BOARD_SIZE as u8 - x - 1))].clone();
        }
    }
    new_board.update_group_connectedness();
    new_board
}

pub fn rotate_board(board: &Board) -> Board {
    let mut new_board = board.clone();
    for x in 0..BOARD_SIZE as u8 {
        for y in 0..BOARD_SIZE as u8 {
            let new_x = y;
            let new_y = BOARD_SIZE as u8 - x - 1;
            new_board[Square(y * BOARD_SIZE as u8 + x)] =
                board[Square(new_y * BOARD_SIZE as u8 + new_x)].clone();
        }
    }
    new_board.update_group_connectedness();
    new_board
}

pub fn rotations_and_symmetries(board: &Board) -> Vec<Board> {
    vec![
        flip_board_x(&board),
        flip_board_y(&board),
        rotate_board(&board),
        rotate_board(&rotate_board(&board)),
        rotate_board(&rotate_board(&rotate_board(&board))),
        rotate_board(&flip_board_x(&board)),
        rotate_board(&flip_board_y(&board)),
    ]
}

pub(crate) const OPENING_MOVE_TEXTS: [&[&str]; 106] = [
    &["a5", "b5"],
    &["a5", "c5"],
    &["a5", "d5"],
    &["a5", "e5"],
    &["a5", "b4"],
    &["a5", "c4"],
    &["a5", "d4"],
    &["a5", "e4"],
    &["a5", "c3", "c4"],
    &["a5", "c3", "b3"],
    &["a5", "c3", "d3"],
    &["a5", "c3", "c2"],
    &["a5", "d3"],
    &["a5", "e3"],
    &["a5", "d2"],
    &["a5", "e2"],
    &["a5", "e1"],
    &["b5", "a5"],
    &["b5", "c5"],
    &["b5", "d5", "d4"],
    &["b5", "d5", "d3"],
    &["b5", "d5", "d2"],
    &["b5", "e5"],
    &["b5", "a4"],
    &["b5", "b4"],
    &["b5", "c4"],
    &["b5", "d4", "b4"],
    &["b5", "d4", "c4"],
    &["b5", "d4", "d3"],
    &["b5", "e4", "b4"],
    &["b5", "e4", "c4"],
    &["b5", "a3"],
    &["b5", "b3"],
    &["b5", "c3", "c4"],
    &["b5", "c3", "b3"],
    &["b5", "c3", "c2"],
    &["b5", "d3"],
    &["b5", "e3"],
    &["b5", "a2"],
    &["b5", "b2"],
    &["b5", "c2"],
    &["b5", "d2", "d3"],
    &["b5", "d2", "b2"],
    &["b5", "d2", "c2"],
    &["b5", "e2", "b2"],
    &["b5", "e2", "c2"],
    &["b5", "a1"],
    &["b5", "b1"],
    &["b5", "c1"],
    &["b5", "d1", "d4"],
    &["b5", "d1", "d3"],
    &["b5", "d1", "d2"],
    &["b5", "e1"],
    &["c5", "a5"],
    &["c5", "b5"],
    &["c5", "a4"],
    &["c5", "b4"],
    &["c5", "c4"],
    &["c5", "a3"],
    &["c5", "b3"],
    &["c5", "c3", "b3"],
    &["c5", "c3", "d3"],
    &["c5", "a2"],
    &["c5", "b2"],
    &["c5", "c2"],
    &["c5", "a1"],
    &["c5", "b1"],
    &["c5", "c1"],
    &["b4", "a5"],
    &["b4", "b5"],
    &["b4", "c5"],
    &["b4", "d5"],
    &["b4", "e5"],
    &["b4", "c4"],
    &["b4", "d4"],
    &["b4", "e4"],
    &["b4", "c3", "c4"],
    &["b4", "c3", "b3"],
    &["b4", "d3"],
    &["b4", "e3"],
    &["b4", "d2"],
    &["b4", "e2"],
    &["b4", "e1"],
    &["c4", "a5"],
    &["c4", "b5"],
    &["c4", "c5"],
    &["c4", "a4"],
    &["c4", "b4"],
    &["c4", "a3"],
    &["c4", "b3"],
    &["c4", "c3", "b3"],
    &["c4", "c3", "d3"],
    &["c4", "c3", "b2"],
    &["c4", "c3", "c2"],
    &["c4", "c3", "d2"],
    &["c4", "a2"],
    &["c4", "b2"],
    &["c4", "c2"],
    &["c4", "a1"],
    &["c4", "b1"],
    &["c4", "c1"],
    &["c3", "a5"],
    &["c3", "b5"],
    &["c3", "c5"],
    &["c3", "b4"],
    &["c3", "c4"],
];
