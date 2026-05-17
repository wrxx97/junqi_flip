#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Camp {
    #[default]
    Red,
    Blue,
}

impl Camp {
    pub fn next(self) -> Self {
        match self {
            Camp::Red => Camp::Blue,
            Camp::Blue => Camp::Red,
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Camp::Red => "R",
            Camp::Blue => "B",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "R" => Some(Camp::Red),
            "B" => Some(Camp::Blue),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rank {
    Unknown,
    Flag,
    Mine,
    Bomb,
    Engineer,
    Platoon,
    Company,
    Battalion,
    Regiment,
    Brigade,
    Division,
    Corps,
    Commander,
}

impl Rank {
    pub fn strength(self) -> i32 {
        match self {
            Rank::Unknown => 0,
            Rank::Flag => 0,
            Rank::Engineer => 2,
            Rank::Platoon => 3,
            Rank::Company => 4,
            Rank::Battalion => 5,
            Rank::Regiment => 6,
            Rank::Brigade => 7,
            Rank::Division => 8,
            Rank::Corps => 9,
            Rank::Commander => 10,
            Rank::Mine | Rank::Bomb => 99,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Rank::Unknown => "?",
            Rank::Flag => "军旗",
            Rank::Mine => "地雷",
            Rank::Bomb => "炸弹",
            Rank::Engineer => "工兵",
            Rank::Platoon => "排长",
            Rank::Company => "连长",
            Rank::Battalion => "营长",
            Rank::Regiment => "团长",
            Rank::Brigade => "旅长",
            Rank::Division => "师长",
            Rank::Corps => "军长",
            Rank::Commander => "司令",
        }
    }

    pub fn can_move(self) -> bool {
        !matches!(self, Rank::Unknown | Rank::Flag | Rank::Mine)
    }

    pub fn code(self) -> &'static str {
        match self {
            Rank::Unknown => "U",
            Rank::Flag => "F",
            Rank::Mine => "M",
            Rank::Bomb => "X",
            Rank::Engineer => "E",
            Rank::Platoon => "P",
            Rank::Company => "C",
            Rank::Battalion => "A",
            Rank::Regiment => "G",
            Rank::Brigade => "B",
            Rank::Division => "D",
            Rank::Corps => "O",
            Rank::Commander => "S",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "U" => Some(Rank::Unknown),
            "F" => Some(Rank::Flag),
            "M" => Some(Rank::Mine),
            "X" => Some(Rank::Bomb),
            "E" => Some(Rank::Engineer),
            "P" => Some(Rank::Platoon),
            "C" => Some(Rank::Company),
            "A" => Some(Rank::Battalion),
            "G" => Some(Rank::Regiment),
            "B" => Some(Rank::Brigade),
            "D" => Some(Rank::Division),
            "O" => Some(Rank::Corps),
            "S" => Some(Rank::Commander),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PieceState {
    pub id: u32,
    pub camp: Camp,
    pub rank: Rank,
    pub revealed: bool,
    pub alive: bool,
    pub row: usize,
    pub col: usize,
}

#[derive(Clone, Debug, Default)]
pub struct GameSnapshot {
    pub turn: Camp,
    pub pieces: Vec<PieceState>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BattleOutcome {
    AttackerWins,
    DefenderWins,
    BothRemoved,
}
