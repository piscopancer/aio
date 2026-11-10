use std::time::Duration;

use rand::Rng;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use specta::Type;

use super::policy::{self, Previous};
use super::{PlaybackContext, PlaybackError, RepeatMode};

/// Identity of one queue slot, so two copies of a track and the shuffle round trip stay distinguishable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct QueueKey(pub u32);

#[derive(Debug, Clone)]
pub struct QueueEntry<T> {
  key: QueueKey,
  item: T,
}

impl<T> QueueEntry<T> {
  pub fn key(&self) -> QueueKey {
    self.key
  }

  pub fn item(&self) -> &T {
    &self.item
  }
}

/// Why the queue moves forward, which matters for repeat-one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Advance {
  TrackEnded,
  Skipped,
}

/// Result of removing an entry, telling the caller whether the playing one went away.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Removal {
  Other,
  /// The current entry was removed; the queue now points at the one that followed it, if any.
  Current {
    has_next: bool,
  },
}

/// What plays after the current entry when it ends on its own.
#[derive(Debug, Clone, Copy)]
pub enum Following<'a, T> {
  /// Nothing, as the queue ends there.
  End,
  /// An entry that is not known yet, since a shuffled queue is mixed again when it wraps around.
  Unknown,
  Entry(&'a QueueEntry<T>),
}

/// What of a queue outlasts the app: where it was started from, its items in play order, the original order of a
/// shuffled queue as places among them, and the current place.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueSnapshot<T> {
  pub context: Option<PlaybackContext>,
  pub items: Vec<T>,
  /// While shuffle is on, the places in `items` in the order the queue had before it was shuffled.
  pub unshuffled: Option<Vec<usize>>,
  pub current: Option<usize>,
}

impl<T> QueueSnapshot<T> {
  /// The snapshot with each item turned into what `resolve` makes of it, leaving out the items it finds no more, and
  /// whether the current item is still there. When it is not, the item that followed it becomes current, or else the
  /// first one, as when the current entry is removed.
  pub fn resolve<U>(self, mut resolve: impl FnMut(T) -> Option<U>) -> (QueueSnapshot<U>, bool) {
    let mut places = Vec::with_capacity(self.items.len());
    let mut items = Vec::with_capacity(self.items.len());
    for item in self.items {
      match resolve(item) {
        Some(item) => {
          places.push(Some(items.len()));
          items.push(item);
        }
        None => places.push(None),
      }
    }
    let kept = self.current.and_then(|current| places.get(current).copied().flatten());
    let current = kept.or_else(|| {
      let after = self.current?;
      let next = places.iter().skip(after).find_map(|place| *place);
      next.or_else(|| (!items.is_empty()).then_some(0))
    });
    let unshuffled = self.unshuffled.map(|order| {
      order
        .iter()
        .filter_map(|&place| places.get(place).copied().flatten())
        .collect()
    });
    let snapshot = QueueSnapshot {
      context: self.context,
      items,
      unshuffled,
      current,
    };
    (snapshot, kept.is_some())
  }
}

/// Whether `order` names every one of `len` places exactly once.
fn whole_order(order: &[usize], len: usize) -> bool {
  let mut seen = vec![false; len];
  order.len() == len
    && order
      .iter()
      .all(|&place| place < len && !std::mem::replace(&mut seen[place], true))
}

/// Aggregate root of the play queue: the entries, which one is current, its context, shuffle and repeat.
///
/// Invariants: `current` always indexes into `entries`; `unshuffled` holds the original order exactly while
/// shuffle is on and contains the same entries as `entries`.
#[derive(Debug, Clone)]
pub struct Queue<T> {
  context: Option<PlaybackContext>,
  entries: Vec<QueueEntry<T>>,
  unshuffled: Option<Vec<QueueEntry<T>>>,
  current: Option<usize>,
  shuffle: bool,
  repeat: RepeatMode,
  last_key: u32,
}

impl<T: Clone> Queue<T> {
  pub fn new(shuffle: bool, repeat: RepeatMode) -> Self {
    Self {
      context: None,
      entries: Vec::new(),
      unshuffled: None,
      current: None,
      shuffle,
      repeat,
      last_key: 0,
    }
  }

  pub fn context(&self) -> Option<PlaybackContext> {
    self.context
  }

  pub fn entries(&self) -> &[QueueEntry<T>] {
    &self.entries
  }

  pub fn current_index(&self) -> Option<usize> {
    self.current
  }

  pub fn current(&self) -> Option<&QueueEntry<T>> {
    self.current.and_then(|index| self.entries.get(index))
  }

  pub fn shuffle(&self) -> bool {
    self.shuffle
  }

  pub fn repeat(&self) -> RepeatMode {
    self.repeat
  }

  /// Swaps in the newer version `fresh` gives of an item, keeping every entry's key and place.
  pub fn refresh(&mut self, fresh: impl Fn(&T) -> Option<T>) {
    let original = self.unshuffled.iter_mut().flatten();
    for entry in self.entries.iter_mut().chain(original) {
      if let Some(item) = fresh(&entry.item) {
        entry.item = item;
      }
    }
  }

  /// The queue as it is saved between runs, each item named by what `name` makes of it.
  pub fn snapshot<U>(&self, name: impl Fn(&T) -> U) -> QueueSnapshot<U> {
    QueueSnapshot {
      context: self.context,
      items: self.entries.iter().map(|entry| name(&entry.item)).collect(),
      unshuffled: self.unshuffled.as_ref().map(|original| {
        original
          .iter()
          .filter_map(|entry| self.position(entry.key).ok())
          .collect()
      }),
      current: self.current,
    }
  }

  /// Takes up a saved queue in place of the one it has, under the shuffle and repeat it has now: a shuffled queue is
  /// put back in order when shuffle has since gone off, and an unshuffled one is taken as its own original order when
  /// it has come on. An original order that does not hold every item exactly once is taken to be the play order.
  pub fn restore(&mut self, snapshot: QueueSnapshot<T>) {
    if snapshot.items.is_empty() {
      return;
    }
    let entries: Vec<QueueEntry<T>> = snapshot.items.into_iter().map(|item| self.entry(item)).collect();
    let original = snapshot
      .unshuffled
      .filter(|order| whole_order(order, entries.len()))
      .map(|order| order.iter().map(|&place| entries[place].clone()).collect());
    self.context = snapshot.context;
    self.current = Some(snapshot.current.unwrap_or(0).min(entries.len() - 1));
    self.unshuffled = Some(original.unwrap_or_else(|| entries.clone()));
    self.entries = entries;
    if !self.shuffle {
      self.unshuffle();
    }
  }

  /// Replaces the queue with `items` from `context`, making the one at `start` current (first when shuffled).
  pub fn start(
    &mut self,
    context: PlaybackContext,
    items: Vec<T>,
    start: usize,
    rng: &mut impl Rng,
  ) -> Result<(), PlaybackError> {
    if items.is_empty() {
      return Err(PlaybackError::NothingToPlay);
    }
    let start = start.min(items.len() - 1);
    let entries = items.into_iter().map(|item| self.entry(item)).collect();
    self.context = Some(context);
    self.entries = entries;
    self.unshuffled = None;
    self.current = Some(start);
    if self.shuffle {
      self.shuffle_entries(rng);
    }
    Ok(())
  }

  pub fn jump(&mut self, key: QueueKey) -> Result<(), PlaybackError> {
    self.current = Some(self.position(key)?);
    Ok(())
  }

  /// Appends items after everything already queued, in both the shuffled and the original order.
  pub fn enqueue(&mut self, items: Vec<T>) {
    let entries: Vec<QueueEntry<T>> = items.into_iter().map(|item| self.entry(item)).collect();
    if let Some(original) = &mut self.unshuffled {
      original.extend(entries.iter().cloned());
    }
    self.entries.extend(entries);
    if self.current.is_none() && !self.entries.is_empty() {
      self.current = Some(0);
    }
  }

  pub fn remove(&mut self, key: QueueKey) -> Result<Removal, PlaybackError> {
    let position = self.position(key)?;
    self.entries.remove(position);
    if let Some(original) = &mut self.unshuffled {
      original.retain(|entry| entry.key != key);
    }
    let Some(current) = self.current else {
      return Ok(Removal::Other);
    };
    if position < current {
      self.current = Some(current - 1);
      Ok(Removal::Other)
    } else if position == current {
      let has_next = current < self.entries.len();
      self.current = if has_next {
        Some(current)
      } else {
        (!self.entries.is_empty()).then_some(0)
      };
      Ok(Removal::Current { has_next })
    } else {
      Ok(Removal::Other)
    }
  }

  /// Shuffling keeps the current entry playing and puts it first, followed by the rest in random order.
  pub fn set_shuffle(&mut self, shuffle: bool, rng: &mut impl Rng) {
    if shuffle == self.shuffle {
      return;
    }
    self.shuffle = shuffle;
    if shuffle {
      self.shuffle_entries(rng);
    } else {
      self.unshuffle();
    }
  }

  /// Puts a shuffled queue back in its original order, keeping the current entry current.
  fn unshuffle(&mut self) {
    if let Some(original) = self.unshuffled.take() {
      let current = self.current().map(QueueEntry::key);
      self.entries = original;
      self.current = current
        .and_then(|key| self.position(key).ok())
        .or(self.current.map(|_| 0));
    }
  }

  pub fn set_repeat(&mut self, repeat: RepeatMode) {
    self.repeat = repeat;
  }

  /// Moves to the next entry; returns false when the end was reached, leaving the first entry current. A shuffled
  /// queue that wraps around to its start is shuffled again for the new round.
  pub fn advance(&mut self, reason: Advance, rng: &mut impl Rng) -> bool {
    let Some(current) = self.current else { return false };
    let next = policy::next_index(self.repeat, current, self.entries.len(), reason);
    let wrapped = current + 1 == self.entries.len() && next != Some(current);
    if wrapped && self.shuffle {
      self.reshuffle_round(current, rng);
    }
    self.current = Some(next.unwrap_or(0));
    next.is_some()
  }

  /// The entry that plays once the current one has played to its end, as far as it is known yet.
  pub fn following(&self) -> Following<'_, T> {
    let Some(current) = self.current else {
      return Following::End;
    };
    match policy::next_index(self.repeat, current, self.entries.len(), Advance::TrackEnded) {
      None => Following::End,
      Some(next) if self.shuffle && current + 1 == self.entries.len() && next != current => Following::Unknown,
      Some(next) => Following::Entry(&self.entries[next]),
    }
  }

  /// Goes back one entry, or reports that the current one should restart.
  pub fn go_back(&mut self, elapsed: Duration) -> Previous {
    let Some(current) = self.current else {
      return Previous::Restart;
    };
    let previous = policy::previous(self.repeat, current, self.entries.len(), elapsed);
    if let Previous::Go(index) = previous {
      self.current = Some(index);
    }
    previous
  }

  fn entry(&mut self, item: T) -> QueueEntry<T> {
    self.last_key = self.last_key.wrapping_add(1);
    QueueEntry {
      key: QueueKey(self.last_key),
      item,
    }
  }

  fn position(&self, key: QueueKey) -> Result<usize, PlaybackError> {
    self
      .entries
      .iter()
      .position(|entry| entry.key == key)
      .ok_or(PlaybackError::EntryNotInQueue)
  }

  /// Mixes the queue again for the next round, keeping the entry at `ended`, which just played, away from the start.
  fn reshuffle_round(&mut self, ended: usize, rng: &mut impl Rng) {
    let len = self.entries.len();
    if len < 2 {
      return;
    }
    let ended = self.entries[ended].key;
    self.entries.shuffle(rng);
    if self.entries[0].key == ended {
      self.entries.swap(0, rng.random_range(1..len));
    }
  }

  fn shuffle_entries(&mut self, rng: &mut impl Rng) {
    if self.entries.is_empty() {
      return;
    }
    self.unshuffled = Some(self.entries.clone());
    let pivot = self.current.unwrap_or(0);
    let first = self.entries.remove(pivot);
    self.entries.shuffle(rng);
    self.entries.insert(0, first);
    self.current = Some(0);
  }
}

#[cfg(test)]
mod tests {
  use rand::SeedableRng;
  use rand::rngs::StdRng;

  use super::*;

  fn rng() -> StdRng {
    StdRng::seed_from_u64(7)
  }

  fn queue(items: &[u32], start: usize, shuffle: bool, repeat: RepeatMode) -> Queue<u32> {
    let mut queue = Queue::new(shuffle, repeat);
    queue
      .start(PlaybackContext::Songs, items.to_vec(), start, &mut rng())
      .unwrap();
    queue
  }

  fn items(queue: &Queue<u32>) -> Vec<u32> {
    queue.entries().iter().map(|e| *e.item()).collect()
  }

  fn current(queue: &Queue<u32>) -> Option<u32> {
    queue.current().map(|e| *e.item())
  }

  #[test]
  fn refreshing_keeps_keys_and_order() {
    let mut queue = queue(&[1, 2, 3], 1, true, RepeatMode::Off);
    let keys: Vec<_> = queue.entries().iter().map(QueueEntry::key).collect();
    queue.refresh(|item| (*item == 2).then_some(20));
    assert_eq!(queue.entries().iter().map(QueueEntry::key).collect::<Vec<_>>(), keys);
    assert_eq!(current(&queue), Some(20));
    queue.set_shuffle(false, &mut rng());
    assert_eq!(items(&queue), vec![1, 20, 3]);
  }

  #[test]
  fn starting_with_nothing_fails() {
    let mut queue = Queue::<u32>::new(false, RepeatMode::Off);
    assert_eq!(
      queue.start(PlaybackContext::Songs, vec![], 0, &mut rng()).unwrap_err(),
      PlaybackError::NothingToPlay
    );
  }

  #[test]
  fn shuffle_keeps_the_current_track_first_and_restores_order() {
    let mut queue = queue(&[1, 2, 3, 4, 5], 2, false, RepeatMode::Off);
    queue.set_shuffle(true, &mut rng());
    assert_eq!(current(&queue), Some(3));
    assert_eq!(queue.current_index(), Some(0));
    queue.advance(Advance::Skipped, &mut rng());
    let playing = current(&queue);
    queue.set_shuffle(false, &mut rng());
    assert_eq!(items(&queue), [1, 2, 3, 4, 5]);
    assert_eq!(current(&queue), playing);
  }

  #[test]
  fn a_shuffled_queue_is_mixed_again_each_round() {
    let mut queue = queue(&[1, 2, 3, 4, 5], 0, true, RepeatMode::All);
    let mut rng = rng();
    let mut rounds = vec![items(&queue)];
    for _ in 0..4 {
      for _ in 0..4 {
        queue.advance(Advance::Skipped, &mut rng);
      }
      let ended = current(&queue);
      queue.advance(Advance::Skipped, &mut rng);
      assert_eq!(queue.current_index(), Some(0));
      assert_ne!(current(&queue), ended);
      rounds.push(items(&queue));
    }
    assert!(rounds.windows(2).any(|pair| pair[0] != pair[1]));
    queue.set_shuffle(false, &mut rng);
    assert_eq!(items(&queue), [1, 2, 3, 4, 5]);
  }

  #[test]
  fn an_unshuffled_queue_keeps_its_order_round_after_round() {
    let mut queue = queue(&[1, 2, 3], 2, false, RepeatMode::All);
    queue.advance(Advance::Skipped, &mut rng());
    assert_eq!(items(&queue), [1, 2, 3]);
    assert_eq!(current(&queue), Some(1));
  }

  #[test]
  fn the_following_entry_is_named_unless_a_new_round_is_still_to_be_mixed() {
    let item = |following: Following<'_, u32>| match following {
      Following::Entry(entry) => Some(*entry.item()),
      Following::End | Following::Unknown => None,
    };
    assert_eq!(item(queue(&[1, 2, 3], 0, false, RepeatMode::Off).following()), Some(2));
    assert_eq!(item(queue(&[1, 2, 3], 1, false, RepeatMode::One).following()), Some(2));
    assert_eq!(item(queue(&[1, 2, 3], 2, false, RepeatMode::All).following()), Some(1));
    assert!(matches!(
      queue(&[1, 2, 3], 2, false, RepeatMode::Off).following(),
      Following::End
    ));
    let mut shuffled = queue(&[1, 2, 3], 0, true, RepeatMode::All);
    shuffled.advance(Advance::Skipped, &mut rng());
    shuffled.advance(Advance::Skipped, &mut rng());
    assert!(matches!(shuffled.following(), Following::Unknown));
  }

  #[test]
  fn reaching_the_end_rewinds_to_the_first_entry() {
    let mut queue = queue(&[1, 2], 1, false, RepeatMode::Off);
    assert!(!queue.advance(Advance::TrackEnded, &mut rng()));
    assert_eq!(current(&queue), Some(1));
  }

  #[test]
  fn removing_the_current_entry_moves_to_the_next() {
    let mut queue = queue(&[1, 2, 3], 1, false, RepeatMode::Off);
    let key = queue.current().unwrap().key();
    assert_eq!(queue.remove(key).unwrap(), Removal::Current { has_next: true });
    assert_eq!(current(&queue), Some(3));
  }

  #[test]
  fn removing_an_earlier_entry_keeps_the_current_one() {
    let mut queue = queue(&[1, 2, 3], 2, false, RepeatMode::Off);
    let first = queue.entries()[0].key();
    queue.remove(first).unwrap();
    assert_eq!(current(&queue), Some(3));
  }

  #[test]
  fn enqueued_entries_survive_unshuffling() {
    let mut queue = queue(&[1, 2, 3], 0, true, RepeatMode::Off);
    queue.enqueue(vec![9]);
    queue.set_shuffle(false, &mut rng());
    assert_eq!(items(&queue), [1, 2, 3, 9]);
  }

  #[test]
  fn jumping_to_a_removed_entry_fails() {
    let mut queue = queue(&[1, 2], 0, false, RepeatMode::Off);
    let key = queue.entries()[1].key();
    queue.remove(key).unwrap();
    assert_eq!(queue.jump(key).unwrap_err(), PlaybackError::EntryNotInQueue);
  }

  #[test]
  fn a_snapshot_restores_the_order_and_the_place() {
    let mut saved = queue(&[1, 2, 3, 4, 5], 2, true, RepeatMode::Off);
    saved.advance(Advance::Skipped, &mut rng());
    let playing = current(&saved);
    let snapshot = saved.snapshot(|item| *item);

    let mut restored = Queue::new(true, RepeatMode::Off);
    restored.restore(snapshot.clone());
    assert_eq!(items(&restored), items(&saved));
    assert_eq!(current(&restored), playing);
    restored.set_shuffle(false, &mut rng());
    assert_eq!(items(&restored), [1, 2, 3, 4, 5]);

    let mut unshuffled = Queue::new(false, RepeatMode::Off);
    unshuffled.restore(snapshot);
    assert_eq!(items(&unshuffled), [1, 2, 3, 4, 5]);
    assert_eq!(current(&unshuffled), playing);
  }

  #[test]
  fn resolving_drops_missing_items_and_moves_past_a_missing_current_one() {
    let snapshot = QueueSnapshot {
      context: Some(PlaybackContext::Songs),
      items: vec![1, 2, 3, 4],
      unshuffled: Some(vec![3, 2, 1, 0]),
      current: Some(1),
    };
    let (kept, current_kept) = snapshot.clone().resolve(|item| (item != 4).then_some(item));
    assert!(current_kept);
    assert_eq!(
      (kept.items, kept.current, kept.unshuffled),
      (vec![1, 2, 3], Some(1), Some(vec![2, 1, 0]))
    );

    let (moved, current_kept) = snapshot.clone().resolve(|item| (item != 2).then_some(item));
    assert!(!current_kept);
    assert_eq!((moved.items, moved.current), (vec![1, 3, 4], Some(1)));

    let (wrapped, _) = snapshot.resolve(|item| (item == 1).then_some(item));
    assert_eq!((wrapped.items, wrapped.current), (vec![1], Some(0)));
  }

  #[test]
  fn an_order_that_misses_an_item_is_not_whole() {
    assert!(whole_order(&[2, 0, 1], 3));
    assert!(!whole_order(&[0, 0, 1], 3));
    assert!(!whole_order(&[0, 1], 3));
    assert!(!whole_order(&[0, 1, 3], 3));
  }
}
