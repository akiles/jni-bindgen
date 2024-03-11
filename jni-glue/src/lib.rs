//! Common glue code between Rust and JNI, used in autogenerated jni-bindgen glue code.
//!
//! See also the [Android JNI tips](https://developer.android.com/training/articles/perf-jni) documentation as well as the
//! [Java Native Interface Specification](https://docs.oracle.com/javase/7/docs/technotes/guides/jni/spec/jniTOC.html).

// Re-export a few things such that we have a consistent name for them in autogenerated glue code wherever we go.

#[doc(hidden)]
pub use ::jni_sys;
#[doc(hidden)]
pub use ::std;

mod refs {

    mod argument;
    mod global;
    mod local;
    mod ref_;

    pub use argument::*;
    pub use global::*;
    pub use local::*;
    pub use ref_::*;
}

mod __jni_bindgen;
mod array;
mod as_jvalue;
mod as_valid_jobject_and_env;
mod env;
mod jchar_;
mod jni_type;
mod object_and_env;
mod string_chars;
mod throwable_type;
mod vm;

pub use array::*;
pub use as_jvalue::*;
pub use as_valid_jobject_and_env::*;
pub use env::*;
pub use jchar_::jchar;
pub use jni_type::JniType;
pub use object_and_env::*;
pub use refs::*;
pub use string_chars::*;
pub use throwable_type::*;
pub use vm::*;
