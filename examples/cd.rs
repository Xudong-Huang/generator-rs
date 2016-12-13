#[macro_use]
extern crate generator;
use generator::*;

#[derive(Debug)]
enum Action {
    Play(&'static str),
    Stop,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum State {
    Playing,
    Stopped,
}

use Action::*;
use State::*;

fn main() {
    let mut cd_player = Gn::new_scoped(|mut s| {
        let mut state = Stopped;
        loop {
            match s.get_yield() {
                Some(Play(s)) => {
                    if state == Stopped {
                        println!("I'm playing {}", s);
                        state = Playing;
                    } else {
                        println!("should first stop");
                    }
                }
                Some(Stop) => {
                    if state == Playing {
                        println!("I'm stopped");
                        state = Stopped;
                    } else {
                        println!("I'm already stopped");
                    }
                }
                a => {
                    println!("invalid action: {:?}", a);
                }
            }
            s.yield_with(state);
        }
    });

    cd_player.send(Play("hello world"));
    cd_player.send(Play("hello another day"));
    cd_player.send(Stop);
    cd_player.send(Stop);
    cd_player.send(Play("hello another day"));
}
