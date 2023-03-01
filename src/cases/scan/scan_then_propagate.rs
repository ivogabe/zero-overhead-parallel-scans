use core::sync::atomic::{Ordering, AtomicU64};
use crate::cases::scan::scan_sequential;
use crate::core::worker::{*};
use crate::core::task::{*};
use crate::core::workassisting_loop::{*};

const BLOCK_COUNT: u64 = 32 * 8;
const MIN_BLOCK_SIZE: u64 = 1024;

#[derive(Copy, Clone)]
struct Data<'a> {
  input: &'a [u64],
  output: &'a [AtomicU64],
  block_size: u64,
  block_count: u64
}

pub fn create_task(input: &[u64], output: &[AtomicU64]) -> Task {
  let len = output.len();
  let mut block_size = (len as u64 + BLOCK_COUNT - 1) / BLOCK_COUNT;
  let mut block_count = BLOCK_COUNT;
  if block_size < MIN_BLOCK_SIZE {
    block_size = MIN_BLOCK_SIZE;
    block_count = (len as u64 + MIN_BLOCK_SIZE - 1) / MIN_BLOCK_SIZE;
  }
  Task::new_dataparallel::<Data>(phase1_run, phase1_finish, Data{ input, output, block_size, block_count }, block_count as u32, false)
}

fn phase1_run(_workers: &Workers, data: &Data, loop_arguments: LoopArguments) {
  workassisting_loop!(loop_arguments, |block_index| {
    // Local scan
    let start = block_index as usize * data.block_size as usize;
    let end = ((block_index as usize + 1) * data.block_size as usize).min(data.output.len());
    scan_sequential(&data.input[start .. end], 0, &data.output[start .. end]);
  });
}

fn phase1_finish(workers: &Workers, data: &Data) {
  // Phase 2
  // Scan the values of the separate blocks.
  // After this loop, all elements at an index `i * block_size - 1` are correct. 
  let mut index = data.block_size as usize - 1;
  let mut aggregate = 0;
  while index < data.output.len() {
    aggregate = aggregate + data.output[index].load(Ordering::Relaxed);
    data.output[index].store(aggregate, Ordering::Relaxed);
    index += data.block_size as usize;
  }

  let block_count = data.block_count;
  workers.push_task(Task::new_dataparallel(phase3_run, phase3_finish, *data, block_count as u32 - 1, false))
}

fn phase3_run(_workers: &Workers, data: &Data, loop_arguments: LoopArguments) {
  workassisting_loop!(loop_arguments, |block_idx_min_1| {
    // No work needs to be done for block 1
    let block_idx = block_idx_min_1 + 1;
    let start = block_idx as usize * data.block_size as usize;
    let prefix = data.output[start - 1].load(Ordering::Relaxed);

    // Exclusive upper bound of this block.
    // The last element of this block is already correct, so we do not iterate over that, hence the `- 1`.
    // In case of the last block, we may have a shorter block.
    let end = (start + data.block_size as usize - 1).min(data.output.len());
    let output = &data.output[start .. end];
    for field in output {
      let old = field.load(Ordering::Relaxed);
      field.store(prefix + old, Ordering::Relaxed);
    }
  });
}

fn phase3_finish(workers: &Workers, _data: &Data) {
  workers.finish();
}
