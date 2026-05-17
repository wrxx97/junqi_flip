use junqi_shared::board::{Terrain, initial_positions, terrain_at};
use junqi_shared::protocol::{ClientCommand, ServerMessage, decode_client, encode_server};
use junqi_shared::rules::{move_path, resolve_battle, should_explode, would_lose_attack};
use junqi_shared::types::{BattleOutcome, Camp, GameSnapshot, PieceState, Rank};
use rand::seq::SliceRandom;
use std::collections::HashSet;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

const ADDRESS: &str = "127.0.0.1:7878";

struct Player {
    camp: Camp,
    stream: TcpStream,
}

struct PlayerCommand {
    camp: Camp,
    command: ClientCommand,
}

struct Game {
    turn: Camp,
    pieces: Vec<PieceState>,
}

fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind(ADDRESS)?;
    println!("军旗服务器已启动: {ADDRESS}");
    println!("等待两个客户端连接...");

    let (tx, rx) = mpsc::channel();
    let mut players = vec![];

    for camp in [Camp::Red, Camp::Blue] {
        let (stream, addr) = listener.accept()?;
        println!("{camp:?} 玩家已连接: {addr}");
        stream.set_nodelay(true)?;

        let mut write_stream = stream.try_clone()?;
        write_message(&mut write_stream, &ServerMessage::Welcome { camp });
        spawn_reader(stream.try_clone()?, camp, tx.clone());
        players.push(Player {
            camp,
            stream: write_stream,
        });
    }

    let mut game = Game::new();
    broadcast(&mut players, &ServerMessage::Snapshot(game.snapshot()));

    run_game_loop(&mut game, &mut players, rx);
    Ok(())
}

fn spawn_reader(stream: TcpStream, camp: Camp, tx: Sender<PlayerCommand>) {
    thread::spawn(move || {
        let reader = BufReader::new(stream);
        for line in reader.lines().map_while(Result::ok) {
            if let Some(command) = decode_client(&line) {
                let _ = tx.send(PlayerCommand { camp, command });
            }
        }
    });
}

fn run_game_loop(game: &mut Game, players: &mut [Player], rx: Receiver<PlayerCommand>) {
    while let Ok(player_command) = rx.recv() {
        let result = game.apply(player_command.camp, player_command.command);
        if let Err(message) = result {
            send_to(players, player_command.camp, &ServerMessage::Error(message));
        }
        broadcast(players, &ServerMessage::Snapshot(game.snapshot()));
    }
}

impl Game {
    fn new() -> Self {
        let mut positions = initial_positions();
        let mut pieces = create_pieces();
        let mut rng = rand::rng();
        positions.shuffle(&mut rng);
        pieces.shuffle(&mut rng);

        let pieces = positions
            .into_iter()
            .zip(pieces)
            .enumerate()
            .map(|(index, ((row, col), (camp, rank)))| PieceState {
                id: index as u32,
                camp,
                rank,
                revealed: false,
                alive: true,
                row,
                col,
            })
            .collect();

        Self {
            turn: Camp::Red,
            pieces,
        }
    }

    fn snapshot(&self) -> GameSnapshot {
        GameSnapshot {
            turn: self.turn,
            pieces: self
                .pieces
                .iter()
                .map(|piece| PieceState {
                    rank: if piece.revealed {
                        piece.rank
                    } else {
                        Rank::Unknown
                    },
                    ..*piece
                })
                .collect(),
        }
    }

    fn apply(&mut self, camp: Camp, command: ClientCommand) -> Result<(), String> {
        match command {
            ClientCommand::Hello => Ok(()),
            ClientCommand::Reveal { row, col } => self.reveal(camp, row, col),
            ClientCommand::Move { from, to } => self.move_piece(camp, from, to),
        }
    }

    fn reveal(&mut self, camp: Camp, row: usize, col: usize) -> Result<(), String> {
        self.ensure_turn(camp)?;
        let piece = self
            .piece_at_mut(row, col)
            .ok_or_else(|| "这里没有棋子".to_string())?;

        if piece.revealed {
            return Err("这个棋子已经翻开了".into());
        }

        piece.revealed = true;
        self.turn = self.turn.next();
        Ok(())
    }

    fn move_piece(
        &mut self,
        camp: Camp,
        from: (usize, usize),
        to: (usize, usize),
    ) -> Result<(), String> {
        self.ensure_turn(camp)?;

        let attacker_index = self
            .piece_index_at(from.0, from.1)
            .ok_or_else(|| "起点没有棋子".to_string())?;
        let attacker = self.pieces[attacker_index];

        if attacker.camp != camp {
            return Err("只能移动自己的棋子".into());
        }
        if !attacker.revealed {
            return Err("只能移动已翻开的棋子".into());
        }

        let defender_index = self.piece_index_at(to.0, to.1);
        if let Some(index) = defender_index {
            let defender = self.pieces[index];
            if defender.camp == attacker.camp {
                return Err("不能吃自己的棋子".into());
            }
            if terrain_at(defender.row, defender.col) == Terrain::Camp {
                return Err("行营中的棋子不能被攻击".into());
            }
            if defender.rank == Rank::Flag && self.has_alive_mine(defender.camp) {
                return Err("该阵营仍有地雷，军旗暂时不能被吃".into());
            }
            if would_lose_attack(attacker.rank, defender.rank) {
                return Err("弱子不能主动攻击强子".into());
            }
        }

        let occupied = self.occupied_cells(Some(attacker.id));
        if move_path(attacker, to.0, to.1, &occupied).is_none() {
            return Err("不符合移动规则".into());
        }

        if let Some(index) = defender_index {
            self.resolve_attack(attacker_index, index, to);
        } else {
            self.pieces[attacker_index].row = to.0;
            self.pieces[attacker_index].col = to.1;
        }

        self.turn = self.turn.next();
        Ok(())
    }

    fn resolve_attack(&mut self, attacker_index: usize, defender_index: usize, to: (usize, usize)) {
        let attacker = self.pieces[attacker_index];
        let defender = self.pieces[defender_index];
        let explosive = should_explode(attacker.rank, defender.rank);

        match resolve_battle(attacker.rank, defender.rank) {
            BattleOutcome::AttackerWins => {
                self.pieces[defender_index].alive = false;
                if explosive {
                    self.pieces[attacker_index].alive = false;
                } else {
                    self.pieces[attacker_index].row = to.0;
                    self.pieces[attacker_index].col = to.1;
                }
            }
            BattleOutcome::DefenderWins => self.pieces[attacker_index].alive = false,
            BattleOutcome::BothRemoved => {
                self.pieces[attacker_index].alive = false;
                self.pieces[defender_index].alive = false;
            }
        }
    }

    fn ensure_turn(&self, camp: Camp) -> Result<(), String> {
        if self.turn == camp {
            Ok(())
        } else {
            Err("还没轮到你".into())
        }
    }

    fn piece_index_at(&self, row: usize, col: usize) -> Option<usize> {
        self.pieces
            .iter()
            .position(|piece| piece.alive && piece.row == row && piece.col == col)
    }

    fn piece_at_mut(&mut self, row: usize, col: usize) -> Option<&mut PieceState> {
        let index = self.piece_index_at(row, col)?;
        self.pieces.get_mut(index)
    }

    fn occupied_cells(&self, ignore_id: Option<u32>) -> HashSet<(usize, usize)> {
        self.pieces
            .iter()
            .filter(|piece| piece.alive && Some(piece.id) != ignore_id)
            .map(|piece| (piece.row, piece.col))
            .collect()
    }

    fn has_alive_mine(&self, camp: Camp) -> bool {
        self.pieces
            .iter()
            .any(|piece| piece.alive && piece.camp == camp && piece.rank == Rank::Mine)
    }
}

fn create_pieces() -> Vec<(Camp, Rank)> {
    [Camp::Red, Camp::Blue]
        .into_iter()
        .flat_map(create_camp_pieces)
        .collect()
}

fn create_camp_pieces(camp: Camp) -> Vec<(Camp, Rank)> {
    let mut pieces = vec![
        (camp, Rank::Flag),
        (camp, Rank::Commander),
        (camp, Rank::Corps),
        (camp, Rank::Bomb),
    ];

    pieces.extend([(camp, Rank::Mine); 2]);
    pieces.extend([(camp, Rank::Engineer); 2]);
    pieces.extend([(camp, Rank::Platoon); 2]);
    pieces.extend([(camp, Rank::Company); 2]);
    pieces.extend([(camp, Rank::Battalion); 2]);
    pieces.extend([(camp, Rank::Regiment); 2]);
    pieces.extend([(camp, Rank::Brigade); 2]);
    pieces.extend([(camp, Rank::Division); 2]);
    pieces
}

fn broadcast(players: &mut [Player], message: &ServerMessage) {
    for player in players {
        write_message(&mut player.stream, message);
    }
}

fn send_to(players: &mut [Player], camp: Camp, message: &ServerMessage) {
    if let Some(player) = players.iter_mut().find(|player| player.camp == camp) {
        write_message(&mut player.stream, message);
    }
}

fn write_message(stream: &mut TcpStream, message: &ServerMessage) {
    let _ = stream.write_all(encode_server(message).as_bytes());
}
