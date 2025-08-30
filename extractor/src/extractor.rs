#![allow(unused)]

use crate::extractor::bridge::PDFHandle;
use cxx::let_cxx_string;
use std::sync::{Arc, Mutex};
use std::{
    os::raw::c_void,
    path::{Path, PathBuf},
    ptr::{self, null_mut},
    slice::from_raw_parts,
    thread::available_parallelism,
};
use tokio::{
    select,
    sync::mpsc::{Receiver, Sender},
    task::{JoinError, JoinSet},
};

#[cfg(feature = "logging")]
use log::error;

#[cxx::bridge]
#[allow(unused)]
mod bridge {
    unsafe extern "C++" {
        include!("D:/coding/synco/pdf_parser_v3/extractor/src_cpp/main.h");
        type PDFHandle;

        unsafe fn init(
            path: &CxxString,
            doc_handle: *mut PDFHandle,
            ctx_handle: *mut PDFHandle,
            pages_buf: *mut i32,
        ) -> Result<()>;

        unsafe fn render_page(
            page_num: i32,
            size_buf: *mut usize,
            width_buf: *mut i32,
            height_buf: *mut i32,
            channels_buf: *mut i32,
            doc_handle: *mut PDFHandle,
            ctx_handle: *mut PDFHandle,
        ) -> Result<*mut u8>; // bytes in PNG format to a picture of the page

        unsafe fn free_image_data(data: *mut u8);

        unsafe fn cleanup_pdf(doc_handle: *mut PDFHandle, ctx_handle: *mut PDFHandle);

        unsafe fn flush_cache(ctx_handle: *mut PDFHandle);

        unsafe fn clone(current_ctx: *mut PDFHandle, new_ctx: *mut PDFHandle) -> Result<()>;

        unsafe fn clone_doc(
            path: &CxxString,
            ctx_handle: *mut PDFHandle,
            new_doc: *mut PDFHandle,
        ) -> Result<()>;
    }
}

pub struct Extractor {
    pub doc_path: PathBuf,
    pub page_count: i32,
    doc_handle: *mut PDFHandle,
    ctx_handle: *mut PDFHandle,
}

#[derive(thiserror::Error, Debug)]
pub enum PageRenderError {
    #[error("Invalid context handle")]
    InvalidContextHandle,

    #[error("Internally passed a nullptr for a buffer!")]
    PassedNullptrToBuffer,

    #[error("Attempted to access a page that doesn't exist!")]
    PageDoesNotExist,

    #[error("{0}")]
    FailedToCreatePixMap(String),

    #[error("{0}")]
    RenderError(String),

    #[error("Passed an unexpected error! {0}")]
    Unexpected(String),
}

impl From<&str> for PageRenderError {
    fn from(value: &str) -> Self {
        if value.eq("Invalid context handle") {
            PageRenderError::InvalidContextHandle
        } else if value.eq("Passed nullptr for a buffer!") {
            PageRenderError::PassedNullptrToBuffer
        } else if value.eq("Attempted to access a page that doesn't exist within this document!") {
            PageRenderError::PageDoesNotExist
        } else if value.eq("Failed to create pixmap: Unknown error") {
            PageRenderError::FailedToCreatePixMap("Unknown Error!".to_string())
        } else if value.contains("Failed to create pixmap:") {
            PageRenderError::FailedToCreatePixMap(value.to_string())
        } else if value.contains("Failed to render page") {
            PageRenderError::RenderError(value.to_string())
        } else {
            PageRenderError::Unexpected(value.to_string())
        }
    }
}

pub enum ControlMessage {
    Stop,
    Pause,
    Resume,
}

macro_rules! debug {
    ($template:expr) => {
        #[cfg(feature = "logging")]
        log::debug!($template);
    };
    ($template:expr, $($args:expr),+ $(,)?) => {
        #[cfg(feature = "logging")]
        log::debug!($template, $($args),+);
    };
}

type MemAddress = usize;
pub type PageNum = i32;
pub type ImageWidth = i32;
pub type ImageHeight = i32;
pub type ImageChannels = i32;

impl Extractor {
    pub fn new(doc_path: impl AsRef<Path>) -> Self {
        let mut doc_handle: *mut c_void = ptr::null_mut();
        let mut ctx_handle: *mut c_void = ptr::null_mut();
        let mut page_count: i32 = 0;
        let_cxx_string!(cxx_str = doc_path.as_ref().to_string_lossy().to_string());

        debug!(
            "Initializing PDF with path: {}",
            doc_path.as_ref().display()
        );

        unsafe {
            bridge::init(
                &cxx_str,
                &mut doc_handle as *mut _ as *mut PDFHandle,
                &mut ctx_handle as *mut _ as *mut PDFHandle,
                &mut page_count as *mut i32,
            )
            .unwrap()
        };

        debug!(
            "After init: doc_handle=0x{:x}, ctx_handle=0x{:x}, page_count={}",
            doc_handle as usize, ctx_handle as usize, page_count
        );

        let result = Self {
            doc_path: doc_path.as_ref().to_path_buf(),
            doc_handle: doc_handle as *mut _ as *mut PDFHandle,
            ctx_handle: ctx_handle as *mut _ as *mut PDFHandle,
            page_count,
        };

        debug!(
            "Created extractor with ctx_handle: 0x{:x}",
            result.ctx_handle as usize
        );
        result
    }

    pub async unsafe fn iter_pages<F, State>(
        &mut self,
        callback: F,
        render_callback: Sender<Result<(), PageRenderError>>,
        state: Arc<Mutex<State>>,
        mut controller: Receiver<ControlMessage>,
    ) -> ()
    where
        F: 'static
            + Fn(PageNum, &[u8], ImageWidth, ImageHeight, ImageChannels, Arc<Mutex<State>>) -> ()
            + Send
            + Sync
            + Clone
            + Copy,
        State: Send + 'static,
    {
        debug!("Iterating over pages {}", self.page_count);

        let mut pool: JoinSet<()> = JoinSet::new();
        let mut pages_spawned = 0;
        let mut pages_completed = 0;
        let max_concurrent_pages = self.calc_max_concurrent_pages();

        loop {
            select! {
                // handle any control messages
                msg = controller.recv() => {
                    match msg {
                        Some(ControlMessage::Stop) => {
                            debug!("Received stop signal, cancelling remaining tasks");
                            pool.abort_all();
                            break;
                        }
                        Some(ControlMessage::Pause) => {
                            debug!("Received pause signal, waiting...");

                            loop {
                                match controller.recv().await {
                                    Some(ControlMessage::Resume) => {
                                        debug!("Received resume signal, continuing...");
                                        break;
                                    }
                                    Some(ControlMessage::Stop) => {
                                        debug!("Received stop signal while paused, halting");
                                        pool.abort_all();
                                        return;
                                    }
                                    Some(ControlMessage::Pause) => {
                                        debug!("Already paused, ignoring additional pause signal");
                                    }
                                    None => {
                                        debug!("Control channel closed while paused");
                                        pool.abort_all();
                                        return;
                                    }
                                }
                            }
                        }
                        Some(ControlMessage::Resume) => {
                            debug!("Received resume signal while not paused, ignoring");
                        }
                        None => {
                            debug!("Control channel closed, finishing remaining tasks");
                        }
                    }
                }

                // spawn new page tasks if we have capacity and more pages to process
                _ = async {}, if pages_spawned < self.page_count && pool.len() < max_concurrent_pages => {
                    // Try to keep the pipeline full by spawning multiple tasks at once for better I/O overlap
                    unsafe { self.spawn_tasks(&mut pages_spawned, callback, render_callback.clone(), state.clone(), &mut pool) };
                }

                // wait for task completion
                result = pool.join_next(), if !pool.is_empty() => {
                    self.handle_task_completion(&mut pages_completed, result);
                }

                // all pages were spawned and completed.
                _ = async {}, if pages_spawned >= self.page_count && pool.is_empty() => {
                    debug!("All pages completed!");
                    break;
                }
            }
        }

        debug!("Done iterating over pages!");
    }

    fn calc_max_concurrent_pages(&self) -> usize {
        available_parallelism()
            .map(|p| {
                let cores = p.get();
                // Optimize for better cache locality with smaller batches on more cores
                match cores {
                    1..=4 => cores * 2,           // 2-8 threads for low-core systems
                    5..=8 => cores + 2,           // 7-10 threads for mid-range systems
                    9..=16 => cores,              // 9-16 threads for high-core systems
                    _ => (cores * 3 / 4).min(32), // 0.75x cores, capped at 32 for very high-core systems
                }
            })
            .unwrap_or(4)
    }

    unsafe fn clone_ctx(&self) -> Result<*mut PDFHandle, String> {
        debug!(
            "clone_ctx called with self.ctx_handle: 0x{:x}",
            self.ctx_handle as usize
        );

        if self.ctx_handle.is_null() {
            return Err("Base context handle is null".to_string());
        }

        let mut ctx: *mut c_void = ptr::null_mut();

        unsafe {
            bridge::clone(
                &self.ctx_handle as *const _ as *mut PDFHandle,
                &mut ctx as *mut _ as *mut PDFHandle,
            )
            .map_err(|e| format!("Failed to clone context: {}", e.what()))?;
        }

        if ctx.is_null() {
            return Err("Cloned context is null".to_string());
        }

        debug!("Successfully cloned context: 0x{:x}", ctx as usize);
        Ok(ctx as *mut PDFHandle)
    }

    unsafe fn clone_doc(&self, new_ctx: *mut PDFHandle) -> Result<*mut PDFHandle, String> {
        debug!("clone_doc called");

        let mut doc: *mut c_void = ptr::null_mut();
        let_cxx_string!(cxx_str = self.doc_path.to_string_lossy().to_string());

        unsafe {
            bridge::clone_doc(
                &cxx_str,
                &new_ctx as *const _ as *mut PDFHandle,
                &mut doc as *mut _ as *mut PDFHandle,
            )
            .map_err(|e| format!("Failed to clone document: {}", e.what()))?;
        }

        if doc.is_null() {
            return Err("Cloned document is null".to_string());
        }

        debug!("Successfully cloned document: 0x{:x}", doc as usize);
        Ok(doc as *mut PDFHandle)
    }

    unsafe fn spawn_page<F, State>(
        page: i32,
        callback: F,
        render_callback: Sender<Result<(), PageRenderError>>,
        thread_ctx_addr: MemAddress,
        thread_doc_addr: MemAddress,
        state: Arc<Mutex<State>>,
        join_set: &mut JoinSet<()>,
    ) where
        F: 'static
            + Fn(i32, &[u8], i32, i32, i32, Arc<Mutex<State>>) -> ()
            + Send
            + Sync
            + Clone
            + Copy,
        State: Send + 'static,
    {
        let render_callback_clone = render_callback.clone();
        let callback_clone = callback.clone();

        join_set.spawn(async move {
            let result = tokio::task::spawn_blocking(move || {
                let mut size: usize = 0;
                let mut width: i32 = 0;
                let mut height: i32 = 0;
                let mut channels: i32 = 0;

                unsafe {
                    Self::iter_page(
                        &mut size,
                        &mut width,
                        &mut height,
                        &mut channels,
                        page,
                        callback_clone,
                        state,
                        thread_ctx_addr,
                        thread_doc_addr,
                    )
                }
            })
            .await;

            match result {
                Ok(Ok(())) => {
                    debug!("Page {} processed successfully", page);
                }
                Ok(Err(e)) => {
                    render_callback_clone.send(Err(e)).await.ok();
                }
                Err(join_error) => {
                    render_callback_clone
                        .send(Err(PageRenderError::Unexpected(format!(
                            "Task panicked: {}",
                            join_error
                        ))))
                        .await
                        .ok();
                }
            }
        });
    }

    unsafe fn iter_page<F, State>(
        size: &mut usize,
        width: &mut i32,
        height: &mut i32,
        channels: &mut i32,
        page: i32,
        callback: F,
        state: Arc<Mutex<State>>,
        ctx_handle: MemAddress,
        doc_handle: MemAddress,
    ) -> Result<(), PageRenderError>
    where
        F: 'static
            + Fn(i32, &[u8], i32, i32, i32, Arc<Mutex<State>>) -> ()
            + Send
            + Sync
            + Clone
            + Copy,
    {
        debug!(
            "Iterating over page {} with ctx: 0x{:x}, doc: 0x{:x}",
            page, ctx_handle, doc_handle
        );

        // Validate handles
        if ctx_handle == 0 {
            return Err(PageRenderError::InvalidContextHandle);
        }

        if doc_handle == 0 {
            return Err(PageRenderError::Unexpected(
                "Document handle is null".to_string(),
            ));
        }

        let image = unsafe {
            // Convert MemAddress values back to PDFHandle values and create stack variables
            let ctx_handle_value: *mut c_void = ctx_handle as *mut c_void;
            let doc_handle_value: *mut c_void = doc_handle as *mut c_void;

            debug!(
                "Calling render_page with ctx_handle_value=0x{:x}, doc_handle_value=0x{:x}",
                ctx_handle_value as usize, doc_handle_value as usize
            );

            bridge::render_page(
                page,
                size as *mut usize,
                width as *mut i32,
                height as *mut i32,
                channels as *mut i32,
                &doc_handle_value as *const *mut c_void as *mut PDFHandle,
                &ctx_handle_value as *const *mut c_void as *mut PDFHandle,
            )
        };

        debug!("Attempted to render page from FFI!");

        // Cleanup function for both context and document
        unsafe fn cleanup_ctx_and_doc(ctx_handle: MemAddress, doc_handle: MemAddress) {
            unsafe {
                bridge::cleanup_pdf(doc_handle as *mut PDFHandle, ctx_handle as *mut PDFHandle)
            };
        }

        let image = match image {
            Ok(i) => i,
            Err(e) => {
                let error_msg = e.what().to_string();

                #[cfg(feature = "logging")]
                error!("Failed to render page {}: {}", page, error_msg);

                unsafe {
                    cleanup_ctx_and_doc(ctx_handle, doc_handle);
                    // cleanup of image happens on C++ side.
                };

                // Check for document corruption errors that should terminate processing
                if error_msg.contains("Document corruption detected")
                    || error_msg.contains("corrupted and cannot be processed")
                    || error_msg.contains("object out of range")
                    || error_msg.contains("non-page object in page tree")
                {
                    #[cfg(feature = "logging")]
                    error!(
                        "Document corruption detected at page {}, terminating processing",
                        page
                    );

                    return Err(PageRenderError::Unexpected(format!(
                        "Document corruption detected at page {}: {}",
                        page, error_msg
                    )));
                }

                return Err(PageRenderError::Unexpected(error_msg));
            }
        };

        debug!("Rendered page!");

        let image_slice = unsafe { from_raw_parts(image, *size) };

        debug!("Converted page to a slice! calling callback function...");

        callback(page, image_slice, *width, *height, *channels, state);

        debug!("Successfully called callback function! freeing image data.");

        unsafe { bridge::free_image_data(image) };

        debug!("Image data freed!");
        if page % 10 == 0 {
            debug!("Flushing MuPDF cache! (every 10 pages)");

            unsafe {
                bridge::flush_cache(ctx_handle as *mut PDFHandle);
            }

            debug!("Flushed MuPDF cache!");
        }

        unsafe {
            cleanup_ctx_and_doc(ctx_handle, doc_handle);
        };
        Ok(())
    }

    fn handle_task_completion(
        &self,
        pages_completed: &mut i32,
        result: Option<Result<(), JoinError>>,
    ) -> () {
        match result {
            Some(Ok(())) => {
                *pages_completed += 1;
                debug!(
                    "Page task completed. Progress: {}/{}",
                    pages_completed, self.page_count
                );
            }
            Some(Err(e)) if e.is_cancelled() => {
                debug!("Page task was cancelled");
            }
            #[allow(unused)]
            Some(Err(e)) => {
                #[cfg(feature = "logging")]
                error!("Page task failed: {}", e);
            }
            None => {
                unreachable!("Join set should never be unexpectedly empty!")
            }
        }
    }

    unsafe fn spawn_tasks<F, State>(
        &self,
        pages_spawned: &mut i32,
        callback: F,
        render_callback: Sender<Result<(), PageRenderError>>,
        state: Arc<Mutex<State>>,
        pool: &mut JoinSet<()>,
    ) -> ()
    where
        F: 'static
            + Fn(i32, &[u8], i32, i32, i32, Arc<Mutex<State>>) -> ()
            + Send
            + Sync
            + Clone
            + Copy,
        State: Send + 'static,
    {
        // Process pages in batches for better cache locality
        const BATCH_SIZE: i32 = 4;
        let available_slots = self.calc_max_concurrent_pages() - pool.len();
        let batch_count = std::cmp::min(available_slots, BATCH_SIZE as usize) as i32;

        for _ in 0..batch_count {
            let page = *pages_spawned;
            if page >= self.page_count {
                return;
            }

            *pages_spawned += 1;

            debug!("Spawning task for page {}", page);

            // First clone the context
            let thread_ctx_addr = match unsafe { self.clone_ctx() } {
                Ok(ctx) => {
                    debug!("Successfully cloned context for page {}", page);
                    ctx as MemAddress
                }
                Err(e) => {
                    #[cfg(feature = "logging")]
                    log::error!("Failed to clone context for page {}: {}", page, e);

                    // Use blocking send to ensure the error is delivered
                    let render_callback_clone = render_callback.clone();
                    tokio::spawn(async move {
                        render_callback_clone
                            .send(Err(PageRenderError::Unexpected(e)))
                            .await
                            .ok();
                    });
                    continue;
                }
            };

            // Then clone the document using the new context
            let thread_doc_addr = match unsafe { self.clone_doc(thread_ctx_addr as *mut PDFHandle) }
            {
                Ok(doc) => {
                    debug!("Successfully cloned document for page {}", page);
                    doc as MemAddress
                }
                Err(e) => {
                    #[cfg(feature = "logging")]
                    log::error!("Failed to clone document for page {}: {}", page, e);

                    // Cleanup the context we just created
                    unsafe {
                        bridge::cleanup_pdf(
                            null_mut() as *mut PDFHandle,
                            thread_ctx_addr as *mut PDFHandle,
                        );
                    }

                    // Use blocking send to ensure the error is delivered
                    let render_callback_clone = render_callback.clone();
                    tokio::spawn(async move {
                        render_callback_clone
                            .send(Err(PageRenderError::Unexpected(e)))
                            .await
                            .ok();
                    });
                    continue;
                }
            };

            debug!(
                "Spawning page {} with ctx_addr: 0x{:x}, doc_addr: 0x{:x}",
                page, thread_ctx_addr, thread_doc_addr
            );

            unsafe {
                Self::spawn_page(
                    page,
                    callback,
                    render_callback.clone(),
                    thread_ctx_addr,
                    thread_doc_addr,
                    state.clone(),
                    pool,
                )
            };
        }
    }
}

impl Drop for Extractor {
    fn drop(&mut self) {
        if !self.doc_handle.is_null() || !self.ctx_handle.is_null() {
            unsafe {
                bridge::cleanup_pdf(
                    if self.doc_handle.is_null() {
                        null_mut()
                    } else {
                        self.doc_handle
                    },
                    if self.ctx_handle.is_null() {
                        null_mut()
                    } else {
                        self.ctx_handle
                    },
                );
            }
        }
    }
}
