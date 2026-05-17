use crate::board::{BOARD_COLS, BOARD_ROWS, Terrain, terrain_at};
use crate::types::{BattleOutcome, PieceState, Rank};
use std::collections::{HashMap, HashSet, VecDeque};

pub fn move_path(
    piece: PieceState,
    to_row: usize,
    to_col: usize,
    occupied: &HashSet<(usize, usize)>,
) -> Option<Vec<(usize, usize)>> {
    if !piece.rank.can_move() {
        return None;
    }

    if is_one_step_road_move((piece.row, piece.col), (to_row, to_col)) {
        return Some(vec![(to_row, to_col)]);
    }

    if terrain_at(piece.row, piece.col) != Terrain::Railway
        || terrain_at(to_row, to_col) != Terrain::Railway
    {
        return None;
    }

    if piece.rank == Rank::Engineer {
        railway_path((piece.row, piece.col), (to_row, to_col), occupied)
    } else {
        railway_straight_path((piece.row, piece.col), (to_row, to_col), occupied)
    }
}

pub fn resolve_battle(attacker: Rank, defender: Rank) -> BattleOutcome {
    if attacker == Rank::Bomb || defender == Rank::Bomb {
        return BattleOutcome::BothRemoved;
    }

    if defender == Rank::Mine {
        return if attacker == Rank::Engineer {
            BattleOutcome::AttackerWins
        } else {
            BattleOutcome::DefenderWins
        };
    }

    match attacker.strength().cmp(&defender.strength()) {
        std::cmp::Ordering::Greater => BattleOutcome::AttackerWins,
        std::cmp::Ordering::Less => BattleOutcome::DefenderWins,
        std::cmp::Ordering::Equal => BattleOutcome::BothRemoved,
    }
}

pub fn would_lose_attack(attacker: Rank, defender: Rank) -> bool {
    if attacker == Rank::Bomb || defender == Rank::Bomb {
        return false;
    }

    if defender == Rank::Mine {
        return attacker != Rank::Engineer;
    }

    attacker.strength() < defender.strength()
}

pub fn should_explode(attacker: Rank, defender: Rank) -> bool {
    attacker == Rank::Bomb || defender == Rank::Bomb
}

fn is_one_step_road_move(from: (usize, usize), to: (usize, usize)) -> bool {
    let dr = from.0.abs_diff(to.0);
    let dc = from.1.abs_diff(to.1);

    if dr + dc == 1 {
        return true;
    }

    dr == 1
        && dc == 1
        && (terrain_at(from.0, from.1) == Terrain::Camp || terrain_at(to.0, to.1) == Terrain::Camp)
}

fn railway_straight_path(
    from: (usize, usize),
    to: (usize, usize),
    occupied: &HashSet<(usize, usize)>,
) -> Option<Vec<(usize, usize)>> {
    if from.0 != to.0 && from.1 != to.1 {
        return None;
    }

    let row_step = (to.0 as isize - from.0 as isize).signum();
    let col_step = (to.1 as isize - from.1 as isize).signum();
    let mut row = from.0 as isize + row_step;
    let mut col = from.1 as isize + col_step;
    let mut path = vec![];

    while (row as usize, col as usize) != to {
        let current = (row as usize, col as usize);
        if terrain_at(current.0, current.1) != Terrain::Railway || occupied.contains(&current) {
            return None;
        }
        path.push(current);
        row += row_step;
        col += col_step;
    }

    path.push(to);
    Some(path)
}

fn railway_path(
    from: (usize, usize),
    to: (usize, usize),
    occupied: &HashSet<(usize, usize)>,
) -> Option<Vec<(usize, usize)>> {
    let mut queue = VecDeque::from([from]);
    let mut visited = HashSet::from([from]);
    let mut parent = HashMap::new();

    while let Some((row, col)) = queue.pop_front() {
        for next in railway_neighbors(row, col) {
            if occupied.contains(&next) && next != to {
                continue;
            }
            if !visited.insert(next) {
                continue;
            }

            parent.insert(next, (row, col));

            if next == to {
                return Some(rebuild_path(from, to, &parent));
            }

            queue.push_back(next);
        }
    }

    None
}

fn rebuild_path(
    from: (usize, usize),
    to: (usize, usize),
    parent: &HashMap<(usize, usize), (usize, usize)>,
) -> Vec<(usize, usize)> {
    let mut path = vec![to];
    let mut current = to;

    while let Some(previous) = parent.get(&current).copied() {
        if previous == from {
            break;
        }
        path.push(previous);
        current = previous;
    }

    path.reverse();
    path
}

fn railway_neighbors(row: usize, col: usize) -> Vec<(usize, usize)> {
    let mut neighbors = vec![];

    for (dr, dc) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
        let next_row = row as isize + dr;
        let next_col = col as isize + dc;
        if next_row < 0
            || next_row >= BOARD_ROWS as isize
            || next_col < 0
            || next_col >= BOARD_COLS as isize
        {
            continue;
        }

        let next = (next_row as usize, next_col as usize);
        if terrain_at(next.0, next.1) == Terrain::Railway {
            neighbors.push(next);
        }
    }

    neighbors
}
