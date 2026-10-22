//! Which decoded source frame each output frame shows. Pure logic.

use super::plan::TIME_EPSILON;

/// Follows one camera's decoded frames along the output frames. For output time `t` it shows the
/// latest frame presented at or before `t` (what `AVVideoComposition` asks a track for), so a source
/// at 36 fps skips some frames at 30 fps, a held frame stays until the next one, and the last frame
/// stays after the source ends.
pub(crate) struct FrameCursor<T> {
    current: Option<T>,
    /// The first frame after `current`, already pulled.
    next: Option<(f64, T)>,
    ended: bool,
}

impl<T> FrameCursor<T> {
    pub fn new() -> FrameCursor<T> {
        FrameCursor {
            current: None,
            next: None,
            ended: false,
        }
    }

    /// Advances to output time `t` (event seconds, never decreasing) and returns the frame to show.
    /// `pull` yields the next frame with its presentation time in presentation order, or `None` at
    /// the end; it is called at most until one frame past `t` is known.
    pub fn advance<E>(
        &mut self,
        t: f64,
        mut pull: impl FnMut() -> Result<Option<(f64, T)>, E>,
    ) -> Result<Option<&T>, E> {
        loop {
            if self.next.is_none() && !self.ended {
                self.next = pull()?;
                self.ended = self.next.is_none();
            }
            match self.next.take() {
                Some((time, frame)) if time <= t + TIME_EPSILON => self.current = Some(frame),
                next => {
                    self.next = next;
                    return Ok(self.current.as_ref());
                }
            }
        }
    }
}

/// Index of the clip a frame presented at `time` comes from: the last one starting at or before it
/// (the first one when none does). `starts` are the clips' event start times in order.
pub(crate) fn clip_at(starts: &[f64], time: f64) -> usize {
    starts
        .iter()
        .rposition(|start| *start <= time + TIME_EPSILON)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::Infallible;

    /// Frames `0..count` at `fps`, starting at `start`, counting how often they are pulled.
    struct Source {
        start: f64,
        fps: f64,
        count: u32,
        next: u32,
        calls: u32,
    }

    impl Source {
        fn new(start: f64, fps: f64, count: u32) -> Source {
            Source {
                start,
                fps,
                count,
                next: 0,
                calls: 0,
            }
        }

        fn pull(&mut self) -> Result<Option<(f64, u32)>, Infallible> {
            self.calls += 1;
            let n = self.next;
            self.next += 1;
            Ok((n < self.count).then(|| (self.start + f64::from(n) / self.fps, n)))
        }
    }

    fn shown(cursor: &mut FrameCursor<u32>, t: f64, source: &mut Source) -> Option<u32> {
        cursor.advance(t, || source.pull()).unwrap().copied()
    }

    #[test]
    fn each_output_frame_shows_the_latest_source_frame_at_or_before_it() {
        let mut source = Source::new(0.0, 36.0, 72);
        let mut cursor = FrameCursor::new();
        for i in 0..60u32 {
            let t = f64::from(i) / 30.0;
            let due = (f64::from(i) * 36.0 / 30.0 + 1e-6).floor() as u32;
            assert_eq!(
                shown(&mut cursor, t, &mut source),
                Some(due),
                "output frame {i}"
            );
        }
    }

    #[test]
    fn nothing_is_shown_before_the_first_frame() {
        let mut source = Source::new(1.0, 36.0, 10);
        let mut cursor = FrameCursor::new();
        assert_eq!(shown(&mut cursor, 0.0, &mut source), None);
        assert_eq!(shown(&mut cursor, 0.99, &mut source), None);
        assert_eq!(shown(&mut cursor, 1.0, &mut source), Some(0));
    }

    #[test]
    fn rounding_noise_at_a_frame_time_still_shows_that_frame() {
        let mut source = Source::new(0.0, 36.0, 72);
        let mut cursor = FrameCursor::new();
        // 0.5 s is frame 18 at 36 fps and frame 15 at 30 fps.
        assert_eq!(shown(&mut cursor, 0.5 - 1e-9, &mut source), Some(18));
    }

    #[test]
    fn a_held_frame_stays_until_the_next_one_and_the_last_one_after_the_end() {
        let frames = [(0.0, 0), (5.0, 1)];
        let mut next = frames.iter().copied();
        let calls = std::cell::Cell::new(0);
        let mut pull = || {
            calls.set(calls.get() + 1);
            Ok::<_, Infallible>(next.next())
        };
        let mut cursor = FrameCursor::new();
        assert_eq!(cursor.advance(4.99, &mut pull).unwrap(), Some(&0));
        assert_eq!(cursor.advance(5.0, &mut pull).unwrap(), Some(&1));
        assert_eq!(cursor.advance(60.0, &mut pull).unwrap(), Some(&1));
        let after_end = calls.get();
        assert_eq!(cursor.advance(61.0, &mut pull).unwrap(), Some(&1));
        assert_eq!(
            calls.get(),
            after_end,
            "an ended source is not pulled again"
        );
    }

    #[test]
    fn frames_are_pulled_at_most_one_ahead() {
        let mut source = Source::new(0.0, 36.0, 72);
        let mut cursor = FrameCursor::new();
        shown(&mut cursor, 0.0, &mut source);
        assert_eq!(source.calls, 2, "the shown frame and the one after it");
        shown(&mut cursor, 1.0 / 30.0, &mut source);
        assert_eq!(source.calls, 3);
    }

    #[test]
    fn a_frame_uses_the_clip_that_starts_at_or_before_it() {
        let starts = [0.0, 60.0, 120.0];
        assert_eq!(clip_at(&starts, 0.0), 0);
        assert_eq!(clip_at(&starts, 59.97), 0);
        assert_eq!(
            clip_at(&starts, 60.0 - 1e-9),
            1,
            "rounding noise at a start"
        );
        assert_eq!(clip_at(&starts, 60.0), 1);
        assert_eq!(clip_at(&starts, 200.0), 2);
        assert_eq!(
            clip_at(&[5.0], 0.0),
            0,
            "before the first clip: the first one"
        );
    }

    #[test]
    fn source_errors_are_passed_on() {
        let mut cursor = FrameCursor::<u32>::new();
        assert_eq!(
            cursor.advance(0.0, || Err("decode failed")),
            Err("decode failed")
        );
    }
}
