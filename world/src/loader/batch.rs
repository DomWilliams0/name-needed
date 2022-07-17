use misc::*;
use std::collections::HashMap;
use std::convert::TryInto;
use std::fmt::{Debug, Formatter};

type BatchId = u16;
type BatchSize = u16;

#[derive(Copy, Clone)]
pub struct UpdateBatch {
    batch_id: BatchId,

    batch_size: BatchSize,
    my_idx: BatchSize,
}

pub struct UpdateBatchBuilder {
    next: UpdateBatch,
}

#[derive(Default)]
pub(crate) struct UpdateBatchUniqueId(BatchId);

pub struct UpdateBatcher<U> {
    batches: HashMap<BatchId, (UpdateBatch, Vec<U>)>,
}

impl UpdateBatch {
    /// Panics if batch size == 0 or >= <typeof(batch_size)>::MAX
    pub(crate) fn builder(
        batch_id: &mut UpdateBatchUniqueId,
        batch_size: usize,
    ) -> UpdateBatchBuilder {
        let batch_id = {
            let current = batch_id.0;
            batch_id.0 = current.wrapping_add(1);
            current
        };

        let batch_size = batch_size.try_into().unwrap_or_else(|_| {
            panic!(
                "max batch size ({}) is too small for batch of size {}",
                BatchSize::MAX,
                batch_size
            )
        });
        assert_ne!(batch_size, 0, "batch size must be > 0");

        UpdateBatchBuilder {
            next: UpdateBatch {
                batch_id,
                batch_size,
                my_idx: 1,
            },
        }
    }

    #[cfg(test)]
    fn is_last(&self) -> bool {
        self.my_idx == self.batch_size
    }
}

impl UpdateBatchBuilder {
    pub fn next_batch(&mut self) -> UpdateBatch {
        let batch = self.next;

        // ensure we haven't gone past the end of the batch
        assert!(
            batch.my_idx <= batch.batch_size,
            "batch size exceeded, only expected {}",
            batch.batch_size
        );

        // prepare next
        self.next.my_idx += 1;

        batch
    }

    pub(crate) fn is_complete(&self) -> Result<(), (usize, usize)> {
        if self.next.my_idx == self.next.batch_size + 1 {
            // all done, no batches left
            Ok(())
        } else {
            // still some batches to go
            Err((self.next.my_idx as usize, self.next.batch_size as usize))
        }
    }

    #[cfg(test)]
    fn batch_id(&self) -> BatchId {
        self.next.batch_id
    }
}

impl<U> UpdateBatcher<U> {
    pub fn submit(&mut self, batch: UpdateBatch, item: U) {
        let (last_seen, items) = self
            .batches
            .entry(batch.batch_id)
            .or_insert_with(|| (batch, Vec::with_capacity(batch.batch_size as usize)));

        *last_seen = batch;
        items.push(item);
    }

    /// (batch id, batch length)
    /// Returns a SmallVec instead of an iterator as a reference to self is still needed to
    /// pop completed batches
    pub fn complete_batches(&self) -> SmallVec<[(BatchId, usize); 8]> {
        self.batches
            .values()
            .filter_map(|(batch, items)| {
                if batch.batch_size as usize == items.len() {
                    Some((batch.batch_id, items.len()))
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn pop_batch(&mut self, batch: BatchId) -> (UpdateBatch, Vec<U>) {
        let (batch, items) = self.batches.remove(&batch).expect("invalid batch");
        (batch, items)
    }
}

impl<U> Default for UpdateBatcher<U> {
    fn default() -> Self {
        Self {
            batches: HashMap::with_capacity(8),
        }
    }
}

impl Debug for UpdateBatch {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "UpdateBatch(id={}, {}/{})",
            self.batch_id, self.my_idx, self.batch_size
        )
    }
}

slog_kv_debug!(UpdateBatch, "batch");

#[cfg(test)]
mod tests {
    use crate::loader::batch::{UpdateBatchUniqueId, UpdateBatcher};
    use crate::loader::UpdateBatch;

    #[test]
    fn batch_creation() {
        let mut ids = UpdateBatchUniqueId::default();
        let mut batches = UpdateBatch::builder(&mut ids, 3);

        let b1 = batches.next_batch();
        let b2 = batches.next_batch();
        let b3 = batches.next_batch();

        assert_eq!(b1.my_idx, 1);
        assert_eq!(b2.my_idx, 2);
        assert_eq!(b3.my_idx, 3);

        assert!(!b1.is_last());
        assert!(!b2.is_last());
        assert!(b3.is_last());

        assert!(batches.is_complete().is_ok());
    }

    #[test]
    fn single_item() {
        let mut ids = UpdateBatchUniqueId::default();
        let mut batches = UpdateBatch::builder(&mut ids, 1);

        let b1 = batches.next_batch();
        assert!(b1.is_last());
    }

    #[test]
    fn batcher() {
        let mut ids = UpdateBatchUniqueId::default();
        let mut batches_a = UpdateBatch::builder(&mut ids, 3); // "hey whats up"
        let mut batches_b = UpdateBatch::builder(&mut ids, 2); // "very cool"
        let mut batches_c = UpdateBatch::builder(&mut ids, 1); // "nice"

        let mut batcher = UpdateBatcher::default();

        batcher.submit(batches_a.next_batch(), "hey");
        batcher.submit(batches_a.next_batch(), "whats");

        // batches are out of order, oh no
        batcher.submit(batches_b.next_batch(), "very");

        // and another
        batcher.submit(batches_c.next_batch(), "nice");

        // only the last batch is complete
        let complete = batcher.complete_batches();
        assert_eq!(complete.to_vec(), vec![(batches_c.batch_id(), 1)]);
        assert_eq!(batcher.pop_batch(batches_c.batch_id()).1, vec!["nice"]);

        // finish off first batch
        batcher.submit(batches_a.next_batch(), "up");
        let complete = batcher.complete_batches();
        assert_eq!(complete.to_vec(), vec![(batches_a.batch_id(), 3)]);
        assert_eq!(
            batcher.pop_batch(batches_a.batch_id()).1,
            vec!["hey", "whats", "up"]
        );

        // and finish off last batch
        batcher.submit(batches_b.next_batch(), "cool");
        let complete = batcher.complete_batches();
        assert_eq!(complete.to_vec(), vec![(batches_b.batch_id(), 2)]);
        assert_eq!(
            batcher.pop_batch(batches_b.batch_id()).1,
            vec!["very", "cool"]
        );

        let complete = batcher.complete_batches();
        assert!(complete.is_empty());
    }

    #[test]
    fn batcher_out_of_order() {
        let mut ids = UpdateBatchUniqueId::default();
        let mut batches = UpdateBatch::builder(&mut ids, 2);

        let mut batcher = UpdateBatcher::default();

        let b1 = batches.next_batch();
        let b2 = batches.next_batch();
        batcher.submit(b2, "two");

        // not done yet
        assert!(batcher.complete_batches().is_empty());

        batcher.submit(b1, "one");

        let complete = batcher.complete_batches().to_vec();
        assert_eq!(complete.len(), 1);

        let (batch_id, batch_len) = *complete.first().unwrap();
        assert_eq!(batch_len, 2);

        let (_, items) = batcher.pop_batch(batch_id);
        assert_eq!(items.len(), 2);
    }
}
