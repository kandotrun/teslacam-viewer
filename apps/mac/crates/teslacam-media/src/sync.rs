//! The synchronized-start state machine shared by all players of an event.
//!
//! Pure logic: it never touches AVFoundation. It turns commands (play, pause, seek, rate) and
//! completions (seek finished, preroll finished, frames ready) into [`Action`]s that the AVFoundation
//! adapter executes on every player at once. The sequence proven by the spike is
//!
//! 1. pause every player,
//! 2. seek every player exactly (zero tolerance),
//! 3. preroll every player at the target rate,
//! 4. wait until every player can hand out the target frame (at most [`FRAME_WAIT_SECONDS`]),
//! 5. start every player with `setRate:time:atHostTime:` on one host time [`START_LEAD_SECONDS`] ahead.
//!
//! All times are host seconds (`CACurrentMediaTime`, the host time clock the players run on) or event
//! seconds (position on the event timeline).

/// How far ahead of "now" the shared start is scheduled, so every player receives it in time.
pub const START_LEAD_SECONDS: f64 = 0.05;
/// Longest wait for decoded target frames before starting anyway.
pub const FRAME_WAIT_SECONDS: f64 = 0.3;
/// Longest wait for seek or preroll completions before continuing anyway.
pub const SETTLE_TIMEOUT_SECONDS: f64 = 3.0;

/// What the app asks for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Command {
    /// Start (or keep) playing.
    Play,
    /// Pause at the current position.
    Pause,
    /// Move to an event time (clamped to the event).
    Seek(f64),
    /// Change the playback rate (0.5…4).
    SetRate(f32),
}

/// What the adapter must do to every player.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Action {
    /// Stop every player now (`setRate:0`).
    PauseAll,
    /// Seek every player exactly to `target`; report [`SyncMachine::seek_done`] with `generation` once
    /// all completions arrived.
    SeekAll {
        /// Event time.
        target: f64,
        /// Tag for the completion.
        generation: u64,
    },
    /// Preroll every player at `rate`; report [`SyncMachine::preroll_done`] with `generation`.
    PrerollAll {
        /// Rate to preroll for.
        rate: f32,
        /// Tag for the completion.
        generation: u64,
    },
    /// `setRate:rate time:position atHostTime:host_time` on every player.
    StartAll {
        /// Playback rate.
        rate: f32,
        /// Event time to start from.
        position: f64,
        /// Host time (seconds) at which every player starts.
        host_time: f64,
    },
}

/// Something the app should hear about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Notice {
    /// Playback reached the end of the event and paused there.
    Ended,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Step {
    Seeking,
    Prerolling,
    AwaitingFrames,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Loading {
        position: f64,
    },
    Settling {
        target: f64,
        generation: u64,
        step: Step,
        since: f64,
    },
    Paused {
        position: f64,
    },
    Running {
        anchor_host: f64,
        anchor_position: f64,
        rate: f32,
    },
}

/// See the module docs.
#[derive(Debug, Clone, PartialEq)]
pub struct SyncMachine {
    duration: f64,
    rate: f32,
    wants_play: bool,
    generation: u64,
    phase: Phase,
}

impl Default for SyncMachine {
    fn default() -> Self {
        SyncMachine::new()
    }
}

impl SyncMachine {
    /// A machine waiting for its players, positioned at 0, rate 1, paused.
    pub fn new() -> Self {
        SyncMachine {
            duration: f64::INFINITY,
            rate: 1.0,
            wants_play: false,
            generation: 0,
            phase: Phase::Loading { position: 0.0 },
        }
    }

    /// The players are ready and the event lasts `duration` seconds: move them to the remembered
    /// position (and start them if play was requested meanwhile).
    pub fn players_ready(&mut self, duration: f64, now: f64) -> Vec<Action> {
        self.duration = duration.max(0.0);
        let Phase::Loading { position } = self.phase else {
            return Vec::new();
        };
        self.settle(self.clamp(position), now)
    }

    /// Applies a command.
    pub fn command(&mut self, command: Command, now: f64) -> Vec<Action> {
        match command {
            Command::Play => self.play(now),
            Command::Pause => self.pause(now),
            Command::Seek(target) => {
                let target = self.clamp(target);
                match self.phase {
                    Phase::Loading { .. } => {
                        self.phase = Phase::Loading { position: target };
                        Vec::new()
                    }
                    _ => self.settle(target, now),
                }
            }
            Command::SetRate(rate) => {
                self.rate = rate;
                match self.phase {
                    Phase::Running { .. } => {
                        let position = self.position(now);
                        self.settle(position, now)
                    }
                    _ => Vec::new(),
                }
            }
        }
    }

    /// Every player finished the seek tagged `generation` (stale tags are ignored).
    pub fn seek_done(&mut self, generation: u64, now: f64) -> Vec<Action> {
        match self.phase {
            Phase::Settling {
                target,
                generation: current,
                step: Step::Seeking,
                ..
            } if current == generation => self.after_seek(target, generation, now),
            _ => Vec::new(),
        }
    }

    /// Every player finished the preroll tagged `generation` (stale tags are ignored).
    pub fn preroll_done(&mut self, generation: u64, now: f64) -> Vec<Action> {
        if let Phase::Settling {
            target,
            generation: current,
            step: Step::Prerolling,
            ..
        } = self.phase
            && current == generation
        {
            self.phase = Phase::Settling {
                target,
                generation,
                step: Step::AwaitingFrames,
                since: now,
            };
        }
        Vec::new()
    }

    /// Advances time-based transitions. `frames_ready` tells whether every player can present the frame
    /// at [`SyncMachine::awaiting_frames`] (ignored otherwise).
    pub fn tick(&mut self, now: f64, frames_ready: bool) -> (Vec<Action>, Option<Notice>) {
        match self.phase {
            Phase::Settling {
                target,
                generation,
                step,
                since,
            } => {
                let timed_out = now - since >= SETTLE_TIMEOUT_SECONDS;
                let actions = match step {
                    Step::AwaitingFrames if frames_ready || now - since >= FRAME_WAIT_SECONDS => {
                        self.start(target, now)
                    }
                    Step::Seeking if timed_out => self.after_seek(target, generation, now),
                    Step::Prerolling if timed_out => {
                        self.phase = Phase::Settling {
                            target,
                            generation,
                            step: Step::AwaitingFrames,
                            since: now,
                        };
                        Vec::new()
                    }
                    _ => Vec::new(),
                };
                (actions, None)
            }
            Phase::Running { .. } if self.position(now) >= self.duration => {
                self.wants_play = false;
                self.phase = Phase::Paused {
                    position: self.duration,
                };
                (vec![Action::PauseAll], Some(Notice::Ended))
            }
            _ => (Vec::new(), None),
        }
    }

    /// Event time at host time `now`.
    pub fn position(&self, now: f64) -> f64 {
        match self.phase {
            Phase::Loading { position } | Phase::Paused { position } => position,
            Phase::Settling { target, .. } => target,
            Phase::Running {
                anchor_host,
                anchor_position,
                rate,
            } => self.clamp(anchor_position + (now - anchor_host).max(0.0) * f64::from(rate)),
        }
    }

    /// The target frame time while the machine waits for decoded frames before starting.
    pub fn awaiting_frames(&self) -> Option<f64> {
        match self.phase {
            Phase::Settling {
                target,
                step: Step::AwaitingFrames,
                ..
            } => Some(target),
            _ => None,
        }
    }

    /// True while play is requested (the UI shows the pause button), including while settling.
    pub fn wants_play(&self) -> bool {
        self.wants_play
    }

    /// True while every player is running on the shared clock.
    pub fn is_running(&self) -> bool {
        matches!(self.phase, Phase::Running { .. })
    }

    /// True once the players are ready and no seek / start is in flight.
    pub fn is_settled(&self) -> bool {
        matches!(self.phase, Phase::Paused { .. } | Phase::Running { .. })
    }

    /// The requested playback rate.
    pub fn rate(&self) -> f32 {
        self.rate
    }

    fn play(&mut self, now: f64) -> Vec<Action> {
        self.wants_play = true;
        match self.phase {
            Phase::Paused { position } if position >= self.duration => self.settle(0.0, now),
            Phase::Paused { position } => {
                self.generation += 1;
                let generation = self.generation;
                self.phase = Phase::Settling {
                    target: position,
                    generation,
                    step: Step::Prerolling,
                    since: now,
                };
                vec![Action::PrerollAll {
                    rate: self.rate,
                    generation,
                }]
            }
            _ => Vec::new(),
        }
    }

    fn pause(&mut self, now: f64) -> Vec<Action> {
        self.wants_play = false;
        match self.phase {
            Phase::Running { .. } => {
                let position = self.position(now);
                self.settle(position, now)
            }
            Phase::Settling {
                target,
                step: Step::Prerolling | Step::AwaitingFrames,
                ..
            } => {
                self.phase = Phase::Paused { position: target };
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    fn settle(&mut self, target: f64, now: f64) -> Vec<Action> {
        self.generation += 1;
        let generation = self.generation;
        self.phase = Phase::Settling {
            target,
            generation,
            step: Step::Seeking,
            since: now,
        };
        vec![Action::PauseAll, Action::SeekAll { target, generation }]
    }

    fn after_seek(&mut self, target: f64, generation: u64, now: f64) -> Vec<Action> {
        if self.wants_play {
            self.phase = Phase::Settling {
                target,
                generation,
                step: Step::Prerolling,
                since: now,
            };
            vec![Action::PrerollAll {
                rate: self.rate,
                generation,
            }]
        } else {
            self.phase = Phase::Paused { position: target };
            Vec::new()
        }
    }

    fn start(&mut self, position: f64, now: f64) -> Vec<Action> {
        let host_time = now + START_LEAD_SECONDS;
        self.phase = Phase::Running {
            anchor_host: host_time,
            anchor_position: position,
            rate: self.rate,
        };
        vec![Action::StartAll {
            rate: self.rate,
            position,
            host_time,
        }]
    }

    fn clamp(&self, t: f64) -> f64 {
        if t.is_nan() {
            0.0
        } else {
            t.clamp(0.0, self.duration)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A machine that is ready (120 s event) and settled, paused at `position`.
    fn paused_at(position: f64) -> SyncMachine {
        let mut machine = SyncMachine::new();
        machine.command(Command::Seek(position), 0.0);
        let actions = machine.players_ready(120.0, 0.0);
        let Some(Action::SeekAll { generation, .. }) = actions.last().copied() else {
            panic!("{actions:?}")
        };
        assert!(machine.seek_done(generation, 0.0).is_empty());
        machine
    }

    /// Drives a play request through preroll and the frame wait; returns the start action.
    fn run_to_start(machine: &mut SyncMachine, actions: Vec<Action>, now: f64) -> Action {
        let mut actions = actions;
        loop {
            match actions.last().copied() {
                Some(Action::SeekAll { generation, .. }) => {
                    actions = machine.seek_done(generation, now)
                }
                Some(Action::PrerollAll { generation, .. }) => {
                    machine.preroll_done(generation, now);
                    actions = machine.tick(now, true).0;
                }
                Some(start @ Action::StartAll { .. }) => return start,
                other => panic!("unexpected {other:?}"),
            }
        }
    }

    #[test]
    fn commands_before_the_players_are_ready_are_remembered() {
        let mut machine = SyncMachine::new();
        assert!(machine.command(Command::Seek(42.0), 0.0).is_empty());
        assert!(machine.command(Command::Play, 0.0).is_empty());
        assert!(machine.wants_play() && !machine.is_settled());
        assert_eq!(machine.position(0.0), 42.0);
        let actions = machine.players_ready(120.0, 1.0);
        assert_eq!(
            actions,
            [
                Action::PauseAll,
                Action::SeekAll {
                    target: 42.0,
                    generation: 1
                }
            ]
        );
        assert_eq!(
            machine.seek_done(1, 1.0),
            [Action::PrerollAll {
                rate: 1.0,
                generation: 1
            }]
        );
        assert!(machine.preroll_done(1, 1.0).is_empty());
        assert_eq!(machine.awaiting_frames(), Some(42.0));
        assert_eq!(machine.tick(1.1, false).0, []);
        let start = machine.tick(1.2, true).0;
        assert_eq!(
            start,
            [Action::StartAll {
                rate: 1.0,
                position: 42.0,
                host_time: 1.2 + START_LEAD_SECONDS
            }]
        );
        assert!(machine.is_running());
        assert_eq!(machine.position(1.2), 42.0);
        assert!((machine.position(3.25) - 44.0).abs() < 1e-9);
    }

    #[test]
    fn a_ready_machine_without_play_intent_settles_paused() {
        let machine = paused_at(10.0);
        assert!(machine.is_settled() && !machine.is_running() && !machine.wants_play());
        assert_eq!(machine.position(99.0), 10.0);
    }

    #[test]
    fn missing_frames_delay_the_start_for_at_most_the_frame_wait() {
        let mut machine = paused_at(5.0);
        let actions = machine.command(Command::Play, 1.0);
        assert_eq!(
            actions,
            [Action::PrerollAll {
                rate: 1.0,
                generation: 2
            }]
        );
        machine.preroll_done(2, 1.0);
        assert_eq!(machine.tick(1.0 + FRAME_WAIT_SECONDS - 0.01, false).0, []);
        let start = machine.tick(1.0 + FRAME_WAIT_SECONDS + 1e-6, false).0;
        assert!(matches!(
            start.as_slice(),
            [Action::StartAll { position: 5.0, .. }]
        ));
    }

    #[test]
    fn stale_seek_completions_are_ignored() {
        let mut machine = paused_at(0.0);
        machine.command(Command::Play, 0.0);
        assert_eq!(
            machine.command(Command::Seek(10.0), 0.0),
            [
                Action::PauseAll,
                Action::SeekAll {
                    target: 10.0,
                    generation: 3
                }
            ]
        );
        assert_eq!(
            machine.command(Command::Seek(20.0), 0.0),
            [
                Action::PauseAll,
                Action::SeekAll {
                    target: 20.0,
                    generation: 4
                }
            ]
        );
        assert!(machine.seek_done(3, 0.0).is_empty());
        assert!(machine.preroll_done(2, 0.0).is_empty());
        assert_eq!(
            machine.seek_done(4, 0.0),
            [Action::PrerollAll {
                rate: 1.0,
                generation: 4
            }]
        );
        assert_eq!(machine.position(0.0), 20.0);
    }

    #[test]
    fn pausing_while_running_realigns_every_player_on_the_paused_time() {
        let mut machine = paused_at(0.0);
        let actions = machine.command(Command::Play, 0.0);
        run_to_start(&mut machine, actions, 0.0);
        // The start was scheduled START_LEAD_SECONDS ahead, so 2 s of playback end at 2 + lead.
        let actions = machine.command(Command::Pause, 2.0 + START_LEAD_SECONDS);
        let [
            Action::PauseAll,
            Action::SeekAll {
                target,
                generation: 3,
            },
        ] = actions.as_slice()
        else {
            panic!("{actions:?}")
        };
        assert!((target - 2.0).abs() < 1e-9);
        assert!(!machine.wants_play());
        assert!(machine.seek_done(3, 2.1).is_empty());
        assert!(machine.is_settled());
        assert_eq!(machine.position(50.0), *target);
    }

    #[test]
    fn pausing_during_the_preroll_stops_without_starting() {
        let mut machine = paused_at(7.0);
        machine.command(Command::Play, 0.0);
        assert!(machine.command(Command::Pause, 0.0).is_empty());
        assert!(machine.preroll_done(2, 0.0).is_empty());
        assert_eq!(machine.tick(5.0, true), (vec![], None));
        assert_eq!(machine.position(5.0), 7.0);
        assert!(machine.is_settled() && !machine.is_running());
    }

    #[test]
    fn reaching_the_end_pauses_there_and_play_restarts_from_zero() {
        let mut machine = paused_at(119.0);
        let actions = machine.command(Command::Play, 0.0);
        run_to_start(&mut machine, actions, 0.0);
        assert_eq!(machine.tick(0.5, false), (vec![], None));
        assert_eq!(
            machine.tick(1.05, false),
            (vec![Action::PauseAll], Some(Notice::Ended))
        );
        assert_eq!(machine.position(9.0), 120.0);
        assert!(!machine.wants_play());
        assert_eq!(machine.tick(2.0, false), (vec![], None));
        let restart = machine.command(Command::Play, 3.0);
        assert_eq!(
            restart,
            [
                Action::PauseAll,
                Action::SeekAll {
                    target: 0.0,
                    generation: 3
                }
            ]
        );
    }

    #[test]
    fn changing_the_rate_while_running_restarts_in_sync_at_the_new_rate() {
        let mut machine = paused_at(10.0);
        let actions = machine.command(Command::Play, 0.0);
        run_to_start(&mut machine, actions, 0.0);
        let actions = machine.command(Command::SetRate(2.0), 1.05);
        assert_eq!(
            actions,
            [
                Action::PauseAll,
                Action::SeekAll {
                    target: 11.0,
                    generation: 3
                }
            ]
        );
        let start = run_to_start(&mut machine, actions, 1.1);
        assert_eq!(
            start,
            Action::StartAll {
                rate: 2.0,
                position: 11.0,
                host_time: 1.1 + START_LEAD_SECONDS
            }
        );
        assert!((machine.position(2.15) - 13.0).abs() < 1e-9);
    }

    #[test]
    fn changing_the_rate_while_paused_applies_on_the_next_start() {
        let mut machine = paused_at(0.0);
        assert!(machine.command(Command::SetRate(0.5), 0.0).is_empty());
        let actions = machine.command(Command::Play, 0.0);
        assert_eq!(
            actions,
            [Action::PrerollAll {
                rate: 0.5,
                generation: 2
            }]
        );
        assert_eq!(machine.rate(), 0.5);
    }

    #[test]
    fn seeks_are_clamped_into_the_event() {
        let mut machine = paused_at(0.0);
        assert_eq!(
            machine.command(Command::Seek(500.0), 0.0)[1],
            Action::SeekAll {
                target: 120.0,
                generation: 2
            }
        );
        assert_eq!(
            machine.command(Command::Seek(-3.0), 0.0)[1],
            Action::SeekAll {
                target: 0.0,
                generation: 3
            }
        );
        assert_eq!(
            machine.command(Command::Seek(f64::NAN), 0.0)[1],
            Action::SeekAll {
                target: 0.0,
                generation: 4
            }
        );
    }

    #[test]
    fn lost_completions_do_not_wedge_the_machine() {
        let mut machine = paused_at(0.0);
        machine.command(Command::Play, 0.0);
        machine.command(Command::Seek(30.0), 0.0);
        assert_eq!(
            machine.tick(SETTLE_TIMEOUT_SECONDS, false).0,
            [Action::PrerollAll {
                rate: 1.0,
                generation: 3
            }]
        );
        assert_eq!(machine.tick(2.0 * SETTLE_TIMEOUT_SECONDS, false).0, []);
        assert_eq!(machine.awaiting_frames(), Some(30.0));
        let start = machine
            .tick(
                2.0 * SETTLE_TIMEOUT_SECONDS + FRAME_WAIT_SECONDS + 1e-6,
                false,
            )
            .0;
        assert!(matches!(
            start.as_slice(),
            [Action::StartAll { position: 30.0, .. }]
        ));
    }

    #[test]
    fn play_while_running_is_a_no_op() {
        let mut machine = paused_at(0.0);
        let actions = machine.command(Command::Play, 0.0);
        run_to_start(&mut machine, actions, 0.0);
        assert!(machine.command(Command::Play, 1.0).is_empty());
        assert!(machine.is_running());
    }
}
