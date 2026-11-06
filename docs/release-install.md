## 설치 · Install

| OS | 파일 · File | 자동 업데이트 · Auto-update |
| --- | --- | --- |
| Windows 10/11 (x64) | `AI.Launcher_{version}_x64-setup.exe` | ✓ |
| Ubuntu 22.04+ (x64) | `AI.Launcher_{version}_amd64.AppImage` | ✓ |
| Ubuntu 22.04+ (x64) | `AI.Launcher_{version}_amd64.deb` (`sudo apt install ./…deb`) | apt |

- Windows 설치 파일은 관리자 권한 없이 현재 사용자에게 설치됩니다. 처음 실행할 때 SmartScreen 경고가 뜨면 「추가 정보 → 실행」을 누르세요(코드 서명 인증서 없음).
- The Windows installer installs per user without admin rights. If SmartScreen warns on first run, choose "More info → Run anyway" (no code-signing certificate).
- 업데이트 파일은 서명돼 있고 런처가 검증한 뒤 설치합니다. 실행 중인 앱(Claude 등)은 업데이트 중에도 닫히지 않습니다.
