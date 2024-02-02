use serde::Deserialize;
use std::ffi::{c_char, c_void, CStr};

pub trait FeedPlumberSource: Sized + 'static {
    type ConfigType: for<'a> Deserialize<'a> + Default;
    fn new(config: Self::ConfigType) -> Self;
    fn poll_source(&mut self) -> Vec<Vec<(String, String)>>;
}

pub trait FeedPlumberSink: Sized + 'static {
    type ConfigType: for<'a> Deserialize<'a> + Default;
    fn new(config: Self::ConfigType) -> Self;
    fn sink_items(&mut self, items: Vec<Vec<(&str, &str)>>);
}

pub trait FeedPlumberProcessor: Sized + 'static {
    type ConfigType: for<'a> Deserialize<'a> + Default;
    fn new(config: Self::ConfigType) -> Self;
    fn process_items(&mut self, items: Vec<Vec<(&str, &str)>>) -> Vec<Vec<(String, String)>>;
}

pub unsafe extern "C" fn source_create<T: FeedPlumberSource>(config: *const c_char) -> *mut c_void {
    let cstr = CStr::from_ptr(config);
    let config = cstr.to_str().unwrap();
    let config =
        toml::from_str::<T::ConfigType>(config).unwrap_or_else(|_| T::ConfigType::default());
    Box::into_raw(Box::new(T::new(config))) as _
}

pub unsafe extern "C" fn sink_create<T: FeedPlumberSink>(config: *const c_char) -> *mut c_void {
    let cstr = CStr::from_ptr(config);
    let config = cstr.to_str().unwrap();
    let config =
        toml::from_str::<T::ConfigType>(config).unwrap_or_else(|_| T::ConfigType::default());
    Box::into_raw(Box::new(T::new(config))) as _
}

pub unsafe extern "C" fn processor_create<T: FeedPlumberProcessor>(
    config: *const c_char,
) -> *mut c_void {
    let cstr = CStr::from_ptr(config);
    let config = cstr.to_str().unwrap();
    let config =
        toml::from_str::<T::ConfigType>(config).unwrap_or_else(|_| T::ConfigType::default());
    Box::into_raw(Box::new(T::new(config))) as _
}
