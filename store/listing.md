# 스토어 등록 자료 (App Store + Play Store)

콘솔 입력용 마스터. 영어(기본) + 한국어. 글자 수 제한은 각 항목에 표기.

---

## 공통 기본 정보

| 항목 | 값 |
|---|---|
| 앱 이름 (App Store name / Play title, ≤30) | **WIPI Emulator** |
| 홈 화면 표시명 (ko) | WIPI 에뮬 |
| 번들 ID / applicationId | com.parkjeongseop.wipi |
| 버전 | 0.1.0 (build 1) |
| 카테고리 | Games → Arcade |
| 지원 언어 | English (기본), 한국어 |
| 개발자 연락 이메일 | parkjeongseop@parkjeongseop.com |
| 개인정보처리방침 URL | https://github.com/ParkJeongseop/WIPI-Emulator/blob/main/docs/PRIVACY.md |
| 마케팅/지원 URL | https://github.com/ParkJeongseop/WIPI-Emulator |
| 가격 | 무료 (무광고, 인앱결제 없음) |

---

## 짧은 설명

### App Store — Subtitle (≤30자)
- EN: `Korean feature-phone games`
- KO: `한국 피처폰 게임 에뮬레이터`

### App Store — Promotional Text (≤170자, 언제든 갱신 가능)
- EN: `Bring your classic Korean feature-phone games back to life. Load your own WIPI, SKVM, and J2ME titles and play them on your modern device.`
- KO: `추억의 한국 피처폰 게임을 다시 만나보세요. 직접 소유한 WIPI·SKVM·J2ME 게임을 불러와 최신 기기에서 즐길 수 있습니다.`

### Play Store — 간단한 설명 (≤80자)
- EN: `Play classic Korean feature-phone games (WIPI / SKVM / J2ME) you own.`
- KO: `직접 소유한 한국 피처폰 게임(WIPI·SKVM·J2ME)을 실행하는 에뮬레이터.`

---

## 전체 설명 (App Store ≤4000 / Play ≤4000)

### English

```
WIPI Emulator brings classic 2000s Korean feature-phone games back to life on
your modern device.

Load and play mobile games that ran on Korea's WIPI, SKVM, and J2ME platforms —
the titles you owned on KTF, LG Telecom (LGT), and SK Telecom handsets.

FEATURES
- Runs KTF (Clet), LGT, SKT (SKVM), and J2ME game packages (.zip / .jar)
- Game library with automatic cover art and title extraction
- On-screen keypad with haptic feedback, plus hardware keyboard and game
  controller support
- Authentic audio: PCM sound effects and MIDI music via a built-in soundfont,
  with separate volume for music and effects
- Vibration faithful to the original WIPI API
- Pause anytime; the game is frozen while the app is in the background
- Fully offline. No accounts, no ads, no tracking, and no data collection.

IMPORTANT
This app does not include any games. You can only load game files that you
legally own. A free demo game ("Heart Catch") is provided so you can try the
import flow right away.

Built on the open-source wie emulator and RustJava. Released under the MIT
license.
```

### 한국어

```
WIPI 에뮬레이터는 2000년대 한국 피처폰 게임을 최신 기기에서 다시 즐길 수 있게
해줍니다.

KTF, LG텔레콤(LGT), SK텔레콤 단말기에서 돌아가던 WIPI·SKVM·J2ME 게임을 불러와
실행하세요.

주요 기능
- KTF(Clet)·LGT·SKT(SKVM)·J2ME 게임 패키지(.zip / .jar) 실행
- 표지·이름을 자동 추출하는 게임 라이브러리
- 햅틱 피드백이 있는 화면 키패드, 하드웨어 키보드·게임패드 지원
- 원음에 충실한 사운드: PCM 효과음과 내장 사운드폰트 기반 MIDI 음악,
  배경음악과 효과음 볼륨 개별 조절
- 원본 WIPI API에 충실한 진동
- 언제든 일시정지, 백그라운드에서는 게임이 멈춤
- 완전 오프라인. 계정·광고·추적·데이터 수집 없음.

안내
본 앱에는 게임이 포함되어 있지 않습니다. 사용자가 적법하게 소유한 게임 파일만
불러올 수 있습니다. 불러오기 기능을 바로 체험할 수 있도록 무료 데모 게임
("하트 캐치")을 제공합니다.

오픈소스 wie 에뮬레이터와 RustJava를 기반으로 하며, MIT 라이선스로 배포됩니다.
```

---

## 키워드 (App Store, ≤100자, 쉼표 구분·공백 최소화)

```
wipi,skvm,j2me,emulator,korean,feature phone,retro,ktf,lgt,skt,mobile,classic,피처폰,에뮬
```

---

## 심사 노트 (App Review Notes / Play 테스트 안내) — 중요

두 스토어 모두 심사관이 게임 실행을 확인해야 하므로, 데모 임포트 절차를 명시.

```
This app is an emulator for Korean feature-phone games (WIPI / SKVM / J2ME).
It ships with NO commercial games — users import game files they legally own.

To verify the full flow, please import our free, MIT-licensed demo game
"Heart Catch" (created by us, source in the repo):

WIPI-C (.zip) demo:
https://github.com/ParkJeongseop/WIPI-Emulator/raw/main/demo/demo_wipi.zip

J2ME (.jar) demo:
https://github.com/ParkJeongseop/WIPI-Emulator/raw/main/demo/demo.jar

Steps:
1. Download either file to the device (Files app / Downloads).
2. Open WIPI Emulator and tap the + button (top right).
3. Select the downloaded .zip or .jar file.
4. Tap the game's cover in the library to start it.
   Move the basket left/right using the on-screen keypad (4 / 6 keys or arrows)
   to catch the falling hearts; your score is shown on screen.
5. To exit: open the pause menu (top-right) and tap Exit (iOS), or press the
   system Back button (Android).

The app collects no data and works fully offline.
Privacy policy:
https://github.com/ParkJeongseop/WIPI-Emulator/blob/main/docs/PRIVACY.md

Thank you for reviewing.
```

---

## 콘텐츠 등급 설문 (정직하게 답할 기준)

앱 자체에는 성인/폭력/도박 콘텐츠 없음. 내장 데모는 단순 캐치 게임. 사용자가
임포트하는 게임은 앱에 포함되지 않음. → 설문에 모두 "없음"으로 답하면
App Store 4+/9+, Play "만 3세 이상(Everyone)" 수준으로 산정됨.
- 폭력/성적 콘텐츠/욕설/약물/도박: 없음
- 사용자 생성/공유 콘텐츠: 없음 (임포트 파일은 기기 로컬 전용, 공유 기능 없음)
- 웹 브라우징/무제한 인터넷 접근: 없음

---

## 개인정보 (App Privacy / Play Data safety)

**둘 다 "데이터 수집 안 함"으로 선언.**
- 수집하는 데이터 유형: 없음
- 제3자 공유: 없음
- 추적: 없음
- 데이터 암호화/삭제 요청: 해당 없음 (수집 자체가 없음)
- 근거: 완전 오프라인, 네트워크 전송 없음, 계정 없음, 게임 파일은 기기 로컬 전용.

---

## 스토어별 업로드 에셋

| 에셋 | 위치 |
|---|---|
| 앱 아이콘 1024 (App Store) | ios/ 아이콘 소스 (make_icon.py 생성물) |
| 앱 아이콘 512 (Play) | android/ mipmap 소스 |
| iOS 스크린샷 6.9" (1320×2868) ×4 | store/ios/ |
| Android 폰 스크린샷 (1080×2400) ×4 | store/android/ |
| Play 그래픽 이미지 (1024×500) | ⚠️ 미제작 — 필요 |
| AAB (Play 업로드) | android/app/build/outputs/bundle/release/app-release.aab |

---

## 남은 준비물 체크리스트

- [ ] Play 피처 그래픽 1024×500 (Play Console 필수)
- [ ] App Store: 유료 개발자 계정($99/년) 활성 상태 확인
- [ ] Play: 개발자 등록($25) 완료 확인
- [ ] 스토어별 앱 생성(이름 중복 확인) — 콘솔에서 수동
- [ ] Play App Signing 활성화 (업로드 키 = upload-keystore.jks)
