use bevy::prelude::*;
use junqi_shared::board::{BOARD_COLS, BOARD_ROWS, Terrain, board_connections, terrain_at};
use junqi_shared::protocol::{ClientCommand, ServerMessage, decode_server, encode_client};
use junqi_shared::types::{Camp, GameSnapshot, PieceState};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::sync::Mutex;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

const ADDRESS: &str = "127.0.0.1:7878";
const CELL_SIZE: f32 = 48.0;
const PIECE_WIDTH: f32 = 40.0;
const PIECE_HEIGHT: f32 = 32.0;
const WOOD_GRAIN_WIDTH: f32 = 32.0;

#[derive(Resource)]
struct NetworkState {
    outbound: Option<Sender<ClientCommand>>,
    inbound: Mutex<Receiver<ServerMessage>>,
    camp: Option<Camp>,
    snapshot: GameSnapshot,
    selected: Option<(usize, usize)>,
    dirty: bool,
    message: String,
}

#[derive(Component)]
struct PieceView;

#[derive(Component)]
struct SelectionBorder;

#[derive(Component)]
struct StatusText;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "军旗网络对战 Client".into(),
                resolution: (900, 700).into(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(connect_to_server())
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                poll_network,
                render_snapshot,
                handle_click,
                update_selection_borders,
                update_status_text,
            ),
        )
        .run();
}

fn connect_to_server() -> NetworkState {
    let (incoming_tx, incoming_rx) = mpsc::channel();

    match TcpStream::connect(ADDRESS) {
        Ok(stream) => {
            let (outgoing_tx, outgoing_rx) = mpsc::channel::<ClientCommand>();
            let mut writer = stream.try_clone().expect("clone client writer");
            let reader = stream;

            thread::spawn(move || {
                for command in outgoing_rx {
                    let _ = writer.write_all(encode_client(&command).as_bytes());
                }
            });

            thread::spawn(move || {
                let reader = BufReader::new(reader);
                for line in reader.lines().map_while(Result::ok) {
                    if let Some(message) = decode_server(&line) {
                        let _ = incoming_tx.send(message);
                    }
                }
            });

            let _ = outgoing_tx.send(ClientCommand::Hello);

            NetworkState {
                outbound: Some(outgoing_tx),
                inbound: Mutex::new(incoming_rx),
                camp: None,
                snapshot: GameSnapshot::default(),
                selected: None,
                dirty: true,
                message: format!("已连接服务器 {ADDRESS}"),
            }
        }
        Err(error) => NetworkState {
            outbound: None,
            inbound: Mutex::new(incoming_rx),
            camp: None,
            snapshot: GameSnapshot::default(),
            selected: None,
            dirty: true,
            message: format!("连接服务器失败: {error}"),
        },
    }
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(Camera2d);
    spawn_board(&mut commands);

    let font = asset_server.load("fonts/simhei.ttf");
    commands.spawn((
        Text2d::new("等待服务器..."),
        TextFont {
            font,
            font_size: 20.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Transform::from_xyz(0.0, 310.0, 5.0),
        StatusText,
    ));
}

fn poll_network(mut network: ResMut<NetworkState>) {
    let mut messages = vec![];
    {
        let Ok(inbound) = network.inbound.lock() else {
            return;
        };
        while let Ok(message) = inbound.try_recv() {
            messages.push(message);
        }
    }

    for message in messages {
        match message {
            ServerMessage::Welcome { camp } => {
                network.camp = Some(camp);
                network.message = format!("你是 {:?} 方", camp);
            }
            ServerMessage::Snapshot(snapshot) => {
                network.snapshot = snapshot;
                network.dirty = true;
            }
            ServerMessage::Error(message) | ServerMessage::Event(message) => {
                network.message = message;
            }
        }
    }
}

fn render_snapshot(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut network: ResMut<NetworkState>,
    views: Query<Entity, With<PieceView>>,
) {
    if !network.dirty {
        return;
    }
    network.dirty = false;

    for entity in views.iter() {
        commands.entity(entity).despawn();
    }

    let font = asset_server.load("fonts/simhei.ttf");
    for piece in network
        .snapshot
        .pieces
        .iter()
        .copied()
        .filter(|piece| piece.alive)
    {
        spawn_piece(&mut commands, &font, piece);
    }
}

fn handle_click(
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform)>,
    mut network: ResMut<NetworkState>,
) {
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }

    let window = windows.single().unwrap();
    let Ok((camera, camera_transform)) = cameras.single() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let Ok(world) = camera.viewport_to_world_2d(camera_transform, cursor) else {
        return;
    };
    let Some((row, col)) = board_pos_from_cursor(world) else {
        return;
    };

    let clicked = network
        .snapshot
        .pieces
        .iter()
        .find(|piece| piece.alive && piece.row == row && piece.col == col)
        .copied();

    let Some(outbound) = network.outbound.clone() else {
        network.message = "未连接服务器".into();
        return;
    };

    if let Some(piece) = clicked {
        if !piece.revealed {
            let _ = outbound.send(ClientCommand::Reveal { row, col });
            network.selected = None;
            return;
        }

        if network.selected.is_none()
            && Some(piece.camp) == network.camp
            && Some(piece.camp) == Some(network.snapshot.turn)
        {
            network.selected = Some((row, col));
            return;
        }
    }

    if let Some(from) = network.selected.take() {
        let _ = outbound.send(ClientCommand::Move {
            from,
            to: (row, col),
        });
    }
}

fn update_selection_borders(
    network: Res<NetworkState>,
    pieces: Query<(&PieceStateView, &Children), With<PieceView>>,
    mut borders: Query<&mut Sprite, With<SelectionBorder>>,
) {
    for (view, children) in pieces.iter() {
        for child in children.iter() {
            if let Ok(mut sprite) = borders.get_mut(child) {
                sprite.color = if network.selected == Some((view.row, view.col)) {
                    Color::srgba(0.98, 0.86, 0.28, 1.0)
                } else {
                    Color::srgba(0.98, 0.86, 0.28, 0.0)
                };
            }
        }
    }
}

fn update_status_text(network: Res<NetworkState>, mut query: Query<&mut Text2d, With<StatusText>>) {
    let Ok(mut text) = query.single_mut() else {
        return;
    };

    let side = network
        .camp
        .map(|camp| format!("{camp:?}"))
        .unwrap_or_else(|| "未分配".into());
    text.0 = format!(
        "你是 {side} 方 | 当前回合: {:?} | {}",
        network.snapshot.turn, network.message
    );
}

#[derive(Component)]
struct PieceStateView {
    row: usize,
    col: usize,
}

fn spawn_piece(commands: &mut Commands, font: &Handle<Font>, piece: PieceState) {
    let text_color = if piece.revealed {
        camp_text_color(piece.camp)
    } else {
        Color::srgb(0.20, 0.12, 0.05)
    };

    commands
        .spawn((
            Sprite::from_color(
                Color::srgb(0.72, 0.50, 0.27),
                Vec2::new(PIECE_WIDTH, PIECE_HEIGHT),
            ),
            Transform::from_translation(cell_translation(piece.row, piece.col, 1.0)),
            PieceView,
            PieceStateView {
                row: piece.row,
                col: piece.col,
            },
        ))
        .with_children(|parent| {
            parent.spawn((
                Sprite::from_color(
                    Color::srgba(0.98, 0.86, 0.28, 0.0),
                    Vec2::new(PIECE_WIDTH + 7.0, PIECE_HEIGHT + 7.0),
                ),
                Transform::from_xyz(0.0, 0.0, -0.1),
                SelectionBorder,
            ));

            parent.spawn((
                Sprite::from_color(
                    Color::srgba(0.96, 0.76, 0.42, 0.45),
                    Vec2::new(PIECE_WIDTH - 4.0, 2.0),
                ),
                Transform::from_xyz(0.0, PIECE_HEIGHT * 0.34, 0.05),
            ));

            for (y, width, alpha) in [
                (-9.0, WOOD_GRAIN_WIDTH, 0.35),
                (-4.0, WOOD_GRAIN_WIDTH - 7.0, 0.28),
                (3.0, WOOD_GRAIN_WIDTH - 2.0, 0.30),
                (8.0, WOOD_GRAIN_WIDTH - 10.0, 0.24),
            ] {
                parent.spawn((
                    Sprite::from_color(
                        Color::srgba(0.32, 0.18, 0.07, alpha),
                        Vec2::new(width, 1.2),
                    ),
                    Transform::from_xyz(0.0, y, 0.08),
                ));
            }

            parent.spawn((
                Sprite::from_color(Color::srgba(0.38, 0.20, 0.08, 0.55), Vec2::new(2.0, 24.0)),
                Transform::from_xyz(-PIECE_WIDTH * 0.42, 0.0, 0.07),
            ));
            parent.spawn((
                Sprite::from_color(Color::srgba(0.94, 0.71, 0.38, 0.45), Vec2::new(2.0, 23.0)),
                Transform::from_xyz(PIECE_WIDTH * 0.42, 0.0, 0.07),
            ));

            parent.spawn((
                Text2d::new(if piece.revealed {
                    piece.rank.label()
                } else {
                    "?"
                }),
                TextFont {
                    font: font.clone(),
                    font_size: 18.0,
                    ..default()
                },
                TextColor(text_color),
                Transform::from_xyz(0.0, 0.0, 0.2),
            ));
        });
}

fn camp_text_color(camp: Camp) -> Color {
    match camp {
        Camp::Red => Color::srgb(0.70, 0.03, 0.02),
        Camp::Blue => Color::srgb(0.03, 0.15, 0.64),
    }
}

fn spawn_board(commands: &mut Commands) {
    for link in board_connections() {
        let from = cell_translation(link.from.0, link.from.1, -1.3);
        let to = cell_translation(link.to.0, link.to.1, -1.3);

        if link.railway {
            spawn_line(
                commands,
                from,
                to,
                6.0,
                Color::srgb(0.11, 0.10, 0.09),
                -1.35,
            );
            spawn_line(
                commands,
                from,
                to,
                2.0,
                Color::srgb(0.68, 0.59, 0.39),
                -1.25,
            );
            spawn_rail_sleepers(commands, from, to);
        } else {
            spawn_line(commands, from, to, 1.5, Color::srgb(0.20, 0.17, 0.12), -1.4);
        }
    }

    for row in 0..BOARD_ROWS {
        for col in 0..BOARD_COLS {
            let terrain = terrain_at(row, col);
            let color = match terrain {
                Terrain::Road => Color::srgb(0.37, 0.32, 0.24),
                Terrain::Railway => Color::srgb(0.28, 0.27, 0.23),
                Terrain::Camp => Color::srgb(0.17, 0.34, 0.21),
            };
            let size = match terrain {
                Terrain::Camp => Vec2::new(42.0, 30.0),
                _ => Vec2::new(42.0, 32.0),
            };

            commands.spawn((
                Sprite::from_color(color, size),
                Transform::from_translation(cell_translation(row, col, -0.6)),
            ));

            if terrain == Terrain::Camp {
                spawn_ellipse_outline(commands, row, col);
            }
        }
    }
}

fn spawn_rail_sleepers(commands: &mut Commands, from: Vec3, to: Vec3) {
    let direction = (to - from).truncate().normalize_or_zero();
    if direction == Vec2::ZERO {
        return;
    }

    let normal = Vec2::new(-direction.y, direction.x);
    for fraction in [0.25, 0.5, 0.75] {
        let center = from.lerp(to, fraction);
        let start = center + (normal * 6.0).extend(0.0);
        let end = center - (normal * 6.0).extend(0.0);
        spawn_line(
            commands,
            start,
            end,
            1.5,
            Color::srgb(0.78, 0.68, 0.48),
            -1.2,
        );
    }
}

fn spawn_ellipse_outline(commands: &mut Commands, row: usize, col: usize) {
    let center = cell_translation(row, col, -0.45);
    let rx = 22.0;
    let ry = 16.0;
    let segments = 28;

    for index in 0..segments {
        let a = index as f32 / segments as f32 * std::f32::consts::TAU;
        let b = (index + 1) as f32 / segments as f32 * std::f32::consts::TAU;
        let from = center + Vec3::new(a.cos() * rx, a.sin() * ry, 0.0);
        let to = center + Vec3::new(b.cos() * rx, b.sin() * ry, 0.0);
        spawn_line(commands, from, to, 2.0, Color::srgb(0.78, 0.95, 0.62), -0.4);
    }
}

fn spawn_line(commands: &mut Commands, from: Vec3, to: Vec3, width: f32, color: Color, z: f32) {
    let delta = to - from;
    let length = delta.truncate().length();
    let angle = delta.y.atan2(delta.x);
    let midpoint = from.lerp(to, 0.5);

    commands.spawn((
        Sprite::from_color(color, Vec2::new(length, width)),
        Transform {
            translation: Vec3::new(midpoint.x, midpoint.y, z),
            rotation: Quat::from_rotation_z(angle),
            ..default()
        },
    ));
}

fn cell_translation(row: usize, col: usize, z: f32) -> Vec3 {
    let board_width = BOARD_COLS as f32 * CELL_SIZE;
    let board_height = BOARD_ROWS as f32 * CELL_SIZE;
    let x = col as f32 * CELL_SIZE - board_width / 2.0 + CELL_SIZE / 2.0;
    let y = board_height / 2.0 - row as f32 * CELL_SIZE - CELL_SIZE / 2.0;
    Vec3::new(x, y, z)
}

fn board_pos_from_cursor(cursor: Vec2) -> Option<(usize, usize)> {
    let board_width = BOARD_COLS as f32 * CELL_SIZE;
    let board_height = BOARD_ROWS as f32 * CELL_SIZE;
    let x = cursor.x + board_width / 2.0;
    let y = board_height / 2.0 - cursor.y;

    let col = (x / CELL_SIZE).floor() as isize;
    let row = (y / CELL_SIZE).floor() as isize;

    if row >= 0 && row < BOARD_ROWS as isize && col >= 0 && col < BOARD_COLS as isize {
        Some((row as usize, col as usize))
    } else {
        None
    }
}
