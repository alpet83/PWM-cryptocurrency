//! Dedicated FIFO writer for JsonFile epoch blocks.

use pwm_core::block::Block;
use std::path::PathBuf;
use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

const WRITER_CAPACITY: usize = 200;

enum Command {
    Append(Arc<Block>),
    Flush(mpsc::Sender<Result<(), String>>),
    Shutdown(mpsc::Sender<Result<(), String>>),
}

struct WriterState {
    sender: Option<SyncSender<Command>>,
    join: Option<JoinHandle<()>>,
}

/// Cloneable handle to one ordered epoch writer thread.
#[derive(Clone)]
pub(crate) struct BlockWriter {
    state: Arc<Mutex<WriterState>>,
}

impl BlockWriter {
    pub(crate) fn new(summary_path: PathBuf) -> Result<Self, String> {
        let (sender, receiver) = mpsc::sync_channel(WRITER_CAPACITY);
        let join = std::thread::Builder::new()
            .name("pwmd-block-writer".to_string())
            .spawn(move || writer_loop(summary_path, receiver))
            .map_err(|e| format!("spawn block writer: {e}"))?;
        Ok(Self {
            state: Arc::new(Mutex::new(WriterState {
                sender: Some(sender),
                join: Some(join),
            })),
        })
    }

    pub(crate) fn enqueue(&self, block: Arc<Block>) -> Result<(), String> {
        self.send(Command::Append(block))
    }

    pub(crate) fn flush(&self) -> Result<(), String> {
        let (reply_tx, reply_rx) = mpsc::channel();
        let state = self
            .state
            .lock()
            .map_err(|_| "block writer mutex poisoned".to_string())?;
        let Some(sender) = state.sender.as_ref() else {
            return Ok(());
        };
        send_fifo(sender, Command::Flush(reply_tx))?;
        reply_rx
            .recv()
            .map_err(|_| "block writer flush reply disconnected".to_string())?
    }

    pub(crate) fn shutdown(&self) -> Result<(), String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "block writer mutex poisoned".to_string())?;
        let Some(sender) = state.sender.take() else {
            return Ok(());
        };
        let (reply_tx, reply_rx) = mpsc::channel();
        let send_result = send_fifo(&sender, Command::Shutdown(reply_tx));
        drop(sender);
        let reply_result = send_result.and_then(|()| {
            reply_rx
                .recv()
                .map_err(|_| "block writer shutdown reply disconnected".to_string())?
        });
        let join_result = state.join.take().map_or(Ok(()), |join| {
            join.join()
                .map_err(|_| "block writer thread panicked".to_string())
        });
        reply_result.and(join_result)
    }

    fn send(&self, command: Command) -> Result<(), String> {
        let state = self
            .state
            .lock()
            .map_err(|_| "block writer mutex poisoned".to_string())?;
        let sender = state
            .sender
            .as_ref()
            .ok_or_else(|| "block writer is shut down".to_string())?;
        send_fifo(sender, command)
    }
}

fn send_fifo(sender: &SyncSender<Command>, command: Command) -> Result<(), String> {
    match sender.try_send(command) {
        Ok(()) => Ok(()),
        Err(TrySendError::Full(command)) => sender
            .send(command)
            .map_err(|_| "block writer disconnected".to_string()),
        Err(TrySendError::Disconnected(_)) => Err("block writer disconnected".to_string()),
    }
}

fn writer_loop(summary_path: PathBuf, receiver: mpsc::Receiver<Command>) {
    let mut pending_error = None;
    while let Ok(command) = receiver.recv() {
        match command {
            Command::Append(block) => {
                if let Some(err) = pending_error.as_ref() {
                    tracing::warn!(
                        height = block.hdr.height,
                        error = %err,
                        "skipping block append after writer failure"
                    );
                    continue;
                }
                if let Err(err) =
                    crate::snapshot::incremental::append_block_for_epoch(&summary_path, &block)
                {
                    pending_error = Some(err);
                }
            }
            Command::Flush(reply) => {
                let _ = reply.send(pending_error.clone().map_or(Ok(()), Err));
            }
            Command::Shutdown(reply) => {
                let _ = reply.send(pending_error.clone().map_or(Ok(()), Err));
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::BlockWriter;
    use crate::app_from_dev_net;
    use crate::snapshot::incremental::load_blocks_from_epochs;
    use pwm_core::block::Block;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(tag: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir()
            .join(format!("pwmd-writer-{tag}-{suffix}"))
            .join("pwm-data.json")
    }

    fn sealed_blocks(count: usize) -> Vec<Block> {
        let app = app_from_dev_net();
        let mut inner = app.inner.try_write().expect("inner");
        (0..count)
            .map(|_| {
                inner.chain.seal(vec![]).expect("seal");
                inner.chain.blocks.back().expect("tip").clone()
            })
            .collect()
    }

    #[test]
    fn preserves_append_order() {
        let path = temp_path("order");
        let writer = BlockWriter::new(path.clone()).expect("writer");
        for block in sealed_blocks(3) {
            writer.enqueue(Arc::new(block)).expect("enqueue");
        }
        writer.flush().expect("flush");
        let heights: Vec<u64> = load_blocks_from_epochs(&path)
            .expect("load")
            .iter()
            .map(|block| block.hdr.height)
            .collect();
        assert_eq!(heights, vec![1, 2, 3]);
        writer.shutdown().expect("shutdown");
    }

    #[test]
    fn flushes_pending_blocks() {
        let path = temp_path("flush");
        let writer = BlockWriter::new(path.clone()).expect("writer");
        writer
            .enqueue(Arc::new(sealed_blocks(1).remove(0)))
            .expect("enqueue");
        writer.flush().expect("flush");
        assert_eq!(load_blocks_from_epochs(&path).expect("load").len(), 1);
        writer.shutdown().expect("shutdown");
    }

    #[test]
    fn shutdown_is_idempotent() {
        let path = temp_path("shutdown");
        let writer = BlockWriter::new(path.clone()).expect("writer");
        let clone = writer.clone();
        writer
            .enqueue(Arc::new(sealed_blocks(1).remove(0)))
            .expect("enqueue");
        clone.shutdown().expect("first shutdown");
        writer.shutdown().expect("second shutdown");
        assert_eq!(load_blocks_from_epochs(&path).expect("load").len(), 1);
    }

    #[test]
    fn writer_stops_after_error() {
        let path = temp_path("fail-fast");
        let writer = BlockWriter::new(path.clone()).expect("writer");
        let blocks = sealed_blocks(3);

        writer
            .enqueue(Arc::new(blocks[0].clone()))
            .expect("enqueue height 1");
        writer
            .enqueue(Arc::new(blocks[2].clone()))
            .expect("enqueue height 3");
        writer
            .enqueue(Arc::new(blocks[1].clone()))
            .expect("enqueue height 2");

        assert!(writer.flush().is_err());
        let heights: Vec<u64> = load_blocks_from_epochs(&path)
            .expect("load")
            .iter()
            .map(|block| block.hdr.height)
            .collect();
        assert_eq!(heights, vec![1]);
        assert!(writer.shutdown().is_err());

        std::fs::remove_dir_all(path.parent().expect("temp directory"))
            .expect("remove temp directory");
    }
}
