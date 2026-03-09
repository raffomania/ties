#![allow(unused_imports)]
pub use garde::rules::*; // re-export garde's rules

pub mod length {
    pub use garde::rules::length::*; // re-export `length` rules

    pub mod simple {
        //! Implemented by types which have a known length.
        //!
        //! The meaning of "length" depends on the type.
        //! For example, the length of a `String` is defined as the number of
        //! _bytes_ it stores.

        use garde::error::Error;

        pub fn apply<T: Simple>(v: &T, (min, max): (usize, usize)) -> Result<(), Error> {
            v.validate_length(min, max)
        }

        pub trait Simple {
            fn validate_length(&self, min: usize, max: usize) -> Result<(), Error>;
        }

        impl<T: HasSimpleLength> Simple for T {
            fn validate_length(&self, min: usize, max: usize) -> Result<(), Error> {
                check_length(self.length(), min, max)
            }
        }

        impl<T: Simple> Simple for Option<T> {
            fn validate_length(&self, min: usize, max: usize) -> Result<(), Error> {
                match self {
                    Some(v) => v.validate_length(min, max),
                    None => Ok(()),
                }
            }
        }

        fn check_length(len: usize, min: usize, max: usize) -> Result<(), Error> {
            if len < min {
                Err(Error::new(format!("length is lower than {min}")))
            } else if len > max {
                Err(Error::new(format!("length is greater than {max}")))
            } else {
                Ok(())
            }
        }

        pub trait HasSimpleLength {
            fn length(&self) -> usize;
        }

        impl HasSimpleLength for url::Url {
            fn length(&self) -> usize {
                self.as_str().len()
            }
        }
    }
}
