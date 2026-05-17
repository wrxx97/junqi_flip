pub const BOARD_ROWS: usize = 12;
pub const BOARD_COLS: usize = 5;
pub const BLUE_HOME_ROWS: std::ops::Range<usize> = 0..5;
pub const RED_HOME_ROWS: std::ops::Range<usize> = 7..12;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Terrain {
    Road,
    Railway,
    Camp,
}

pub struct BoardConnection {
    pub from: (usize, usize),
    pub to: (usize, usize),
    pub railway: bool,
}

pub fn terrain_at(row: usize, col: usize) -> Terrain {
    if is_camp(row, col) {
        Terrain::Camp
    } else if is_railway(row, col) {
        Terrain::Railway
    } else {
        Terrain::Road
    }
}

pub fn is_camp(row: usize, col: usize) -> bool {
    matches!(
        (row, col),
        (2, 1) | (2, 3) | (3, 2) | (4, 1) | (4, 3) | (7, 1) | (7, 3) | (8, 2) | (9, 1) | (9, 3)
    )
}

fn is_railway(row: usize, col: usize) -> bool {
    if is_camp(row, col) {
        return false;
    }

    matches!(row, 1 | 5 | 6 | 10) || ((1..=10).contains(&row) && matches!(col, 0 | 2 | 4))
}

pub fn initial_positions() -> Vec<(usize, usize)> {
    BLUE_HOME_ROWS
        .chain(RED_HOME_ROWS)
        .flat_map(|row| (0..BOARD_COLS).map(move |col| (row, col)))
        .filter(|(row, col)| terrain_at(*row, *col) != Terrain::Camp)
        .collect()
}

pub fn board_connections() -> Vec<BoardConnection> {
    let mut connections = vec![];

    for row in 0..BOARD_ROWS {
        for col in 0..BOARD_COLS {
            for (next_row, next_col) in [(row + 1, col), (row, col + 1)] {
                if next_row < BOARD_ROWS && next_col < BOARD_COLS {
                    connections.push(connection((row, col), (next_row, next_col)));
                }
            }
        }
    }

    for row in 1..BOARD_ROWS {
        for col in 1..BOARD_COLS {
            let from = (row - 1, col - 1);
            let to = (row, col);
            if is_camp(from.0, from.1) || is_camp(to.0, to.1) {
                connections.push(connection(from, to));
            }
        }
    }

    for row in 1..BOARD_ROWS {
        for col in 0..BOARD_COLS - 1 {
            let from = (row - 1, col + 1);
            let to = (row, col);
            if is_camp(from.0, from.1) || is_camp(to.0, to.1) {
                connections.push(connection(from, to));
            }
        }
    }

    connections
}

fn connection(from: (usize, usize), to: (usize, usize)) -> BoardConnection {
    BoardConnection {
        from,
        to,
        railway: terrain_at(from.0, from.1) == Terrain::Railway
            && terrain_at(to.0, to.1) == Terrain::Railway,
    }
}
