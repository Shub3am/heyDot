fn main() {
    // dot-screen's Swift bridge needs the system Swift runtime at launch; its own rpath link arg
    // does not reach this binary, so the app would abort on libswift_Concurrency.dylib.
    println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
    tauri_build::build()
}
