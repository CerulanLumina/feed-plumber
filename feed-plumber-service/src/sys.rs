use std::{
    cell::OnceCell,
    collections::HashMap,
    ffi::{c_void, CStr, CString},
    fmt::{Debug, Formatter},
    ptr::NonNull,
    slice::from_raw_parts,
};

use log::{debug, warn};
use tap::TapFallible;

use sys_feed_plumber_plugin::{
    FeedPlumberPlugin, FeedPlumberProcessorMeta, FeedPlumberSinkMeta, FeedPlumberSourceMeta, Item,
    Items as ItemsRaw, KeyValuePair,
};

#[allow(dead_code)]
pub struct Plugin {
    sources: HashMap<String, PluginSourceMeta>,
    sinks: HashMap<String, PluginSinkMeta>,
    processors: HashMap<String, PluginProcessorMeta>,
    inner: FeedPlumberPlugin,
}

impl Debug for Plugin {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Plugin{{ sources: {:?}, sinks: {:?} }}",
            self.sources.keys().collect::<Vec<_>>(),
            self.sinks.keys().collect::<Vec<_>>()
        )
    }
}

impl Plugin {
    pub fn from_raw(raw: FeedPlumberPlugin) -> Self {
        Self {
            sources: Self::sources(&raw).map(|a| (a.name.clone(), a)).collect(),
            sinks: Self::sinks(&raw).map(|a| (a.name.clone(), a)).collect(),
            processors: Self::processors(&raw)
                .map(|a| (a.name.clone(), a))
                .collect(),
            inner: raw,
        }
    }

    pub fn instantiate_source(
        &self,
        r#type: &str,
        name: String,
        config: &str,
    ) -> Option<PluginSourceInstance> {
        self.sources
            .get(r#type)
            .map(|a| a.instantiate_new(name, config))
    }

    pub fn instantiate_sink(
        &self,
        r#type: &str,
        name: String,
        config: &str,
    ) -> Option<PluginSinkInstance> {
        self.sinks
            .get(r#type)
            .map(|a| a.instantiate_new(name, config))
    }

    pub fn instantiate_processor(
        &self,
        r#type: &str,
        name: String,
        config: &str,
    ) -> Option<PluginProcessorInstance> {
        self.processors
            .get(r#type)
            .map(|a| a.instantiate_new(name, config))
    }

    pub fn supplies_source(&self, src: &str) -> bool {
        self.sources.contains_key(src)
    }

    pub fn supplies_sink(&self, sink: &str) -> bool {
        self.sinks.contains_key(sink)
    }

    pub fn supplies_processor(&self, processor: &str) -> bool {
        self.processors.contains_key(processor)
    }

    fn sources(plugin: &FeedPlumberPlugin) -> impl Iterator<Item = PluginSourceMeta> {
        // Safety: We are relying on FFI to be good, but sources_len corresponds to sources
        let sources_slice = unsafe { slice_from_ptr(plugin.sources, plugin.sources_len) };
        debug!("registering {} sources", sources_slice.len());
        sources_slice
            .iter()
            .filter_map(|a| {
                a.name
                    .as_cstr()
                    .to_str()
                    .tap_err(|e| warn!("Registering source failed: invalid UTF-8. {}", e))
                    .ok()
                    .map(|name| (a, name.to_owned()))
            })
            .map(|(inner, name)| PluginSourceMeta {
                name,
                inner: *inner,
            })
    }

    fn sinks(plugin: &FeedPlumberPlugin) -> impl Iterator<Item = PluginSinkMeta> {
        // Safety: We are relying on FFI to be good, but sinks_len corresponds to sinks
        let sinks_slice = unsafe { slice_from_ptr(plugin.sinks, plugin.sinks_len) };
        sinks_slice
            .iter()
            .filter_map(|a| {
                a.name
                    .as_cstr()
                    .to_str()
                    .ok()
                    .map(|name| (a, name.to_owned()))
            })
            .map(|(inner, name)| PluginSinkMeta {
                name,
                inner: *inner,
            })
    }

    fn processors(plugin: &FeedPlumberPlugin) -> impl Iterator<Item = PluginProcessorMeta> {
        // Safety: We are relying on FFI to be good, but processors_len corresponds to processors
        let processors_slice = unsafe { slice_from_ptr(plugin.processors, plugin.processors_len) };
        processors_slice
            .iter()
            .filter_map(|a| {
                a.name
                    .as_cstr()
                    .to_str()
                    .ok()
                    .map(|name| (a, name.to_owned()))
            })
            .map(|(inner, name)| PluginProcessorMeta {
                name,
                inner: *inner,
            })
    }
}

unsafe fn slice_from_ptr<'a, T>(ptr: *const T, mut len: usize) -> &'a [T] {
    let ptr = if len > 0 && !ptr.is_null() {
        ptr
    } else {
        len = 0;
        NonNull::dangling().as_ptr()
    };
    // Safety: we checked that the ptr is not null (or if it is, we are using zero-length slice),
    // and that we are properly aligned if slice is zero-length.
    from_raw_parts(ptr, len)
}

pub struct PluginSourceMeta {
    pub name: String,
    inner: FeedPlumberSourceMeta,
}

impl PluginSourceMeta {
    pub fn instantiate_new(&self, name: String, config: &str) -> PluginSourceInstance {
        let config = CString::new(config).unwrap();
        // Safety: FFI
        let handle = unsafe { (self.inner.create)(config.as_ptr()) };
        drop(config); // Ensures config lives at least as long as FFI
        PluginSourceInstance {
            name,
            handle,
            meta: self.inner,
        }
    }
}

#[allow(dead_code)]
pub struct PluginSourceInstance {
    name: String,
    handle: *mut c_void,
    meta: FeedPlumberSourceMeta,
}

impl PluginSourceInstance {
    #[allow(dead_code)]
    pub fn name(&self) -> &str {
        &self.name
    }
    fn poll_source_raw(&mut self) -> ItemsRaw {
        // Safety: FFI
        unsafe { (self.meta.poll_source)(self.handle) }
    }

    pub fn poll_source(&mut self) -> Items {
        Items::from_raw(self.poll_source_raw())
    }
}

pub struct PluginSinkMeta {
    pub name: String,
    inner: FeedPlumberSinkMeta,
}

impl PluginSinkMeta {
    pub fn instantiate_new(&self, name: String, config: &str) -> PluginSinkInstance {
        let config = CString::new(config).unwrap();
        // Safety: FFI
        let handle = unsafe { (self.inner.create)(config.as_ptr()) };
        drop(config); // Ensures config lives at least as long as FFI
        PluginSinkInstance {
            name,
            handle,
            meta: self.inner,
        }
    }
}

pub struct PluginSinkInstance {
    name: String,
    handle: *mut c_void,
    meta: FeedPlumberSinkMeta,
}

impl PluginSinkInstance {
    pub fn name(&self) -> &str {
        &self.name
    }
    fn sink_items_raw(&mut self, items: ItemsRaw) {
        // Safety: FFI
        unsafe { (self.meta.sink_items)(self.handle, items) };
    }

    pub fn sink_items(&mut self, items: &Items) {
        items.with_raw(|items| {
            self.sink_items_raw(items);
        });
    }
}

pub struct PluginProcessorMeta {
    pub name: String,
    inner: FeedPlumberProcessorMeta,
}

impl PluginProcessorMeta {
    pub fn instantiate_new(&self, name: String, config: &str) -> PluginProcessorInstance {
        let config = CString::new(config).unwrap();
        // Safety: FFI
        let handle = unsafe { (self.inner.create)(config.as_ptr()) };
        drop(config); // Ensures config lives at least as long as FFI
        PluginProcessorInstance {
            name,
            handle,
            meta: self.inner,
        }
    }
}

pub struct PluginProcessorInstance {
    name: String,
    handle: *mut c_void,
    meta: FeedPlumberProcessorMeta,
}

impl PluginProcessorInstance {
    pub fn name(&self) -> &str {
        &self.name
    }
    fn process_items_raw(&mut self, items: ItemsRaw) -> ItemsRaw {
        // Safety: FFI
        unsafe { (self.meta.process_items)(self.handle, items) }
    }

    pub fn process_items(&mut self, items: &Items) -> Items {
        let cell = OnceCell::new();
        items.with_raw(|items| {
            let items = self.process_items_raw(items);
            cell.set(Items::from_raw(items)).ok().unwrap();
        });
        cell.into_inner().unwrap()
    }
}

#[derive(Clone)]
pub struct Items(Vec<Vec<(String, String)>>);

impl Items {
    #[allow(dead_code)]
    pub fn items(&self) -> impl Iterator<Item = &Vec<(String, String)>> {
        self.0.iter()
    }

    fn from_raw(raw: ItemsRaw) -> Self {
        let ret = if raw.items.is_null() || raw.len == 0 {
            return Self(Vec::new());
        } else {
            let mut items_out = Vec::new();
            // Safety: We checked for null and length above
            let items = unsafe { from_raw_parts(raw.items, raw.len) };
            for item in items {
                let mut pairs_out = Vec::new();
                if !item.key_values.is_null() && item.len > 0 {
                    let pairs = unsafe { from_raw_parts(item.key_values, item.len) };
                    for pair in pairs {
                        if !pair.key.is_null() && !pair.value.is_null() {
                            let key = unsafe { CStr::from_ptr(pair.key) };
                            let value = unsafe { CStr::from_ptr(pair.value) };
                            if let Ok((key, value)) =
                                key.to_str().and_then(|a| value.to_str().map(|b| (a, b)))
                            {
                                pairs_out.push((key.to_owned(), value.to_owned()));
                            }
                        }
                        // Safety: FFI
                        unsafe { (pair.destroy)(pair.key, pair.value) };
                    }
                    items_out.push(pairs_out);
                }
                // Safety: FFI
                unsafe { (item.destroy)(item.key_values, item.len) };
            }
            items_out
        };
        // Safety: FFI
        unsafe { (raw.destroy)(raw.items, raw.len) };
        Self(ret)
    }

    fn with_raw(&self, f: impl FnOnce(ItemsRaw)) {
        let mut outer = Vec::with_capacity(self.0.len());
        for item in &self.0 {
            let mut inner = Vec::with_capacity(item.len());
            for (key, value) in item {
                let key = CString::new(key.as_str()).unwrap().into_raw();
                let value = CString::new(value.as_str()).unwrap().into_raw();
                inner.push(KeyValuePair {
                    key,
                    value,
                    destroy: no_op::<_, _>,
                });
            }
            let mut inner = inner.into_boxed_slice();
            let ptr = inner.as_mut_ptr();
            let len = inner.len();
            std::mem::forget(inner);
            outer.push(Item {
                key_values: ptr,
                len,
                destroy: no_op::<_, _>,
            });
        }
        let mut outer = outer.into_boxed_slice();
        let ptr = outer.as_mut_ptr();
        let len = outer.len();
        let items_raw = ItemsRaw {
            items: ptr,
            len,
            destroy: no_op::<_, _>,
        };

        f(items_raw);

        unsafe {
            for item in outer.into_vec() {
                for pair in Vec::from_raw_parts(item.key_values, item.len, item.len) {
                    drop(CString::from_raw(pair.key));
                    drop(CString::from_raw(pair.value));
                }
            }
        }
    }
}

unsafe extern "C" fn no_op<A, B>(_: A, _: B) {}
