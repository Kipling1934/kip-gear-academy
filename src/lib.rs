#![no_std]
#![allow(static_mut_refs)]
use gstd::{exec, msg, prelude::*};
use pebbles_game_io::*;

static mut GAME_STATE: Option<GameState> = None;

fn get_random_u32() -> u32 {
    let salt = msg::id();
    let (hash, _num) = exec::random(salt.into()).expect("get_random_u32(): random call failed");
    u32::from_le_bytes([hash[0], hash[1], hash[2], hash[3]])
}

fn get_state_mut() -> &'static mut GameState {
    unsafe { GAME_STATE.as_mut().expect("Game state not initialized") }
}

fn get_state() -> &'static GameState {
    unsafe { GAME_STATE.as_ref().expect("Game state not initialized") }
}

fn calculate_winning_move(pebbles: u32, max_per_turn: u32) -> Option<u32> {
    if pebbles == 0 {
        return None;
    }

    // Calculate optimal move strategy
    let remainder = pebbles % (max_per_turn + 1);
    if remainder == 0 {
        // If in losing position, choose a random move
        Some((get_random_u32() % max_per_turn) + 1)
    } else {
        // If in winning position, choose the optimal move
        Some(remainder)
    }
}

fn make_program_move() -> PebblesEvent {
    let state = get_state_mut();

    if state.pebbles_remaining == 0 {
        return PebblesEvent::Won(Player::User);
    }

    let pebbles_to_remove = match state.difficulty {
        DifficultyLevel::Easy => {
            // In easy mode, choose moves randomly
            (get_random_u32() % state.max_pebbles_per_turn) + 1
        }
        DifficultyLevel::Hard => {
            // In hard mode, use optimal strategy
            calculate_winning_move(state.pebbles_remaining, state.max_pebbles_per_turn).unwrap_or(1)
        }
    };

    let pebbles_to_remove = pebbles_to_remove.min(state.pebbles_remaining);
    state.pebbles_remaining -= pebbles_to_remove;

    if state.pebbles_remaining == 0 {
        state.winner = Some(Player::Program);
        PebblesEvent::Won(Player::Program)
    } else {
        PebblesEvent::CounterTurn(pebbles_to_remove)
    }
}

#[no_mangle]
extern "C" fn init() {
    let init_config: PebblesInit = msg::load().expect("Failed to decode init message");

    if init_config.pebbles_count == 0 {
        panic!("Initial pebbles count must be greater than 0");
    }
    if init_config.max_pebbles_per_turn == 0 {
        panic!("Max pebbles per turn must be greater than 0");
    }
    if init_config.max_pebbles_per_turn >= init_config.pebbles_count {
        panic!("Max pebbles per turn must be less than total pebbles");
    }

    let first_player = if get_random_u32() % 2 == 0 {
        Player::User
    } else {
        Player::Program
    };

    let state = GameState {
        pebbles_count: init_config.pebbles_count,
        max_pebbles_per_turn: init_config.max_pebbles_per_turn,
        pebbles_remaining: init_config.pebbles_count,
        difficulty: init_config.difficulty,
        first_player: first_player.clone(),
        winner: None,
    };

    unsafe { GAME_STATE = Some(state) };

    // If program goes first, make the first move
    if matches!(first_player, Player::Program) {
        let event = make_program_move();
        msg::reply(event, 0).expect("Failed to reply");
    }
}

#[no_mangle]
extern "C" fn handle() {
    let action: PebblesAction = msg::load().expect("Failed to decode action");
    let state = get_state_mut();

    let event = match action {
        PebblesAction::Turn(pebbles) => {
            if state.winner.is_some() {
                panic!("Game is already finished");
            }
            if pebbles == 0 || pebbles > state.max_pebbles_per_turn {
                panic!("Invalid number of pebbles");
            }
            if pebbles > state.pebbles_remaining {
                panic!("Not enough pebbles remaining");
            }

            state.pebbles_remaining -= pebbles;

            if state.pebbles_remaining == 0 {
                state.winner = Some(Player::User);
                PebblesEvent::Won(Player::User)
            } else {
                make_program_move()
            }
        }
        PebblesAction::GiveUp => {
            state.winner = Some(Player::Program);
            PebblesEvent::Won(Player::Program)
        }
        PebblesAction::Restart {
            difficulty,
            pebbles_count,
            max_pebbles_per_turn,
        } => {
            if pebbles_count == 0 {
                panic!("Initial pebbles count must be greater than 0");
            }
            if max_pebbles_per_turn == 0 {
                panic!("Max pebbles per turn must be greater than 0");
            }
            if max_pebbles_per_turn >= pebbles_count {
                panic!("Max pebbles per turn must be less than total pebbles");
            }

            let first_player = if get_random_u32() % 2 == 0 {
                Player::User
            } else {
                Player::Program
            };

            state.difficulty = difficulty;
            state.pebbles_count = pebbles_count;
            state.max_pebbles_per_turn = max_pebbles_per_turn;
            state.pebbles_remaining = pebbles_count;
            state.first_player = first_player.clone();
            state.winner = None;

            if matches!(first_player, Player::Program) {
                make_program_move()
            } else {
                PebblesEvent::CounterTurn(0)
            }
        }
    };

    msg::reply(event, 0).expect("Failed to reply");
}

#[no_mangle]
extern "C" fn state() {
    msg::reply(get_state(), 0).expect("Failed to reply with state");
}
