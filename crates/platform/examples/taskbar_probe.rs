//! Diagnostics for window taskbar identity.
//! `taskbar_probe <hwnd> get | clear | set <app-id>`

fn main() {
    #[cfg(windows)]
    {
        use launcher_platform::win::{com_init, taskbar};
        com_init();
        let args: Vec<String> = std::env::args().skip(1).collect();
        let hwnd: isize = args[0].parse().expect("hwnd");
        match args.get(1).map(String::as_str) {
            Some("clear") => println!("{:?}", taskbar::clear(hwnd)),
            Some("set") => println!(
                "{:?}",
                taskbar::apply(hwnd, &taskbar::WindowIdentity { app_id: args[2].clone(), relaunch_command: None, display_name: String::new(), icon: None })
            ),
            _ => {}
        }
        println!("current: {:?}", taskbar::current_app_id(hwnd));
    }
}
