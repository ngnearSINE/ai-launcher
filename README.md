# AI Launcher

여러 계정의 Claude 데스크톱과 AI 앱을 나란히 띄우는 런처입니다. Windows와 Ubuntu(각각 x64·arm64)에서 동작합니다.

- **계정마다 한 인스턴스**: 인스턴스마다 데이터 폴더가 따로 있어 로그인이 섞이지 않습니다.
- **작업 표시줄에 따로 고정**(Windows): 인스턴스마다 작업 표시줄 버튼이 따로 생기고, 각각 고정할 수 있습니다. 고정한 버튼은 그 인스턴스를 정확히 띄웁니다.
- **사용량 카드**: Claude의 5시간·주간·모델별(예: Fable)·크레딧 사용량을 인스턴스 카드에 보여 줍니다. 사용량은 **그 인스턴스 프로필의 로그인으로만** 조회합니다.
- **자동 검색 + 경로 등록**: 기본 위치에 설치된 Claude·Orca·Cursor·Poe·Grok Bot과 Chrome에 설치한 AI 웹앱을 찾아 등록합니다. 다른 위치의 프로그램은 경로로 추가할 수 있습니다.
- **웹앱**: Chrome에 설치한 Gemini·Grok 같은 웹앱은 평소 프로필 그대로 기본 인스턴스로 엽니다. 다른 계정은 독립된 브라우저 폴더의 앱 창으로 띄웁니다. Chrome·Edge 브라우저 자체는 카드로 보이지 않습니다.
- **빠른 실행·트레이·플로팅 런처**: 전역 단축키(기본 `Alt+Shift+Space`)로 검색 → Enter. 트레이 메뉴에서 바로 실행할 수도 있고, 항상 위에 떠 있는 아이콘 줄(사용량 고리 표시)로 바꿔 쓸 수 있습니다. 관리 창과 플로팅은 제목줄 단추나 `Alt+Shift+F`로 오갑니다.

## 지원하는 앱

| 종류 | 예 | 계정 분리 방법 |
| --- | --- | --- |
| Claude 데스크톱 | Windows Store판·설치판, Ubuntu `claude-desktop` | `--user-data-dir` |
| Electron 앱 | Orca, Cursor(Ubuntu AppImage 포함), Poe, Grok Bot, Grok Desktop(Ubuntu), 경로로 추가한 Electron 앱 | `--user-data-dir` |
| 웹앱 | Chrome에 설치한 웹앱(Gemini, Grok, ChatGPT 등), Claude 웹, Perplexity, 직접 입력 | 기본 인스턴스는 설치된 웹앱 그대로, 추가 계정은 브라우저 데이터 폴더 + 앱 창 |
| 그 밖의 프로그램 | 경로로 추가 | 실행만 관리(원하면 `--user-data-dir` 켜기) |

## 사용법

1. 처음 실행하면 설치된 앱을 찾아 카드로 보여 줍니다. 앱마다 기본 인스턴스(평소 로그인)가 하나씩 생깁니다.
2. **새 인스턴스**를 눌러 앱을 고르고 별명과 색을 정합니다. 그 인스턴스는 새 폴더에서 시작하므로 다른 계정으로 로그인하면 됩니다.
3. 인스턴스를 실행한 뒤 작업 표시줄에 생긴 버튼을 우클릭해 **작업 표시줄에 고정**을 고릅니다. 카드 메뉴의 **시작 메뉴에 추가**로 만든 바로가기도 같은 버튼으로 묶입니다.

세부 동작은 설정에서 바꿉니다. 작업 표시줄 버튼을 나눌지 합칠지, 색 배지, 고정 버튼 실행 방식은 **설정 → 작업 표시줄**에서 정합니다. 인스턴스마다 **편집 → 고급**에서 따로 바꿀 수도 있습니다.

### 단축키

| 키 | 동작 |
| --- | --- |
| `Alt+Shift+Space` (전역) | 빠른 실행 |
| `Ctrl+N` | 새 인스턴스 |
| `/`, `Ctrl+F` | 검색 |
| `1`–`9` | n번째 인스턴스 실행 |
| `F2` / `Del` | 편집 / 삭제 |
| `Ctrl+,` | 설정 |

## 개인정보

- 각 인스턴스의 로그인 정보는 그 프로필 폴더 안에 암호화돼 있습니다. 런처는 사용량을 조회할 때만 이 PC에서 복호화하고(Windows DPAPI, Linux 키링), 저장하거나 기록하지 않습니다.
- 네트워크 요청은 `api.anthropic.com`의 사용량·프로필 조회뿐입니다. 사용량 조회는 설정에서 끌 수 있습니다.

## 빌드

Rust(stable)와 Node.js 20 이상이 필요합니다.

```powershell
.\local-build.ps1 -Test        # UI·Rust 검사 + 릴리스 실행 파일
.\local-build.ps1 -Bundle      # Windows 설치 파일
```

Ubuntu:

```bash
sudo apt install libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev
cd ui && npm ci && npm run build && cd ../app && npx --prefix ../ui tauri build --bundles deb appimage
```

## 구조

```
crates/core      앱 종류(어댑터)·설치본·인스턴스·설정 저장 — OS와 무관
crates/usage     프로필 하나의 로그인으로 사용량 조회
crates/platform  프로세스·창·작업 표시줄 ID·바로가기·배지 아이콘
app              Tauri 앱: 실행/감시/트레이/단축키/창
ui               Svelte 5 화면(메인·빠른 실행·플로팅)
```

## 라이선스

MIT
