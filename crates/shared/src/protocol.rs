use crate::types::{Camp, GameSnapshot, PieceState, Rank};

#[derive(Debug)]
pub enum ClientCommand {
    Hello,
    Reveal {
        row: usize,
        col: usize,
    },
    Move {
        from: (usize, usize),
        to: (usize, usize),
    },
}

#[derive(Clone, Debug)]
pub enum ServerMessage {
    Welcome { camp: Camp },
    Snapshot(GameSnapshot),
    Error(String),
    Event(String),
}

pub fn encode_client(command: &ClientCommand) -> String {
    match command {
        ClientCommand::Hello => "HELLO\n".into(),
        ClientCommand::Reveal { row, col } => format!("REVEAL {row} {col}\n"),
        ClientCommand::Move { from, to } => {
            format!("MOVE {} {} {} {}\n", from.0, from.1, to.0, to.1)
        }
    }
}

pub fn decode_client(line: &str) -> Option<ClientCommand> {
    let mut parts = line.split_whitespace();
    match parts.next()? {
        "HELLO" => Some(ClientCommand::Hello),
        "REVEAL" => Some(ClientCommand::Reveal {
            row: parts.next()?.parse().ok()?,
            col: parts.next()?.parse().ok()?,
        }),
        "MOVE" => Some(ClientCommand::Move {
            from: (parts.next()?.parse().ok()?, parts.next()?.parse().ok()?),
            to: (parts.next()?.parse().ok()?, parts.next()?.parse().ok()?),
        }),
        _ => None,
    }
}

pub fn encode_server(message: &ServerMessage) -> String {
    match message {
        ServerMessage::Welcome { camp } => format!("WELCOME {}\n", camp.code()),
        ServerMessage::Snapshot(snapshot) => {
            let pieces = snapshot
                .pieces
                .iter()
                .map(encode_piece)
                .collect::<Vec<_>>()
                .join(";");
            format!("SNAPSHOT {} {pieces}\n", snapshot.turn.code())
        }
        ServerMessage::Error(message) => format!("ERROR {}\n", message.replace('\n', " ")),
        ServerMessage::Event(message) => format!("EVENT {}\n", message.replace('\n', " ")),
    }
}

pub fn decode_server(line: &str) -> Option<ServerMessage> {
    let (kind, rest) = line
        .trim_end()
        .split_once(' ')
        .unwrap_or((line.trim_end(), ""));
    match kind {
        "WELCOME" => Some(ServerMessage::Welcome {
            camp: Camp::from_code(rest.trim())?,
        }),
        "SNAPSHOT" => {
            let (camp, pieces) = rest.split_once(' ').unwrap_or((rest, ""));
            let pieces = if pieces.is_empty() {
                vec![]
            } else {
                pieces.split(';').filter_map(decode_piece).collect()
            };
            Some(ServerMessage::Snapshot(GameSnapshot {
                turn: Camp::from_code(camp)?,
                pieces,
            }))
        }
        "ERROR" => Some(ServerMessage::Error(rest.into())),
        "EVENT" => Some(ServerMessage::Event(rest.into())),
        _ => None,
    }
}

fn encode_piece(piece: &PieceState) -> String {
    format!(
        "{},{},{},{},{},{},{}",
        piece.id,
        piece.camp.code(),
        piece.rank.code(),
        u8::from(piece.revealed),
        u8::from(piece.alive),
        piece.row,
        piece.col
    )
}

fn decode_piece(text: &str) -> Option<PieceState> {
    let mut parts = text.split(',');
    Some(PieceState {
        id: parts.next()?.parse().ok()?,
        camp: Camp::from_code(parts.next()?)?,
        rank: Rank::from_code(parts.next()?)?,
        revealed: parts.next()? == "1",
        alive: parts.next()? == "1",
        row: parts.next()?.parse().ok()?,
        col: parts.next()?.parse().ok()?,
    })
}
