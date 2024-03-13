//! Module containing a tween player that process tweens with time span.
//!
//! # Entity structure
//!
//! If we have this entity:
//!   ```no_run
//!   # use bevy::prelude::*;
//!   # use bevy_tween::prelude::*;
//!   # let world = World::new();
//!   # let mut commands_queue = bevy::ecs::system::CommandQueue::default();
//!   # let mut commands = Commands::new(&mut commands_queue, &world);
//!   let my_entity = commands.spawn(SpriteBundle::default()).id();
//!   ```
//!  
//!   We can create a span tween player with span tween in 2 ways:
//! - Span tween in the same entity as a span tween player.<br/>
//!   This is the case where you might want to make a simple animation where
//!   there's not many parameteres. Because an entity can only have one unique
//!   component, it limits on what animation you can achieve with this.
//!   ```no_run
//!   # use bevy::prelude::*;
//!   # use bevy_tween::prelude::*;
//!   # let world = World::new();
//!   # let mut commands_queue = bevy::ecs::system::CommandQueue::default();
//!   # let mut commands = Commands::new(&mut commands_queue, &world);
//!   # let my_entity = commands.spawn(SpriteBundle::default()).id();
//!   // Spawning some span tween player
//!   commands.spawn((
//!       // The span tween player:
//!       SpanTweenPlayerBundle::new(Duration::from_secs(1)),
//!       // The tween:
//!       // Tween this from the start to the second 1.
//!       SpanTweenBundle::new(..Duration::from_secs(1)),
//!       // Tween this with ease quadratic out.
//!       EaseFunction::QuadraticOut,
//!       // Tween a component.
//!       ComponentTween::new_target(
//!           // Tween the component of this entity
//!           my_entity,
//!           // Tween transform's translation of the entity
//!           interpolate::Translation {
//!               start: Vec3::new(0., 0., 0.),
//!               end: Vec3::new(0., 100., 0.),
//!           }
//!       )
//!   ));
//!   ```
//! - Span tween(s) as a child of a span tween player.<br/>
//!   This is the case where you want to make a more complex animation. By having
//!   span tweens as span tween player's children, you can have any number of
//!   span tween types you wanted .
//!   ```no_run
//!   # use bevy::prelude::*;
//!   # use bevy_tween::prelude::*;
//!   # let world = World::new();
//!   # let mut commands_queue = bevy::ecs::system::CommandQueue::default();
//!   # let mut commands = Commands::new(&mut commands_queue, &world);
//!   # let my_entity = commands.spawn(SpriteBundle::default()).id();
//!   // Spawning some span tween player
//!   commands.spawn(
//!       // The span tween player:
//!       SpanTweenPlayerBundle::new(Duration::from_secs(1)),
//!   ).with_children(|c| {
//!       // The span tween:
//!       c.spawn((
//!           SpanTweenBundle::new(..Duration::from_secs(1)),
//!           EaseFunction::QuadraticOut,
//!           ComponentTween::new_target(
//!               my_entity,
//!               interpolate::Translation {
//!                   start: Vec3::new(0., 0., 0.),
//!                   end: Vec3::new(0., 100., 0.),
//!               }
//!           )
//!       ));
//!      // spawn some more span tween if needed.
//!      // c.spawn( ... );
//!   });
//!   ```
//! - Also the above 2 combined will works just fine btw.

use std::{ops, time::Duration};

use bevy::{ecs::system::EntityCommands, prelude::*};
use tween_timer::Repeat;

use crate::{
    interpolation::Interpolation,
    prelude::EaseFunction,
    tween::{TweenPlayerMarker, TweenState},
    tween_timer::{self, AnimationDirection, TickResult, TweenTimer},
};

/// Plugin for using span tween
#[derive(Debug)]
pub struct SpanTweenPlugin;
impl Plugin for SpanTweenPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            span_tween_player_system.in_set(crate::TweenSystemSet::TweenPlayer),
        )
        .register_type::<SpanTweenPlayer>()
        .register_type::<TimeBound>()
        .register_type::<TweenTimeSpan>();
    }
}

/// Span tween player
#[derive(Debug, Default, Component, Clone, PartialEq, Eq, Hash, Reflect)]
#[reflect(Component)]
pub struct SpanTweenPlayer {
    /// The inner timer
    pub timer: TweenTimer,
}

impl From<TweenTimer> for SpanTweenPlayer {
    fn from(value: TweenTimer) -> Self {
        SpanTweenPlayer { timer: value }
    }
}

/// Bounding enum for [`Duration`] to be exclusivively checked or inclusivively
/// checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum TimeBound {
    /// Inclusively check this duration
    Inclusive(Duration),
    /// Exclusively check this duration
    Exclusive(Duration),
}

impl TimeBound {
    /// Get the inner duration
    pub fn duration(&self) -> Duration {
        match self {
            TimeBound::Inclusive(d) | TimeBound::Exclusive(d) => *d,
        }
    }
}

impl Default for TimeBound {
    fn default() -> Self {
        TimeBound::Inclusive(Duration::ZERO)
    }
}

#[derive(Debug, Clone, Copy)]
enum DurationQuotient {
    Before,
    Inside,
    After,
}

/// Error type for when creating a new [`TweenTimeSpan`].
#[derive(Debug)]
pub enum NewTweenTimeSpanError {
    /// The provided min, max will result in a [`TweenTimeSpan`] that does not
    /// appear on a timeline
    NotTime {
        #[allow(missing_docs)]
        min: TimeBound,
        #[allow(missing_docs)]
        max: TimeBound,
    },
    /// The provided min is greater than max and it's not allowed.
    MinGreaterThanMax {
        #[allow(missing_docs)]
        min: TimeBound,
        #[allow(missing_docs)]
        max: TimeBound,
    },
}

impl std::error::Error for NewTweenTimeSpanError {}
impl std::fmt::Display for NewTweenTimeSpanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NewTweenTimeSpanError::NotTime { min, max } => {
                write!(f, "This span does not contain any time: min {min:?} max {max:?}")
            }
            NewTweenTimeSpanError::MinGreaterThanMax { min, max } => {
                write!(f, "This span has min greater than max: min {min:?} max {max:?}")
            }
        }
    }
}

/// Define the range of time for a span tween that will be interpolating for.
#[derive(Debug, Component, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
#[reflect(Component)]
pub struct TweenTimeSpan {
    /// Minimum time for the span tween.
    min: TimeBound,
    /// Maximum time for the span tween.
    max: TimeBound,
}
impl TweenTimeSpan {
    /// Create a new [`TweenTimeSpan`] unchecked for invalid min, max.
    pub(crate) fn new_unchecked(
        min: TimeBound,
        max: TimeBound,
    ) -> TweenTimeSpan {
        TweenTimeSpan { min, max }
    }

    /// Create a new [`TweenTimeSpan`]
    pub fn new(
        min: TimeBound,
        max: TimeBound,
    ) -> Result<TweenTimeSpan, NewTweenTimeSpanError> {
        if matches!(
            (min, max),
            (TimeBound::Exclusive(_), TimeBound::Exclusive(_))
        ) && min.duration() == max.duration()
        {
            return Err(NewTweenTimeSpanError::NotTime { min, max });
        } else if min.duration() > max.duration() {
            return Err(NewTweenTimeSpanError::MinGreaterThanMax { min, max });
        }
        Ok(Self::new_unchecked(min, max))
    }

    fn quotient(&self, duration: Duration) -> DurationQuotient {
        let after_min = match self.min {
            TimeBound::Inclusive(min) => duration >= min,
            TimeBound::Exclusive(min) => duration > min,
        };
        let before_max = match self.max {
            TimeBound::Inclusive(max) => duration <= max,
            TimeBound::Exclusive(max) => duration < max,
        };
        match (after_min, before_max) {
            (true, true) => DurationQuotient::Inside,
            (true, false) => DurationQuotient::After,
            (false, true) => DurationQuotient::Before,
            (false, false) => unreachable!(),
        }
    }

    /// Get the min time
    pub fn min(&self) -> TimeBound {
        self.min
    }

    /// Get the max time
    pub fn max(&self) -> TimeBound {
        self.max
    }
}

impl Default for TweenTimeSpan {
    fn default() -> Self {
        TweenTimeSpan::try_from(Duration::ZERO..Duration::ZERO).unwrap()
    }
}

impl TryFrom<ops::Range<Duration>> for TweenTimeSpan {
    type Error = NewTweenTimeSpanError;

    fn try_from(range: ops::Range<Duration>) -> Result<Self, Self::Error> {
        TweenTimeSpan::new(
            TimeBound::Inclusive(range.start),
            TimeBound::Exclusive(range.end),
        )
    }
}
impl TryFrom<ops::RangeInclusive<Duration>> for TweenTimeSpan {
    type Error = NewTweenTimeSpanError;

    fn try_from(
        range: ops::RangeInclusive<Duration>,
    ) -> Result<Self, Self::Error> {
        TweenTimeSpan::new(
            TimeBound::Inclusive(*range.start()),
            TimeBound::Inclusive(*range.end()),
        )
    }
}

impl TryFrom<ops::RangeTo<Duration>> for TweenTimeSpan {
    type Error = NewTweenTimeSpanError;

    fn try_from(range: ops::RangeTo<Duration>) -> Result<Self, Self::Error> {
        TweenTimeSpan::new(
            TimeBound::Inclusive(Duration::ZERO),
            TimeBound::Exclusive(range.end),
        )
    }
}

impl TryFrom<ops::RangeToInclusive<Duration>> for TweenTimeSpan {
    type Error = NewTweenTimeSpanError;

    fn try_from(
        range: ops::RangeToInclusive<Duration>,
    ) -> Result<Self, Self::Error> {
        TweenTimeSpan::new(
            TimeBound::Inclusive(Duration::ZERO),
            TimeBound::Inclusive(range.end),
        )
    }
}

/// Bundle for a span tween player
#[derive(Default, Bundle)]
pub struct SpanTweenPlayerBundle {
    /// [`SpanTweenPlayer`] span player intestine
    pub span_player: SpanTweenPlayer,
    /// [`TweenTimer`] marker to declare a tween player
    pub tween_player_marker: TweenPlayerMarker,
}

impl SpanTweenPlayerBundle {
    /// Create new [`SpanTweenPlayerBundle`] with `duration`
    pub fn new(duration: Duration) -> Self {
        let mut t = SpanTweenPlayerBundle::default();
        t.span_player.timer.set_duration(duration);
        t
    }

    /// [`SpanTweenPlayerBundle`] with the specified `paused` for the inner
    /// [`TweenTimer`]
    pub fn with_paused(mut self, paused: bool) -> Self {
        self.span_player.timer.set_paused(paused);
        self
    }

    // pub fn with_elasped(mut self, elasped: Duration) -> Self {
    //     self.tween_player.set_elasped(elasped);
    //     self
    // }
    /// [`SpanTweenPlayerBundle`] with the specified `direction` for the inner
    /// [`TweenTimer`]
    pub fn with_direction(mut self, direction: AnimationDirection) -> Self {
        self.span_player.timer.set_direction(direction);
        self
    }

    /// [`SpanTweenPlayerBundle`] with the specified `repeat`
    /// setting the inner [`TweenTimer`]'s repeat to Some
    pub fn with_repeat(mut self, repeat: tween_timer::Repeat) -> Self {
        self.span_player.timer.set_repeat(Some(repeat));
        self
    }

    /// [`SpanTweenPlayerBundle`] with the specified `repeat_style`
    /// setting the inner [`TweenTimer`]'s repeat_style to Some
    pub fn with_repeat_style(
        mut self,
        repeat_style: tween_timer::RepeatStyle,
    ) -> Self {
        self.span_player.timer.set_repeat_style(Some(repeat_style));
        self
    }

    /// [`SpanTweenPlayerBundle`] with without repeat,
    /// setting the inner [`TweenTimer`]'s repeat to None.
    pub fn without_repeat(mut self) -> Self {
        self.span_player.timer.set_repeat(None);
        self
    }

    /// [`SpanTweenPlayerBundle`] with without repeat_style
    /// setting the inner [`TweenTimer`]'s repeat_style to None.
    pub fn without_repeat_style(mut self) -> Self {
        self.span_player.timer.set_repeat_style(None);
        self
    }
}

impl From<TweenTimer> for SpanTweenPlayerBundle {
    fn from(value: TweenTimer) -> Self {
        SpanTweenPlayerBundle {
            span_player: SpanTweenPlayer { timer: value },
            tween_player_marker: TweenPlayerMarker,
        }
    }
}

/// Bundle for a span tween
#[derive(Default, Bundle)]
pub struct SpanTweenBundle {
    /// [`TweenTimeSpan`] to define the range of time this span tween will work for.
    pub span: TweenTimeSpan,
    /// [`TweenState`] required to work as a span tween
    pub state: TweenState,
}

impl SpanTweenBundle {
    /// Create a new [`SpanTweenBundle`] from this `span`
    pub fn new<S>(span: S) -> Self
    where
        S: TryInto<TweenTimeSpan>,
        S::Error: std::fmt::Debug,
    {
        SpanTweenBundle {
            span: span.try_into().expect("valid span"),
            state: Default::default(),
        }
    }
}

/// Fired when a span tween player repeated or completed
#[derive(Debug, Clone, PartialEq, Eq, Hash, Event, Reflect)]
pub struct SpanTweenPlayerEnded {
    /// Tween timer that just ended
    pub tween_player: Entity,
    /// Currently timer direction. If is [`RepeatStyle::PingPong`], the current
    /// direction will be its already changed direction.
    pub current_direction: AnimationDirection,
    /// The repeat this tween player had.
    pub with_repeat: Option<Repeat>,
}

impl SpanTweenPlayerEnded {
    /// Returns true if the player's timer is all done.
    /// All done meaning that there will be nore more ticking and all
    /// configured repeat is exhausted.
    pub fn is_all_done(&self) -> bool {
        self.with_repeat
            .map(|repeat| repeat.exhausted())
            .unwrap_or(true)
    }
}

/// System for updating any span tweens to the correct [`TweenState`] as playing
/// by its span tween player
pub fn span_tween_player_system(
    time: Res<Time<Real>>,
    q_other_tween_player: Query<(), With<SpanTweenPlayer>>,
    mut q_tween_span_player: Query<(
        Entity,
        &mut SpanTweenPlayer,
        Option<&Children>,
    )>,
    mut q_tween: Query<(&mut TweenState, &TweenTimeSpan)>,
    mut ended_writer: EventWriter<SpanTweenPlayerEnded>,
) {
    use AnimationDirection::*;
    use DurationQuotient::*;

    use crate::tween_timer::RepeatStyle::*;

    let delta = time.delta();
    q_tween_span_player.iter_mut().for_each(
        |(player_entity, mut player, children)| {
            let timer = &mut player.timer;
            if timer.paused {
                return;
            }

            if timer.is_all_done() {
                return;
            }

            let delta = Duration::from_secs_f32(
                delta.as_secs_f32() * timer.speed_scale.as_secs_f32(),
            );

            let tick_result = timer.tick(delta, timer.direction);

            match tick_result {
                TickResult::AllDone | TickResult::Repeated => {
                    ended_writer.send(SpanTweenPlayerEnded {
                        tween_player: player_entity,
                        current_direction: timer.direction,
                        with_repeat: timer.repeat,
                    });
                }
                TickResult::Continue => {}
            }

            let children = children
                .iter()
                .flat_map(|a| a.iter())
                .filter(|c| !q_other_tween_player.contains(**c));
            let tweens = [&player_entity].into_iter().chain(children);
            for &tween_entity in tweens {
                let Ok((mut tween_state, tween_span)) =
                    q_tween.get_mut(tween_entity)
                else {
                    continue;
                };

                let elasped_quotient = tween_span.quotient(timer.elasped().now);
                let previous_quotient =
                    tween_span.quotient(timer.elasped().previous);

                let tween_min = Duration::ZERO;
                let tween_max =
                    tween_span.max().duration() - tween_span.min().duration();
                let tween_elasped = timer
                    .elasped()
                    .now
                    .saturating_sub(tween_span.min().duration())
                    .min(tween_max);
                // Look at this behemoth of edge case handling.
                //
                // The edge cases are the time when the tween are really short
                // or delta is really long per frame.
                //
                // This is likely only an issue with this player implementation.
                //
                // This is not accounted for when the tween might repeat
                // multiple time in one frame. When that tween is this ridiculously
                // fast or the game heavily lagged, I don't think that need to
                // be accounted.
                let new_tween_elasped = match (
                    timer.direction,
                    previous_quotient,
                    elasped_quotient,
                    timer.elasped().repeat_style,
                ) {
                    (_, Inside, Inside, None) => Some(tween_elasped),
                    // -------------------------------------------------------
                    | (Forward, Before, Inside, None)
                    | (Forward, Inside, After, None)
                    | (Forward, Before, After, None)
                        => Some(tween_elasped),

                    // -------------------------------------------------------
                    | (Backward, After, Inside, None)
                    | (Backward, Inside, Before, None)
                    | (Backward, After, Before, None)
                        => Some(tween_elasped),

                    // --------------------------------------------------------
                    // don't remove these comments, may use for debugging in the future
                    | (Forward, Before, Before, Some(WrapAround)) // 1&2 max
                    | (Forward, Inside, Before, Some(WrapAround)) // 1 max
                        => Some(tween_max),
                    | (Forward, Before, Inside, Some(WrapAround)) // 2 now
                    | (Forward, Before, After, Some(WrapAround)) // 2 now, max
                    | (Forward, Inside, Inside, Some(WrapAround)) // 1&2 now
                    | (Forward, Inside, After, Some(WrapAround)) // 2 now, max
                    | (Forward, After, Inside, Some(WrapAround)) // 1 now 
                    | (Forward, After, After, Some(WrapAround)) // 1&2 now, max
                    // | (Forward, After, Before, Some(WrapAround)) // 1
                        => Some(tween_elasped),

                    // -------------------------------------------------------
                    | (Backward, After, After, Some(WrapAround)) // 1&2 min
                    | (Backward, Inside, After, Some(WrapAround)) // 1 min
                        => Some(tween_min),
                    | (Backward, Before, Before, Some(WrapAround)) // 1&2 now, min
                    | (Backward, Before, Inside, Some(WrapAround)) // 1 now 
                    | (Backward, Inside, Before, Some(WrapAround)) // 2 now, min
                    | (Backward, Inside, Inside, Some(WrapAround)) // 1&2 now
                    | (Backward, After, Before, Some(WrapAround)) // 2 now, min
                    | (Backward, After, Inside, Some(WrapAround)) // 2 now
                    // | (Backward, Before, After, Some(WrapAround)) // 1
                        => Some(tween_elasped),

                    // -------------------------------------------------------
                    | (Backward, Before, Before, Some(PingPong)) // 1&2 now, min
                    | (Backward, Before, Inside, Some(PingPong)) // 1 now
                    | (Backward, Before, After, Some(PingPong)) // 1 now, max
                    | (Backward, Inside, Before, Some(PingPong)) // 2 now, min
                    | (Backward, Inside, Inside, Some(PingPong)) // 1&2 now
                    | (Backward, Inside, After, Some(PingPong)) // 1 now, max
                    | (Backward, After, Before, Some(PingPong)) // 2 now, min
                    | (Backward, After, Inside, Some(PingPong)) // 2 now
                    // | (Backward, After, After, Some(PingPong)) // 1&2
                        => Some(tween_elasped),

                    // -------------------------------------------------------
                    // | (Forward, Before, Before, Some(PingPong)) // 1&2
                    | (Forward, Before, Inside, Some(PingPong)) // 2 now
                    | (Forward, Before, After, Some(PingPong)) // 2 now, max
                    | (Forward, Inside, Before, Some(PingPong)) // 1 now, min
                    | (Forward, Inside, Inside, Some(PingPong)) // 1&2 now
                    | (Forward, Inside, After, Some(PingPong)) // 2 now, max
                    | (Forward, After, Before, Some(PingPong)) // 1 now, min
                    | (Forward, After, Inside, Some(PingPong)) // 1 now
                    | (Forward, After, After, Some(PingPong)) // 1&2 now, max
                        => Some(tween_elasped),
                    _ => None,
                };
                let new_tween_state = TweenState {
                    local_elasped: new_tween_elasped,
                    local_previous_elasped: tween_state.local_elasped,
                    local_end: tween_max,
                    direction: timer.direction,
                };
                *tween_state = new_tween_state;
            }
        },
    );
}

/// Helper trait for [`SpanTweensBuilder`].
pub trait BuildSpanTweens<'a> {
    /// Create a [`SpanTweensBuilder`].
    fn build_tweens(&mut self) -> SpanTweensBuilder<'a, '_>;
}

impl<'a> BuildSpanTweens<'a> for ChildBuilder<'a> {
    /// Create a [`SpanTweensBuilder`] using a [`ChildBuilder`] that's usually
    /// returned by [`BuildChildren::with_children`].
    fn build_tweens(&mut self) -> SpanTweensBuilder<'a, '_> {
        SpanTweensBuilder {
            child_builder: self,
        }
    }
}

/// Helper struct to build big complex tweens children with less boilerplate.
pub struct SpanTweensBuilder<'a, 'b> {
    child_builder: &'b mut ChildBuilder<'a>,
}

impl<'a, 'b> SpanTweensBuilder<'a, 'b> {
    /// Create a new span tween.
    pub fn tween<S, I, T>(
        &mut self,
        span: S,
        interpolation: I,
        tween: T,
    ) -> &mut Self
    where
        S: TryInto<TweenTimeSpan>,
        S::Error: std::fmt::Debug,
        I: Component + Interpolation,
        T: Bundle,
    {
        self.tween_and(span, interpolation, tween, |_| {})
    }

    /// Create a new span tween then call a closure with the tween's
    /// [`EntityCommands`].
    pub fn tween_and<S, I, T, F>(
        &mut self,
        span: S,
        interpolation: I,
        bundle: T,
        f: F,
    ) -> &mut Self
    where
        S: TryInto<TweenTimeSpan>,
        S::Error: std::fmt::Debug,
        I: Component + Interpolation,
        T: Bundle,
        F: FnOnce(EntityCommands),
    {
        let commands = self.child_builder.spawn((
            SpanTweenBundle::new(span),
            interpolation,
            bundle,
        ));
        f(commands);
        self
    }

    /// Create a new span tween that's 0 seconds in duration which basically
    /// not tween anything but change the value instantly at some input time
    /// then call a closure with the tween's [`EntityCommands`].
    pub fn jump_and<T, F>(&mut self, at: Duration, bundle: T, f: F) -> &mut Self
    where
        T: Bundle,
        F: FnOnce(EntityCommands),
    {
        self.tween_and(at..=at, EaseFunction::Linear, bundle, f)
    }

    /// Create a new span tween that's 0 seconds in duration which basically
    /// not tween anything but change the value instantly at some input time.
    pub fn jump<T>(&mut self, at: Duration, bundle: T) -> &mut Self
    where
        T: Bundle,
    {
        self.tween_and(at..=at, EaseFunction::Linear, bundle, |_| {})
    }
}
