use std::ffi::{c_char, c_void, CStr};

pub type InitializationFunction = unsafe extern "C" fn() -> FeedPlumberPlugin;

pub const INITIALIZATION_FUNCTION_NAME: &str = "_feedplumber_plugin_init";

#[repr(C)]
pub struct KeyValuePair {
    pub key: *mut c_char,
    pub value: *mut c_char,
    pub destroy: unsafe extern "C" fn(*mut c_char, *mut c_char),
}

#[repr(C)]
pub struct Item {
    pub key_values: *mut KeyValuePair,
    pub len: usize,
    pub destroy: unsafe extern "C" fn(*mut KeyValuePair, usize),
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct Items {
    pub items: *mut Item,
    pub len: usize,
    pub destroy: unsafe extern "C" fn(*mut Item, usize),
}

/// Strings that should be valid for static lifetime
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct StaticString {
    inner: *const c_char,
}

// SAFETY: Struct is opaque and contains only a static str as ptr
unsafe impl Sync for StaticString {}
unsafe impl Send for StaticString {}

impl StaticString {
    pub const fn from_static(s: &'static CStr) -> StaticString {
        StaticString { inner: s.as_ptr() }
    }

    pub fn as_cstr(&self) -> &CStr {
        unsafe { CStr::from_ptr(self.inner) }
    }
}

#[repr(C)]
pub struct FeedPlumberPlugin {
    pub sources: *const FeedPlumberSourceMeta,
    pub sources_len: usize,
    pub sinks: *const FeedPlumberSinkMeta,
    pub sinks_len: usize,
    pub processors: *const FeedPlumberProcessorMeta,
    pub processors_len: usize,
}
unsafe impl Sync for FeedPlumberPlugin {}
unsafe impl Send for FeedPlumberPlugin {}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct FeedPlumberSourceMeta {
    pub name: StaticString,
    pub create: unsafe extern "C" fn(*const c_char) -> *mut c_void,
    pub poll_source: unsafe extern "C" fn(*mut c_void) -> Items,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct FeedPlumberSinkMeta {
    pub name: StaticString,
    pub create: unsafe extern "C" fn(*const c_char) -> *mut c_void,
    pub sink_items: unsafe extern "C" fn(*mut c_void, Items),
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct FeedPlumberProcessorMeta {
    pub name: StaticString,
    pub create: unsafe extern "C" fn(*const c_char) -> *mut c_void,
    pub process_items: unsafe extern "C" fn(*mut c_void, Items) -> Items,
}
