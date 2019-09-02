use generator::*;

#[derive(Debug)]
enum Action {
    Play(&'static str),
    Stop,
}

#[derive(Debug, Clone, Copy)]
enum State {
    Playing,
    Stopped,
}

use crate::Action::*;
use crate::State::*;

fn main() {
    let mut cd_player = Gn::new_scoped(|mut s| {
        let mut state = Stopped;
        loop {
            match state {
                Stopped => match s.get_yield() {
                    Some(Play(t)) => {
                        println!("I'm playing {}", t);
                        state = Playing;
                    }
                    Some(Stop) => println!("I'm already stopped"),
                    _ => unreachable!("some thing wrong"),
                },

                Playing => match s.get_yield() {
                    Some(Stop) => {
                        println!("I'm stopped");
                        state = Stopped;
                    }
                    Some(Play(_)) => println!("should first stop"),
                    _ => unreachable!("some thing wrong"),
                },
            }

            s.yield_with(state);
        }
    });

    for _ in 0..1000 {
        cd_player.send(Play("hello world"));
        cd_player.send(Play("hello another day"));
        cd_player.send(Stop);
        cd_player.send(Stop);
        cd_player.send(Play("hello another day"));
        cd_player.send(Stop);
    }
}
