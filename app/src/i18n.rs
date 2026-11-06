//! The few strings the Rust side shows itself (tray menu, notifications). Everything else is
//! translated in the UI.

use launcher_core::Settings;

pub fn lang(settings: &Settings) -> &'static str {
    match settings.language.as_str() {
        "ko" => "ko",
        "en" => "en",
        _ => {
            let sys = sys_locale::get_locale().unwrap_or_default().to_lowercase();
            if sys.starts_with("ko") {
                "ko"
            } else {
                "en"
            }
        }
    }
}

pub fn t(lang: &str, key: &str) -> String {
    let ko = lang == "ko";
    let s = match key {
        "open" => if ko { "AI 런처 열기" } else { "Open AI Launcher" },
        "quick" => if ko { "빠른 실행" } else { "Quick launch" },
        "float.show" => if ko { "플로팅 런처 표시" } else { "Show floating launcher" },
        "float.hide" => if ko { "플로팅 런처 숨기기" } else { "Hide floating launcher" },
        "instances" => if ko { "인스턴스" } else { "Instances" },
        "settings" => if ko { "설정" } else { "Settings" },
        "quit" => if ko { "종료" } else { "Quit" },
        "running" => if ko { "실행 중" } else { "running" },
        "empty" => if ko { "인스턴스 없음" } else { "No instances" },
        "notify.low.title" => if ko { "사용량이 거의 찼어요" } else { "Usage almost used up" },
        "notify.reset.title" => if ko { "사용량이 초기화됐어요" } else { "Usage reset" },
        _ => key,
    };
    s.to_string()
}
