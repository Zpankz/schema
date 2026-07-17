//! Append-only Timeline: the immutable ground-truth record of real
//! transitions.
//!
//! Publication observable ("The outer loop", stage 4): "Every real
//! transition is appended to the Timeline. This record persists unchanged;
//! only hypotheses and notes can be revised."
//!
//! In the Python reference immutability needed runtime seals; in Rust it is
//! structural: the backing `Vec` is private, `Transition` fields are
//! reachable only by value or shared reference, and no method returns
//! `&mut` into history. There is nothing to guard at runtime because the
//! borrow checker refuses the mutation at compile time.

pub type Grid = Vec<Vec<u8>>;

/// One real environment transition. No public constructor: transitions are
/// minted only by [`Timeline::append`], so every instance corresponds to a
/// recorded interaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transition {
    pub index: usize,
    pub state_before: Grid,
    pub action: String,
    pub state_after: Grid,
    pub level_up: bool,
    pub dead: bool,
    pub win: bool,
}

impl Transition {
    pub fn terminal(&self) -> bool {
        self.level_up || self.dead || self.win
    }
}

/// Append-only sequence of [`Transition`]s.
#[derive(Debug, Default)]
pub struct Timeline {
    records: Vec<Transition>,
}

impl Timeline {
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn append(
        &mut self,
        state_before: Grid,
        action: &str,
        state_after: Grid,
        level_up: bool,
        dead: bool,
        win: bool,
    ) -> &Transition {
        self.records.push(Transition {
            index: self.records.len(),
            state_before,
            action: action.to_string(),
            state_after,
            level_up,
            dead,
            win,
        });
        self.records.last().expect("just pushed")
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Transition> {
        self.records.iter()
    }

    pub fn get(&self, i: usize) -> Option<&Transition> {
        self.records.get(i)
    }

    pub fn last(&self) -> Option<&Transition> {
        self.records.last()
    }
}

impl<'a> IntoIterator for &'a Timeline {
    type Item = &'a Transition;
    type IntoIter = std::slice::Iter<'a, Transition>;
    fn into_iter(self) -> Self::IntoIter {
        self.records.iter()
    }
}
