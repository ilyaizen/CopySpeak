//! Just-in-time paginated synthesis.
//!
//! Paginated readings are billed per submitted fragment. Submitting every
//! fragment up front pays for audio a stopped reading never plays. This module
//! decides when the next fragment must be submitted: early enough that its
//! first audio lands inside the current fragment's tail, late enough that an
//! early stop pays for as little as possible.
//!
//! The decision is a pure function of the webview's reported playback progress
//! and the Rust-side fragment duration, so it is unit tested without a webview
//! or a live TTS provider.

use crate::playback_signal::PlaybackProgress;
use std::time::{Duration, Instant};

/// Smallest head start: the next fragment starts submitting this long before
/// the current fragment ends, so its first audio lands before the tail runs out.
pub(crate) const MIN_LEAD: Duration = Duration::from_secs(3);
/// Room on top of the previous fragment's measured generation cost. The next
/// fragment can be slower than the last one (a longer fragment, provider
/// jitter), and a late batch fragment would leave an audible gap.
pub(crate) const LEAD_MARGIN: Duration = Duration::from_millis(1_200);
/// Cap on the adaptive lead. A larger head start only wastes credits.
pub(crate) const MAX_LEAD: Duration = Duration::from_secs(12);
/// No fresh caption for this long means the webview's clock died; submit the
/// next fragment anyway, exactly as the free-running loop did.
pub(crate) const NO_PROGRESS_TIMEOUT: Duration = Duration::from_secs(5);
/// Hard cap for one wait. A paused reading may sit here a long time; only a
/// pathologically mismatched caption ever reaches this.
pub(crate) const MAX_WAIT: Duration = Duration::from_secs(600);
/// How close a reported fragment duration must be to the Rust-side duration
/// when the webview sends no fragment index.
const DURATION_MATCH_TOLERANCE_MS: u64 = 500;

/// One fragment playback the loop is waiting on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct JitTarget {
    /// Zero-based index of the fragment whose playback the loop waits on.
    pub fragment_index: usize,
    /// Its full duration in milliseconds; 0 when the payload format cannot
    /// report one (a non-WAV batch payload).
    pub duration_ms: u64,
}

/// Whether to submit the next fragment now or keep waiting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum JitDecision {
    Synthesize,
    Wait,
}

impl JitDecision {
    pub(crate) fn synthesize_now(self) -> bool {
        self == Self::Synthesize
    }
}

/// The lead to use before submitting the next fragment: the previous
/// fragment's measured generation cost (first chunk for the streaming path,
/// whole synthesis for the batch path) plus a margin, never below [`MIN_LEAD`].
/// The result is wall-clock time; [`JitWaiter`] converts it to the media time
/// the webview reports.
pub(crate) fn lead_after(generation_cost: Duration) -> Duration {
    (generation_cost + LEAD_MARGIN).clamp(MIN_LEAD, MAX_LEAD)
}

/// One wait for the playback of a specific fragment to approach its end.
#[derive(Debug)]
pub(crate) struct JitWaiter {
    target: JitTarget,
    lead: Duration,
    /// Playback rate the webview renders; media time runs this fast in wall
    /// seconds, so the wall-clock fallback divides by it.
    speed: f64,
    started_at: Instant,
    /// Wall-clock estimate of when the target fragment's playback started.
    anchor: Option<Instant>,
    /// Last observed media position; a frozen position must not re-anchor.
    last_position_ms: Option<u64>,
    /// When the caption first went stale inside this wait.
    clock_lost_since: Option<Instant>,
}

impl JitWaiter {
    pub(crate) fn new(
        target: JitTarget,
        lead: Duration,
        playback_speed: f32,
        now: Instant,
    ) -> Self {
        let speed = if playback_speed.is_finite() && playback_speed > 0.0 {
            f64::from(playback_speed)
        } else {
            1.0
        };
        Self {
            target,
            lead,
            speed,
            started_at: now,
            anchor: None,
            last_position_ms: None,
            clock_lost_since: None,
        }
    }

    /// Feed the latest progress report (`None` when the caption is stale) and
    /// decide whether the next fragment must be submitted now.
    pub(crate) fn observe(
        &mut self,
        progress: Option<PlaybackProgress>,
        now: Instant,
    ) -> JitDecision {
        let Some(progress) = progress else {
            let lost = *self.clock_lost_since.get_or_insert(now);
            return if now.duration_since(lost) >= NO_PROGRESS_TIMEOUT {
                JitDecision::Synthesize
            } else {
                JitDecision::Wait
            };
        };
        self.clock_lost_since = None;

        if !self.matches(&progress) {
            // The playhead already moved past the target fragment: this wait
            // can no longer save a gap, so submit at once.
            if progress
                .fragment_index
                .is_some_and(|index| index > self.target.fragment_index)
            {
                return JitDecision::Synthesize;
            }
            // An earlier fragment is still playing (the normal batch-path
            // backlog). Waiting is bounded by that fragment's own playback.
            return if now.duration_since(self.started_at) >= MAX_WAIT {
                JitDecision::Synthesize
            } else {
                JitDecision::Wait
            };
        }

        // Re-anchor from a moving playhead only; a frozen one (pause, seek
        // rebuild, stalled scheduler) must not drift the estimate.
        if !progress.paused && Some(progress.position_ms) != self.last_position_ms {
            self.last_position_ms = Some(progress.position_ms);
            let played = Duration::from_secs_f64(progress.position_ms as f64 / 1000.0 / self.speed);
            self.anchor = Some(now - played);
        }
        if !progress.paused
            && self.target.duration_ms > 0
            && progress.position_ms.saturating_add(self.lead_media_ms()) >= self.target.duration_ms
        {
            return JitDecision::Synthesize;
        }
        // A playhead that outruns the position reports (a throttled webview
        // clock) still ends near the wall-clock anchor.
        if !progress.paused {
            if let (Some(anchor), true) = (self.anchor, self.target.duration_ms > 0) {
                let wall_duration =
                    Duration::from_secs_f64(self.target.duration_ms as f64 / 1000.0 / self.speed);
                if now + self.lead >= anchor + wall_duration {
                    return JitDecision::Synthesize;
                }
            }
        }
        if now.duration_since(self.started_at) >= MAX_WAIT {
            return JitDecision::Synthesize;
        }
        JitDecision::Wait
    }

    /// The lead in media milliseconds. [`lead_after`] reports a wall-clock
    /// head start, while the webview's position and duration are media time,
    /// which advances `playback_speed` times faster in wall seconds.
    fn lead_media_ms(&self) -> u64 {
        (self.lead.as_secs_f64() * self.speed * 1000.0).round() as u64
    }

    /// Whether a progress report describes the fragment this waiter watches.
    fn matches(&self, progress: &PlaybackProgress) -> bool {
        match progress.fragment_index {
            Some(index) => index == self.target.fragment_index,
            // Without an index, the fragment's duration identifies it. An
            // unknown Rust-side duration accepts any report: the reading must
            // make progress, and a redundant head start only costs credits a
            // free-running loop would have spent anyway.
            None if self.target.duration_ms == 0 => true,
            None => {
                progress.duration_ms.abs_diff(self.target.duration_ms)
                    <= DURATION_MATCH_TOLERANCE_MS
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TARGET_MS: u64 = 30_000;

    fn target() -> JitTarget {
        JitTarget {
            fragment_index: 1,
            duration_ms: TARGET_MS,
        }
    }

    fn progress(index: Option<usize>, position_ms: u64) -> PlaybackProgress {
        PlaybackProgress {
            fragment_index: index,
            position_ms,
            duration_ms: TARGET_MS,
            paused: false,
            active: true,
        }
    }

    fn waiter(now: Instant) -> JitWaiter {
        JitWaiter::new(target(), Duration::from_secs(3), 1.0, now)
    }

    /// The normal streaming case: the current fragment plays and the next one
    /// is submitted only when its playback enters the lead window.
    #[test]
    fn waits_until_the_playhead_is_within_the_lead() {
        let start = Instant::now();
        let mut waiter = waiter(start);
        let early = start + Duration::from_secs(5);
        assert_eq!(
            waiter.observe(Some(progress(Some(1), 5_000)), early),
            JitDecision::Wait
        );
        let just_outside = start + Duration::from_secs(20);
        assert_eq!(
            waiter.observe(Some(progress(Some(1), 26_000)), just_outside),
            JitDecision::Wait
        );
        let inside = start + Duration::from_secs(27);
        assert_eq!(
            waiter.observe(Some(progress(Some(1), 27_500)), inside),
            JitDecision::Synthesize
        );
    }

    /// A caption that describes an earlier fragment (the batch path queues
    /// fragments while the previous one plays) must not start the next one.
    #[test]
    fn an_earlier_fragment_does_not_open_the_window() {
        let start = Instant::now();
        let mut waiter = waiter(start);
        let early = start + Duration::from_secs(2);
        assert_eq!(
            waiter.observe(Some(progress(Some(0), 29_900)), early),
            JitDecision::Wait
        );
    }

    /// A playback that already moved past the target (a missed window) must
    /// submit the next fragment at once instead of stalling the reading.
    #[test]
    fn a_later_fragment_opens_the_window_at_once() {
        let start = Instant::now();
        let mut waiter = waiter(start);
        let now = start + Duration::from_secs(2);
        assert_eq!(
            waiter.observe(Some(progress(Some(2), 1_000)), now),
            JitDecision::Synthesize
        );
    }

    /// Pausing freezes the position: generation must stop, and a resume that
    /// reaches the lead window must continue it.
    #[test]
    fn a_paused_playhead_never_opens_the_window() {
        let start = Instant::now();
        let mut waiter = waiter(start);
        let paused = PlaybackProgress {
            paused: true,
            active: true,
            ..progress(Some(1), 29_500)
        };
        let now = start + Duration::from_secs(10);
        assert_eq!(waiter.observe(Some(paused), now), JitDecision::Wait);
        // A long pause must not be mistaken for a dead caption clock either.
        let later = now + Duration::from_secs(120);
        assert_eq!(waiter.observe(Some(paused), later), JitDecision::Wait);
        // Resuming near the end opens it.
        let resumed = later + Duration::from_millis(120);
        assert_eq!(
            waiter.observe(Some(progress(Some(1), 29_620)), resumed),
            JitDecision::Synthesize
        );
    }

    /// A dead caption clock must not hang the reading: after the grace period
    /// the next fragment is submitted anyway.
    #[test]
    fn a_dead_caption_clock_falls_back_to_submitting() {
        let start = Instant::now();
        let mut waiter = waiter(start);
        let stale_since = start + Duration::from_secs(1);
        assert_eq!(waiter.observe(None, stale_since), JitDecision::Wait);
        assert_eq!(
            waiter.observe(
                None,
                stale_since + NO_PROGRESS_TIMEOUT - Duration::from_millis(1)
            ),
            JitDecision::Wait
        );
        assert_eq!(
            waiter.observe(None, stale_since + NO_PROGRESS_TIMEOUT),
            JitDecision::Synthesize
        );
    }

    /// A stale-caption gap that later recovers must not count toward the dead
    /// clock fallback once fresh reports return.
    #[test]
    fn a_recovered_caption_resets_the_dead_clock() {
        let start = Instant::now();
        let mut waiter = waiter(start);
        assert_eq!(
            waiter.observe(None, start + Duration::from_secs(4)),
            JitDecision::Wait
        );
        assert_eq!(
            waiter.observe(
                Some(progress(Some(1), 1_000)),
                start + Duration::from_secs(4)
            ),
            JitDecision::Wait
        );
        // The clock was alive at 4 s, so the stale window restarts from there.
        assert_eq!(
            waiter.observe(None, start + Duration::from_secs(6)),
            JitDecision::Wait
        );
        assert_eq!(
            waiter.observe(None, start + Duration::from_secs(11)),
            JitDecision::Synthesize
        );
    }

    /// The wall-clock anchor catches a playhead that stopped reporting position
    /// while audio kept playing: the window still opens before the fragment end.
    #[test]
    fn a_throttled_position_still_opens_the_window_from_the_anchor() {
        let start = Instant::now();
        let mut waiter = waiter(start);
        let observed = start + Duration::from_secs(1);
        assert_eq!(
            waiter.observe(Some(progress(Some(1), 1_000)), observed),
            JitDecision::Wait
        );
        // No new positions arrive; the fragment ends ~29 s after its start.
        let late = start + Duration::from_secs(27);
        assert_eq!(
            waiter.observe(Some(progress(Some(1), 1_000)), late),
            JitDecision::Synthesize
        );
    }

    /// A legacy webview that sends no fragment index is matched by duration,
    /// and a playback that is far outside the duration tolerance waits.
    #[test]
    fn duration_identifies_the_fragment_without_an_index() {
        let start = Instant::now();
        let mut waiter = waiter(start);
        let now = start + Duration::from_secs(28);
        assert_eq!(
            waiter.observe(Some(progress(None, 28_000)), now),
            JitDecision::Synthesize
        );

        let mut waiter = JitWaiter::new(target(), Duration::from_secs(3), 1.0, start);
        let other = PlaybackProgress {
            fragment_index: None,
            position_ms: 28_000,
            duration_ms: 12_000,
            paused: false,
            active: true,
        };
        assert_eq!(waiter.observe(Some(other), now), JitDecision::Wait);
    }

    /// At a non-default speed the wall-clock lead converts to media time: the
    /// same 3 s of head start covers 1.3x more media position, because 1 s of
    /// media plays in 1/1.3 s of wall time.
    #[test]
    fn the_lead_converts_to_media_time_at_a_non_default_speed() {
        let start = Instant::now();
        let mut waiter = JitWaiter::new(target(), Duration::from_secs(3), 1.3, start);
        // 3 s wall is 3900 ms media: 26 000 + 3900 is still short of 30 000.
        assert_eq!(
            waiter.observe(
                Some(progress(Some(1), 26_000)),
                start + Duration::from_secs(20)
            ),
            JitDecision::Wait
        );
        assert_eq!(
            waiter.observe(
                Some(progress(Some(1), 26_200)),
                start + Duration::from_secs(20)
            ),
            JitDecision::Synthesize
        );
    }

    /// A slow non-default playback rate stretches the wall clock: the anchor
    /// must not open the window while the media playhead is still early.
    #[test]
    fn the_anchor_scales_with_the_playback_rate() {
        let start = Instant::now();
        let mut waiter = JitWaiter::new(target(), Duration::from_secs(3), 0.5, start);
        assert_eq!(
            waiter.observe(
                Some(progress(Some(1), 1_000)),
                start + Duration::from_secs(2)
            ),
            JitDecision::Wait
        );
        // 1 s of media at half speed started at +0 s; 30 s of media ends at
        // +60 s, so +25 s wall is still outside a 3 s lead.
        assert_eq!(
            waiter.observe(
                Some(progress(Some(1), 1_000)),
                start + Duration::from_secs(25)
            ),
            JitDecision::Wait
        );
        assert_eq!(
            waiter.observe(
                Some(progress(Some(1), 1_000)),
                start + Duration::from_secs(58)
            ),
            JitDecision::Synthesize
        );
    }

    /// The lead grows with the previous fragment's measured cost but stays
    /// inside its bounds.
    #[test]
    fn the_lead_tracks_the_generation_cost() {
        assert_eq!(lead_after(Duration::ZERO), MIN_LEAD);
        // A cost below the floor keeps the floor.
        assert_eq!(lead_after(Duration::from_millis(1_500)), MIN_LEAD);
        assert_eq!(
            lead_after(Duration::from_millis(4_000)),
            Duration::from_millis(5_200)
        );
        assert_eq!(lead_after(Duration::from_secs(30)), MAX_LEAD);
    }
}
