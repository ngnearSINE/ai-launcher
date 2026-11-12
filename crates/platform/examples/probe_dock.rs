//! Diagnostics: the landmarks the docked strip snaps to, for every horizontal taskbar.
//! `cargo run -p launcher-platform --example probe_dock`

fn main() {
    #[cfg(windows)]
    {
        launcher_platform::win::com_init();
        for (tb, r) in launcher_platform::win::dock::taskbars() {
            println!("taskbar {tb:#x} {r:?}");
            println!("  {:?}", launcher_platform::win::dock::landmarks(tb));
        }
    }
}
