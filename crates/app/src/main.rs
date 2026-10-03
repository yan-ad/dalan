#[cfg(not(target_os = "macos"))]
compile_error!(
    "The desktop bootstrap currently targets macOS only. Core tests support other hosts."
);

#[cfg(target_os = "macos")]
mod desktop;

#[cfg(target_os = "macos")]
fn main() {
    desktop::run();
}
