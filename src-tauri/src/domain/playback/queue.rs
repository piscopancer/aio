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
    } else if let Some(original) = self.unshuffled.take() {
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

  /// Moves to the next entry; returns false when the end was reached, leaving the first entry current.
  pub fn advance(&mut self, reason: Advance) -> bool {
    let Some(current) = self.current else { return false };
    match policy::next_index(self.repeat, current, self.entries.len(), reason) {
      Some(next) => {
        self.current = Some(next);
        true
      }
      None => {
        self.current = Some(0);
        false
      }
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
    queue.advance(Advance::Skipped);
    let playing = current(&queue);
    queue.set_shuffle(false, &mut rng());
    assert_eq!(items(&queue), [1, 2, 3, 4, 5]);
    assert_eq!(current(&queue), playing);
  }

  #[test]
  fn reaching_the_end_rewinds_to_the_first_entry() {
    let mut queue = queue(&[1, 2], 1, false, RepeatMode::Off);
    assert!(!queue.advance(Advance::TrackEnded));
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
}
