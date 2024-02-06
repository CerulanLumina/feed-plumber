use anyhow::Context;
use std::ffi::{c_char, c_void, CStr};
use sys_feed_plumber_plugin::CreationResult;

pub trait FeedPlumberSource: Sized + 'static {
    fn new(config: &str) -> anyhow::Result<Self>;
    fn poll_source(&mut self) -> anyhow::Result<Vec<Vec<(String, String)>>>;
}

pub trait FeedPlumberSink: Sized + 'static {
    fn new(config: &str) -> anyhow::Result<Self>;
    fn sink_items(&mut self, items: Vec<Vec<(&str, &str)>>);
}

pub trait FeedPlumberProcessor: Sized + 'static {
    fn new(config: &str) -> anyhow::Result<Self>;
    fn process_items(
        &mut self,
        items: Vec<Vec<(&str, &str)>>,
    ) -> anyhow::Result<Vec<Vec<(String, String)>>>;
}

pub unsafe extern "C" fn source_create<T: FeedPlumberSource>(
    config: *const c_char,
) -> CreationResult {
    let cstr = CStr::from_ptr(config);
    let config = cstr.to_str().unwrap();
    crate::raw::result_to_creation_result(
        T::new(config)
            .context("Initializing source")
            .context("Creating source"),
    )
}

pub unsafe extern "C" fn sink_create<T: FeedPlumberSink>(config: *const c_char) -> CreationResult {
    let cstr = CStr::from_ptr(config);
    let config = cstr.to_str().unwrap();
    crate::raw::result_to_creation_result(
        T::new(config)
            .context("Initializing sink")
            .context("Creating sink"),
    )
}

pub unsafe extern "C" fn processor_create<T: FeedPlumberProcessor>(
    config: *const c_char,
) -> CreationResult {
    let cstr = CStr::from_ptr(config);
    let config = cstr.to_str().unwrap();
    crate::raw::result_to_creation_result(
        T::new(config)
            .context("Initializing processor")
            .context("Creating processor"),
    )
}
