//! Feeding a `.wasm` file to `spacewasm`'s streaming decoder.
//!
//! `spacewasm::Module::new` reads through a [`WasmStream`] rather than a slice,
//! because on board the module arrives from a file or a radio in chunks and never
//! exists in memory whole. It borrows each chunk and hands it back through
//! `return_`, so the stream owns the buffers and has to keep them alive for
//! exactly as long as the decoder holds them.

use spacewasm::{InnerVec, WasmStream};
use std::collections::HashMap;

/// Chunk size handed to the decoder. Only affects how often `read` is called.
const CHUNK: usize = 1024;

/// A [`WasmStream`] over a module already in memory.
pub struct Bytes {
    data: Vec<u8>,
    pos: usize,
    /// Chunks the decoder currently holds, keyed by the pointer it will hand back.
    /// Keeping the `Vec` here is what keeps the borrowed memory alive and lets it
    /// be freed exactly once, on `return_`.
    lent: HashMap<*mut u8, Vec<u8>>,
}

impl Bytes {
    pub fn new(data: Vec<u8>) -> Self {
        Bytes {
            data,
            pos: 0,
            lent: HashMap::new(),
        }
    }

    /// Bytes handed to the decoder so far. Only the tests below read it: the decoder
    /// tracks its own position.
    #[cfg(test)]
    pub fn consumed(&self) -> usize {
        self.pos
    }
}

impl WasmStream for Bytes {
    fn read(&mut self) -> Result<Option<InnerVec<u8>>, u8> {
        if self.pos >= self.data.len() {
            return Ok(None);
        }
        let end = (self.pos + CHUNK).min(self.data.len());
        let mut chunk = self.data[self.pos..end].to_vec();
        self.pos = end;

        let ptr = chunk.as_mut_ptr();
        let capacity = chunk.capacity();
        let len = chunk.len();
        // SAFETY: `chunk` is moved into `self.lent` below and not touched again
        // until `return_`, so this view stays valid and uniquely held for as long
        // as the decoder has it. The `InnerVec` is a borrow, not an owner: it is
        // dropped by `return_` handing it back, and the `Vec` frees the memory.
        let view = unsafe { InnerVec::from_raw_parts(ptr, capacity, len) };
        self.lent.insert(ptr, chunk);
        Ok(Some(view))
    }

    fn return_(&mut self, chunk: InnerVec<u8>) {
        // Dropping the `Vec` frees the allocation the `InnerVec` pointed at. A
        // chunk we never lent out would mean the decoder invented a pointer.
        self.lent
            .remove(&chunk.ptr())
            .expect("spacewasm returned a chunk this stream never lent out");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drain(stream: &mut Bytes) -> Vec<u8> {
        let mut seen = Vec::new();
        while let Some(chunk) = stream.read().expect("no I/O error") {
            // `InnerVec` derefs to a slice of the lent bytes.
            seen.extend_from_slice(&chunk);
            stream.return_(chunk);
        }
        seen
    }

    #[test]
    fn replays_every_byte_in_order() {
        let data: Vec<u8> = (0..(CHUNK * 2 + 7)).map(|i| i as u8).collect();
        let mut stream = Bytes::new(data.clone());
        assert_eq!(drain(&mut stream), data);
        assert_eq!(stream.consumed(), data.len());
    }

    /// The decoder may call `read` again after the end; that has to keep
    /// returning `None` rather than lending a chunk it will never return.
    #[test]
    fn reads_past_the_end_are_empty() {
        let mut stream = Bytes::new(b"abc".to_vec());
        drain(&mut stream);
        for _ in 0..4 {
            assert!(stream.read().expect("no I/O error").is_none());
        }
        assert!(stream.lent.is_empty(), "no chunk should be outstanding");
    }

    #[test]
    fn an_empty_module_lends_nothing() {
        let mut stream = Bytes::new(Vec::new());
        assert!(stream.read().expect("no I/O error").is_none());
        assert_eq!(stream.consumed(), 0);
    }

    /// A chunk is outstanding until returned, and freeing it twice would be a
    /// double free — so the bookkeeping has to be exact.
    #[test]
    fn tracks_outstanding_chunks() {
        let mut stream = Bytes::new(vec![7; CHUNK * 2]);
        let first = stream.read().expect("no I/O error").expect("a chunk");
        let second = stream.read().expect("no I/O error").expect("a chunk");
        assert_eq!(stream.lent.len(), 2);
        stream.return_(second);
        assert_eq!(stream.lent.len(), 1);
        stream.return_(first);
        assert!(stream.lent.is_empty());
    }
}
