// use crate::extractor::{ControlMessage, Extractor, PageRenderError};
// use std::env;
// use std::fs::File;
// use std::io::Write;
// use std::path::PathBuf;
// use std::sync::{Arc, Mutex};
// use tokio::sync::mpsc::channel;

pub mod classifier;
pub mod config;
pub mod extractor;
pub mod pattern;

#[derive(Clone, Copy)]
struct Doc;

pub struct State {
    doc: Doc,
}

// type SafeState = Arc<Mutex<State>>;
// #[tokio::main]
// async fn main() {
//     if env::var("RUST_LOG").is_err() {
//         unsafe { env::set_var("RUST_LOG", "debug") };
//     }
//     env_logger::init();

//     let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
//     let mut extractor = Extractor::new(root.join("data/test.pdf"));
//     let (render_sender, mut render_receiver) = channel::<Result<(), PageRenderError>>(100);
//     let (_, controller_receiver) = channel::<ControlMessage>(10);

//     fn callback(
//         page: i32,
//         img: &[u8],
//         width: i32,
//         height: i32,
//         channels: i32,
//         state: SafeState,
//     ) -> () {
//         let output = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("out");
//         std::fs::create_dir_all(&output).unwrap();
//         let mut file = File::create(output.join(format!("page_{}.png", page))).unwrap();
//         file.write_all(img).unwrap();

//         println!(
//             "Created image {} with width {} height {} and channels {}",
//             page, width, height, channels
//         );
//     }

//     let state = Arc::new(Mutex::new(State { doc: Doc {} }));
//     let start_time = std::time::Instant::now();
//     // Use tokio::select! to handle both processing and errors concurrently
//     let processing_task = async {
//         unsafe {
//             extractor
//                 .iter_pages(callback, render_sender, state, controller_receiver)
//                 .await;
//         }
//     };

//     let error_handling_task = async {
//         let mut success_count = 0;
//         let mut error_count = 0;

//         while let Some(result) = render_receiver.recv().await {
//             match result {
//                 Ok(()) => {
//                     success_count += 1;
//                     println!("✅ Page rendered successfully (total: {})", success_count);
//                 }
//                 Err(e) => {
//                     error_count += 1;
//                     eprintln!(
//                         "❌ Error rendering page: {} (total errors: {})",
//                         e, error_count
//                     );
//                 }
//             }
//         }

//         println!(
//             "Final results: {} successful, {} errors",
//             success_count, error_count
//         );
//     };

//     // Run both tasks concurrently
//     tokio::join!(processing_task, error_handling_task);

//     let end = start_time.elapsed();
//     println!("PDF processing complete! took {:?}", end);
// }
