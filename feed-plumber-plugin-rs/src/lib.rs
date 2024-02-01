use crate::raw::{items_destroy, key_values_to_item, vec_to_raw};
use std::ffi::{c_char, c_void, CStr};
use sys_feed_plumber_plugin::Items;

pub mod sys {
    pub use cstr::cstr;
    pub use sys_feed_plumber_plugin::*;
}

pub trait FeedPlumberSource: Sized + 'static {
    fn new() -> Self;
    fn poll_source(&mut self) -> Vec<Vec<(String, String)>>;
}

pub trait FeedPlumberSink: Sized + 'static {
    fn new() -> Self;
    fn sink_items(&mut self, items: Vec<Vec<(&str, &str)>>);
}

#[macro_export]
macro_rules! feed_plumber_plugin {
    {
        sources: $($source_name:literal => $source_ty:ty),*;
        sinks: $($sink_name:literal => $sink_ty:ty),*;
    } => {
        #[no_mangle]
        pub extern "C" fn _feedplumber_plugin_init() -> $crate::sys::FeedPlumberPlugin {
            let sources = Box::leak(Box::new([$($crate::sys::FeedPlumberSourceMeta {
                name: $crate::sys::StaticString::from_static($crate::sys::cstr!($source_name)),
                create: $crate::source_create::<$source_ty>,
                poll_source: $crate::source_poll_source::<$source_ty>,
            },),*]));
            let sinks = Box::leak(Box::new([$($crate::sys::FeedPlumberSinkMeta {
                name: $crate::sys::StaticString::from_static($crate::sys::cstr!($sink_name)),
                create: $crate::sink_create::<$sink_ty>,
                sink_items: $crate::sink_sink_items::<$sink_ty>,
            },),*]));
            $crate::sys::FeedPlumberPlugin {
                sources: sources.as_ptr(),
                sources_len: sources.len(),
                sinks: sinks.as_ptr(),
                sinks_len: sinks.len(),
            }
        }
    };
    {
        sources: $($source_name:literal => $source_ty:ty),*;
    } => {
        feed_plumber_plugin! {
            sources: $($source_name => $source_ty),*;
            sinks:;
        }
    };
    {
        sinks: $($sink_name:literal => $sink_ty:ty),*;
    } => {
        feed_plumber_plugin! {
            sources:;
            sinks: $($sink_name => $sink_ty),*;
        }
    };
    {
        sinks: $($sink_name:literal => $sink_ty:ty),*;
        sources: $($source_name:literal => $source_ty:ty),*;
    } => {
        feed_plumber_plugin! {
            sources: $($source_name => $source_ty),*;
            sinks: $($sink_name => $sink_ty),*;
        }
    }
}

pub unsafe extern "C" fn source_poll_source<T: FeedPlumberSource>(handle: *mut c_void) -> Items {
    let source = &mut *(handle as *mut T);
    let pairs = source.poll_source();
    let items = pairs
        .into_iter()
        .map(key_values_to_item)
        .collect::<Vec<_>>();
    let (ptr, len) = vec_to_raw(items);
    Items {
        items: ptr,
        len,
        destroy: items_destroy,
    }
}

pub unsafe extern "C" fn source_create<T: FeedPlumberSource>(_: *const c_char) -> *mut c_void {
    Box::into_raw(Box::new(T::new())) as _
}

pub unsafe extern "C" fn sink_create<T: FeedPlumberSink>(_: *const c_char) -> *mut c_void {
    Box::into_raw(Box::new(T::new())) as _
}

pub unsafe extern "C" fn sink_sink_items<T: FeedPlumberSink>(handle: *mut c_void, items: Items) {
    let sink = &mut *(handle as *mut T);
    let items_slice = std::slice::from_raw_parts(items.items, items.len);
    let mut outer_vec = Vec::new();
    for item in items_slice {
        let mut inner_vec = Vec::new();
        let key_value_slice = std::slice::from_raw_parts(item.key_values, item.len);
        for key_value in key_value_slice {
            let key = CStr::from_ptr(key_value.key).to_str().unwrap();
            let value = CStr::from_ptr(key_value.value).to_str().unwrap();
            inner_vec.push((key, value));
        }
        outer_vec.push(inner_vec);
    }
    sink.sink_items(outer_vec);
}

pub mod raw {
    use std::{
        ffi::{c_char, CString},
        mem::forget,
    };
    use sys_feed_plumber_plugin::{Item, KeyValuePair};

    pub unsafe extern "C" fn pair_destroy(ptr1: *mut c_char, ptr2: *mut c_char) {
        drop(CString::from_raw(ptr1));
        drop(CString::from_raw(ptr2));
    }

    pub unsafe extern "C" fn item_destroy(ptr: *mut KeyValuePair, len: usize) {
        drop(Vec::from_raw_parts(ptr, len, len));
    }

    pub unsafe extern "C" fn items_destroy(ptr: *mut Item, len: usize) {
        drop(Vec::from_raw_parts(ptr, len, len));
    }

    pub fn string_to_pointer(a: String) -> *mut c_char {
        CString::new(a).unwrap().into_raw()
    }

    pub fn dual_string_to_pointer(a: (String, String)) -> (*mut c_char, *mut c_char) {
        (string_to_pointer(a.0), string_to_pointer(a.1))
    }

    pub fn key_values_to_item(pairs: Vec<(String, String)>) -> Item {
        let ret = pairs
            .into_iter()
            .map(dual_string_to_pointer)
            .map(|(key, value)| KeyValuePair {
                key,
                value,
                destroy: pair_destroy,
            })
            .collect::<Vec<_>>();
        let (key_values, len) = vec_to_raw(ret);
        Item {
            key_values,
            len,
            destroy: item_destroy,
        }
    }

    #[inline]
    pub fn vec_to_raw<T>(v: Vec<T>) -> (*mut T, usize) {
        let mut slice = v.into_boxed_slice();
        let len = slice.len();
        let ptr = slice.as_mut_ptr();
        forget(slice);
        (ptr, len)
    }
}