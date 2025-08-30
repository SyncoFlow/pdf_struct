use cxx_build;
use std::env::var;

fn main() {
    let root = var("CARGO_MANIFEST_DIR").unwrap();
    let toolchain_file = format!("{}/vcpkg/scripts/buildsystems/vcpkg.cmake", root.clone());

    cxx_build::bridge("src/extractor.rs")
        .file("src_cpp/main.cpp")
        .std("c++20")
        .include("build/vcpkg_installed/x64-windows/include")
        .cpp(true)
        .compile("pdf_parser_v3");

    let cfg = cmake::Config::new(".")
        .configure_arg(format!("-DCMAKE_TOOLCHAIN_FILE={}", toolchain_file))
        .build();

    println!("cargo:rustc-link-search=native={}/lib", cfg.display());
    println!("cargo:rustc-link-search=native=build/vcpkg_installed/x64-windows/lib");
    println!("cargo:rustc-link-search=native={}/build/vcpkg_installed/x64-windows/lib", root.clone());
    println!("cargo:rustc-link-lib=static=libmupdf");
    println!("cargo:rustc-link-lib=static=binding");
    println!("cargo:rerun-if-changed=src_cpp/main.cpp");
    println!("cargo:rerun-if-changed=CMakeLists.txt");
}
