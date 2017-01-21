use std::ffi::{CStr, CString, IntoStringError, NulError};
use std::os::raw::c_char;
use std::str::Utf8Error;
use std::marker::Sized;
use std::mem;

// -------------------- Our Trait ------------------------

pub trait ReprC {
    type C;
    type Error;

    fn from_repr_c_owned(c: *mut Self::C) -> Result<Self, Self::Error> where Self: Sized;
    fn from_repr_c_cloned(c: *const Self::C) -> Result<Self, Self::Error> where Self: Sized;
    fn into_repr_c(self) -> Result<Self::C, Self::Error>;
}

// -------------------- Strings Module ------------------------

#[derive(Debug)]
pub enum StringError {
    Utf8(Utf8Error),
    Null(NulError),
    IntoString(IntoStringError),
}

impl From<Utf8Error> for StringError {
    fn from(e: Utf8Error) -> Self {
        StringError::Utf8(e)
    }
}

impl From<NulError> for StringError {
    fn from(e: NulError) -> Self {
        StringError::Null(e)
    }
}

impl From<IntoStringError> for StringError {
    fn from(e: IntoStringError) -> Self {
        StringError::IntoString(e)
    }
}

impl ReprC for String {
    type C = (*mut c_char);
    type Error = StringError;

    fn from_repr_c_owned(c: *mut Self::C) -> Result<Self, Self::Error> {
        Ok(unsafe { CString::from_raw(*c) }.into_string()?)
    }
    fn from_repr_c_cloned(c: *const Self::C) -> Result<Self, Self::Error> {
        Ok(unsafe { CStr::from_ptr(*c) }.to_str()?.to_owned())
    }
    fn into_repr_c(self) -> Result<Self::C, Self::Error> {
        Ok((CString::new(self)?.into_raw()))
    }
}

// -------------------- Vec Module ------------------------

impl<T: ReprC + Clone> ReprC for Vec<T> {
    type C = (*mut T::C, usize, usize);
    type Error = T::Error;

    fn from_repr_c_owned(c: *mut Self::C) -> Result<Self, Self::Error> {
        let v_ffi = unsafe { Vec::from_raw_parts((*c).0, (*c).1, (*c).2) };
        let mut v = Vec::with_capacity(v_ffi.len());
        for mut elt in v_ffi {
            v.push(T::from_repr_c_owned(&mut elt)?);
        }
        Ok(v)
    }
    fn from_repr_c_cloned(c: *const Self::C) -> Result<Self, Self::Error> {
        let slice_ffi = unsafe { std::slice::from_raw_parts((*c).0, (*c).1) };
        let mut v = Vec::with_capacity(slice_ffi.len());
        for elt in slice_ffi {
            v.push(T::from_repr_c_cloned(elt)?);
        }
        Ok(v)
    }
    fn into_repr_c(self) -> Result<Self::C, Self::Error> {
        let mut v = Vec::with_capacity(self.len());
        for elt in self {
            let new_elt = elt.into_repr_c()?;
            v.push(new_elt);
        }
        let (ptr, len, cap) = (v.as_mut_ptr(), v.len(), v.capacity());
        mem::forget(v);
        Ok((ptr, len, cap))
    }
}

// Specialise for primitive u8 to prevent unnecessary copy of it. Vec of PODs can directly be owned.
impl ReprC for Vec<u8> {
    type C = (*mut u8, usize, usize);
    type Error = ();

    fn from_repr_c_owned(c: *mut Self::C) -> Result<Self, Self::Error> {
        Ok(unsafe { Vec::from_raw_parts((*c).0, (*c).1, (*c).2) })
    }
    fn from_repr_c_cloned(c: *const Self::C) -> Result<Self, Self::Error> {
        Ok(unsafe { std::slice::from_raw_parts((*c).0, (*c).1) }.to_vec())
    }
    fn into_repr_c(mut self) -> Result<Self::C, Self::Error> {
        let (ptr, len, cap) = (self.as_mut_ptr(), self.len(), self.capacity());
        std::mem::forget(self);
        Ok((ptr, len, cap))
    }
}

// -------------------- IPC Module ------------------------

#[derive(Debug)]
enum IpcError {
    StringError(StringError),
    U8Error,
}

impl From<StringError> for IpcError {
    fn from(e: StringError) -> Self {
        IpcError::StringError(e)
    }
}
impl From<()> for IpcError {
    fn from(_: ()) -> Self {
        IpcError::U8Error
    }
}

// -----------------

#[derive(Clone)]
struct One {
    a: String,
}

impl ReprC for One {
    type C = OneFfi;
    type Error = IpcError;


    fn from_repr_c_owned(c: *mut Self::C) -> Result<Self, Self::Error> {
        Ok(One { a: unsafe { String::from_repr_c_owned(&mut ((*c).a))? } })
    }
    fn from_repr_c_cloned(c: *const Self::C) -> Result<Self, Self::Error> {
        Ok(One { a: unsafe { String::from_repr_c_cloned(&((*c).a))? } })
    }
    fn into_repr_c(self) -> Result<Self::C, Self::Error> {
        Ok(OneFfi { a: self.a.into_repr_c()? })
    }
}

#[repr(C)]
#[derive(Debug)]
struct OneFfi {
    a: *mut c_char,
}

// -----------------

struct Two {
    a: String,
    b: Vec<u8>,
    c: Vec<One>,
    d: One,
}

impl ReprC for Two {
    type C = TwoFfi;
    type Error = IpcError;

    fn from_repr_c_owned(c: *mut Self::C) -> Result<Self, Self::Error> {
        let two_ffi = unsafe { &mut *c };
        Ok(Two {
            a: String::from_repr_c_owned(&mut (two_ffi.a))?,
            b: Vec::<u8>::from_repr_c_owned(&mut (two_ffi.b, two_ffi.b_len, two_ffi.b_cap))?,
            c: Vec::<One>::from_repr_c_owned(&mut (two_ffi.c, two_ffi.c_len, two_ffi.c_cap))?,
            d: One::from_repr_c_owned(&mut two_ffi.d)?,
        })
    }
    fn from_repr_c_cloned(c: *const Self::C) -> Result<Self, Self::Error> {
        let two_ffi = unsafe { &*c };
        Ok(Two {
            a: String::from_repr_c_cloned(&(two_ffi.a))?,
            b: Vec::<u8>::from_repr_c_cloned(&(two_ffi.b, two_ffi.b_len, two_ffi.b_cap))?,
            c: Vec::<One>::from_repr_c_cloned(&(two_ffi.c, two_ffi.c_len, two_ffi.c_cap))?,
            d: One::from_repr_c_cloned(&two_ffi.d)?,
        })
    }
    fn into_repr_c(self) -> Result<Self::C, Self::Error> {
        let (b_ptr, b_len, b_cap) = self.b.into_repr_c()?;
        let (c_ptr, c_len, c_cap) = self.c.into_repr_c()?;
        Ok(TwoFfi {
            a: self.a.into_repr_c()?,
            b: b_ptr,
            b_len: b_len,
            b_cap: b_cap,
            c: c_ptr,
            c_len: c_len,
            c_cap: c_cap,
            d: self.d.into_repr_c()?,
        })
    }
}

#[repr(C)]
#[derive(Debug)]
struct TwoFfi {
    a: *mut c_char,
    b: *mut u8,
    b_len: usize,
    b_cap: usize,
    c: *mut OneFfi,
    c_len: usize,
    c_cap: usize,
    d: OneFfi,
}

impl Drop for TwoFfi {
    fn drop(&mut self) {
        println!("Dropping {:?}", self);
        let _ = Two::from_repr_c_owned(self);
    }
}

// ----------------------------------------------------------------------

fn main() {
    let two = {
        let string = "SomeString".to_string();
        let one_str = "Hello".to_string();
        let one = One { a: one_str };
        let v_u8 = vec![10, 20, 30, 40, 50];
        let v_one = {
            let one_1 = One { a: "one_1".to_string() };
            let one_2 = One { a: "one_2".to_string() };
            let one_3 = One { a: "one_3".to_string() };
            let v = vec![one_1, one_2, one_3];
            v
        };

        println!("Initial values of ptrs: {:p} {:p} {:p} {:p}",
                 string.as_ptr(),
                 v_u8.as_ptr(),
                 v_one.as_ptr(),
                 one.a.as_ptr());

        Two {
            a: string,
            b: v_u8,
            c: v_one,
            d: one,
        }
    };

    let mut two_ffi = two.into_repr_c().unwrap();
    // At this point give to Frontend via callback as `o_cb(&two_ffi);`

    const EXPLICIT_DROP: bool = false;

    if EXPLICIT_DROP {
        let _ = Two::from_repr_c_owned(&mut two_ffi);
        mem::forget(two_ffi);
    } // else it will be implicitly dropped due to Drop impl on TwoFfi
}
