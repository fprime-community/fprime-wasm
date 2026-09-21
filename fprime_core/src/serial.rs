use core::cell::UnsafeCell;
use core::marker::PhantomData;

use crate::abi;

use crate::Serializable;

pub struct Queue<T: Serializable, const N: usize> {
    index: i32,
    buffer: [u8; N],
    data: PhantomData<T>,
}

/// The input serial port at `index`, carrying values of type `T`.
///
/// ```ignore
/// let inbox = serial_in!(u32, 0);
/// let request = inbox.recv_block();
/// ```
///
/// The buffer is sized from `T`, so the wire width is never yours to write down.
/// This is how to build one: [`Queue::new`] takes that width as a second
/// parameter and has nothing to infer it from.
#[macro_export]
macro_rules! serial_in {
    ($t:ty, $index:expr) => {
        $crate::Queue::<$t, { <$t as $crate::Serializable>::SIZE }>::new($index)
    };
}

impl<T: Serializable, const N: usize> Queue<T, N> {
    pub fn new(index: i32) -> Queue<T, N> {
        const { assert!(N == T::SIZE) };

        Queue {
            index,
            buffer: [0; N],
            data: Default::default(),
        }
    }

    pub fn recv_block(&self) -> T {
        let mut written: u32 = 0;

        let _ = unsafe {
            abi::serial_recv(
                self.index,
                self.buffer.as_ptr() as u32,
                core::mem::size_of::<T>() as u32,
                (&mut written) as *mut u32 as u32,
                /* blocking */ 0,
            )
        };

        T::deserialize(&self.buffer[0..(written as usize)])
    }

    pub fn recv_poll(&self) -> Option<T> {
        let mut written: u32 = 0;

        let status = unsafe {
            abi::serial_recv(
                self.index,
                self.buffer.as_ptr() as u32,
                core::mem::size_of::<T>() as u32,
                (&mut written) as *mut u32 as u32,
                /* non blocking */ 1,
            )
        };

        if status == 0 {
            Some(T::deserialize(&self.buffer[0..(written as usize)]))
        } else {
            None
        }
    }
}

pub struct Sender<T: Serializable, const N: usize> {
    index: i32,
    buffer: UnsafeCell<[u8; N]>,
    data: PhantomData<T>,
}

/// The output serial port at `index`, carrying values of type `T`.
///
/// ```ignore
/// let outbox = serial_out!(u32, 0);
/// outbox.send(1);
/// ```
///
/// The counterpart of [`serial_in!`], and sized the same way.
#[macro_export]
macro_rules! serial_out {
    ($t:ty, $index:expr) => {
        $crate::Sender::<$t, { <$t as $crate::Serializable>::SIZE }>::new($index)
    };
}

impl<T: Serializable, const N: usize> Sender<T, N> {
    pub fn new(index: i32) -> Sender<T, N> {
        const { assert!(N == T::SIZE) };

        Sender {
            index,
            buffer: UnsafeCell::new([0; N]),
            data: Default::default(),
        }
    }

    pub fn send(&self, value: T) {
        let mut size = 0;

        // SAFETY: a guest is single-threaded and the borrow does not outlive this call, so
        // nothing else can hold a reference to the buffer while this one is live.
        let buffer = unsafe { &mut *self.buffer.get() };
        value.serialize_to(buffer, &mut size);

        unsafe { abi::serial_send(self.index, buffer.as_ptr() as u32, size as u32) };
    }
}
