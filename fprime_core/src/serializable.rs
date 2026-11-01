pub trait Serializable: Sized {
    const SIZE: usize;

    fn serialize_to(&self, to: &mut [u8], offset: &mut usize);
    fn deserialize_from(from: &[u8], offset: &mut usize) -> Self;
    fn deserialize(from: &[u8]) -> Self {
        let mut offset: usize = 0;
        Self::deserialize_from(&from, &mut offset)
    }
}

macro_rules! primitive {
    ($primitive: ty) => {
        impl Serializable for $primitive {
            const SIZE: usize = size_of::<$primitive>();

            fn serialize_to(&self, to: &mut [u8], offset: &mut usize) {
                let bytes = self.to_be_bytes();
                let Some(dst) = to.get_mut(*offset..*offset + Self::SIZE) else {
                    crate::panic(crate::PanicCode::Truncated)
                };

                dst.copy_from_slice(&bytes);
                *offset += Self::SIZE;
            }

            fn deserialize_from(from: &[u8], offset: &mut usize) -> Self {
                let Some(bytes) = from.get(*offset..*offset + Self::SIZE) else {
                    crate::panic(crate::PanicCode::Truncated)
                };

                let Ok(bytes) = <[u8; size_of::<$primitive>()]>::try_from(bytes) else {
                    crate::panic(crate::PanicCode::Truncated)
                };
                *offset += Self::SIZE;
                Self::from_be_bytes(bytes)
            }
        }
    };
}

primitive!(u8);
primitive!(i8);
primitive!(u16);
primitive!(i16);
primitive!(u32);
primitive!(i32);
primitive!(u64);
primitive!(i64);
primitive!(f32);
primitive!(f64);

impl<T: Serializable, const N: usize> Serializable for [T; N] {
    const SIZE: usize = T::SIZE * N;

    fn serialize_to(&self, to: &mut [u8], offset: &mut usize) {
        for i in self {
            i.serialize_to(to, offset);
        }
    }

    fn deserialize_from(from: &[u8], offset: &mut usize) -> Self {
        let mut out: [T; N] = unsafe {
            #[allow(invalid_value)]
            core::mem::MaybeUninit::uninit().assume_init()
        };

        for i in 0..N {
            out[i] = T::deserialize_from(from, offset);
        }

        out
    }
}

impl Serializable for bool {
    const SIZE: usize = 1;

    fn serialize_to(&self, to: &mut [u8], offset: &mut usize) {
        let val: u8 = if *self { 1 } else { 0 };
        val.serialize_to(to, offset);
    }

    fn deserialize_from(from: &[u8], offset: &mut usize) -> Self {
        if u8::deserialize_from(from, offset) != 0 {
            true
        } else {
            false
        }
    }
}
