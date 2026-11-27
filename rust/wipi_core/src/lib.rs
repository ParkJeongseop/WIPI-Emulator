//! Platform-agnostic mobile host for the wie emulator core.
//! Android(JNI)와 iOS(C ABI) 브리지가 공유하는 platform 구현과 세션 로직.

pub mod cheat;
pub mod platform;
pub mod session;

use std::collections::BTreeMap;

use wie_backend::{Emulator, Options, Platform, extract_zip};
use wie_core_arm_portable::PortableExecutor;
use wie_j2me::J2MEEmulator;
use wie_ktf::KtfEmulator;
use wie_lgt::LgtEmulator;
use wie_skt::SktEmulator;

pub const SCREEN_WIDTH: u32 = 240;
pub const SCREEN_HEIGHT: u32 = 320;

/// 게임을 어느 크기의 화면에서 돌릴지 — 호스트가 게임별로 정하는 설정.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScreenMode {
    /// 선언 크기에서만 제대로 나오는 것으로 확인된 타이틀([`DECLARED_SCREEN_TITLES`])은 선언 크기, 나머지는 프레임 크기.
    #[default]
    Auto,
    /// 패키지가 선언한 크기. 프레임에는 비율을 유지해 맞춰 보여준다.
    Declared,
    /// 프레임 크기(240×320).
    Frame,
}

impl ScreenMode {
    /// 브리지가 주고받는 값: 0 = 자동, 1 = 선언 크기, 2 = 프레임 크기. 모르는 값은 자동.
    pub fn from_raw(raw: u32) -> Self {
        match raw {
            1 => Self::Declared,
            2 => Self::Frame,
            _ => Self::Auto,
        }
    }
}

/// 선언 화면 크기(`DisplaySize`)로 돌려야 제대로 나오는 KTF 타이틀의 pid.
///
/// KTF 패키지의 `DisplaySize`는 대개 "이 크기 이상"이라는 뜻이라, 176*220을 선언한 116종 가운데 90여 종은
/// 240×320을 채우거나 가운데에 그린다. 그래서 선언 크기를 무조건 따르지 않고, 두 크기로 모두 돌려 본 결과
/// 선언 크기가 나은 것만 여기 둔다. 같은 pid의 큰 화면 빌드(크아bnb2007)는 240*320을 선언하므로 그대로다.
///
/// 240×320에서 깨지는 것: 테트리스2006(PD003452, 240×176용 배치를 고름), 크아bnb2007(PD004541, 메뉴에 오프셋을
/// 두 번 더함), 서울타이쿤2(PD004707, 큰 화면 리소스가 없어 메뉴에서 멈춤), 대물낚시광2(PD001732, 화면이 번짐),
/// 화산제일검2(PD005381, 큰 화면 타이틀 이미지가 없음).
///
/// 240×320에서 돌기는 하지만 왼쪽 위 176×220에만 그리는 것: 카우스블레이드(PD002010), 스도쿠(PD002819),
/// 동전쌓기2006(PD002917), 도신노홍철맞고(PD003092), 아포칼립스(PD003129), 서든어택(PD004083),
/// 리얼사커2007(PD005307), 다이하드4(PD005453), 정무문3(PD005704), 맞고삼국대전(PD001074), 피자타이쿤(PD002413),
/// 셔터-영혼의울림(PD004363), 폴라폴리2007(PD004868), 미궁미술관살인사건(PD006449).
const DECLARED_SCREEN_TITLES: &[&str] = &[
    "PD001074", "PD001732", "PD002010", "PD002413", "PD002819", "PD002917", "PD003092", "PD003129", "PD003452", "PD004083", "PD004363", "PD004541",
    "PD004707", "PD004868", "PD005307", "PD005381", "PD005453", "PD005704", "PD006449",
];

/// 패키지가 프레임과 다른 화면 크기를 선언했으면 그 크기. 호스트는 이 값이 있을 때만 화면 크기 설정을 보여준다.
pub fn declared_screen(buf: &[u8]) -> Option<(u32, u32)> {
    declared_screen_of(&normalize_archive(extract_zip(buf).ok()?))
}

fn declared_screen_of(files: &BTreeMap<String, Vec<u8>>) -> Option<(u32, u32)> {
    if !KtfEmulator::loadable_archive(files) {
        return None;
    }
    let value = adf_field(files.get("__adf__")?, b"DisplaySize:")?;
    let value = std::str::from_utf8(&value).ok()?.trim();
    let (width, height) = value.split_once(['*', 'x', 'X'])?;
    let size: (u32, u32) = (width.trim().parse().ok()?, height.trim().parse().ok()?);

    (size.0 > 0 && size.1 > 0 && size != (SCREEN_WIDTH, SCREEN_HEIGHT)).then_some(size)
}

/// 이 KTF 패키지를 선언 크기의 화면에서 돌릴지.
fn runs_at_declared_screen(files: &BTreeMap<String, Vec<u8>>, screen_mode: ScreenMode) -> bool {
    match screen_mode {
        ScreenMode::Auto => {
            declared_screen_of(files).is_some() && KtfEmulator::archive_id(files).is_some_and(|pid| DECLARED_SCREEN_TITLES.contains(&pid.as_str()))
        }
        ScreenMode::Declared => true,
        ScreenMode::Frame => false,
    }
}

/// 게임 라이브러리 표시용 메타데이터. 콘솔 에뮬과 달리 WIPI 패키지는
/// 표지 아이콘과 게임명이 zip 안에 동봉돼 있어 외부 DB가 필요 없다.
pub struct GameMetadata {
    /// __adf__(KTF)의 Name 값 — EUC-KR raw 바이트. 호스트가 디코딩한다. 없으면 None.
    pub name_euckr: Option<Vec<u8>>,
    /// 표지 아이콘 PNG 바이트 (big → middle → small 순으로 탐색). 없으면 None.
    pub icon_png: Option<Vec<u8>>,
}

/// 게임 패키지(zip/jar)에서 표지·이름을 추출한다. jar나 아이콘이 없는 포맷이면 해당 필드가 None.
pub fn extract_metadata(buf: &[u8]) -> GameMetadata {
    let Ok(files) = extract_zip(buf) else {
        return GameMetadata {
            name_euckr: None,
            icon_png: None,
        };
    };
    let files = normalize_archive(files);

    // ez-java(JAD+JAR) 패키지: 이름은 JAD의 MIDlet-Name(UTF-8), 아이콘은 JAR 안의 PNG.
    // 호스트는 이름을 EUC-KR로 디코딩하므로 여기서 맞춰 인코딩한다. 판별 순서는 create_emulator와 같다.
    let other_platform = KtfEmulator::loadable_archive(&files) || LgtEmulator::loadable_archive(&files) || SktEmulator::loadable_archive(&files);
    if !other_platform && J2MEEmulator::loadable_archive(&files) {
        return GameMetadata {
            name_euckr: J2MEEmulator::archive_title(&files).map(|title| encoding_rs::EUC_KR.encode(&title).0.into_owned()),
            icon_png: J2MEEmulator::archive_icon(&files),
        };
    }

    let icon_png = ["big.icon", "middle.icon", "small.icon"]
        .iter()
        .find_map(|name| files.get(*name))
        .cloned();

    let name_euckr = files.get("__adf__").and_then(|adf| adf_field(adf, b"Name:"));

    GameMetadata { name_euckr, icon_png }
}

/// 패키지 안의 파일들이 한 디렉터리 아래에 들어 있으면(단말 파일시스템 덤프 `W/apps/<id>/…`, 게임명 폴더로
/// 묶은 배포본) 그 접두사를 벗겨 루트에 놓는다. 어느 플랫폼도 알아보지 못하는 경우에만, 플랫폼 표식 파일
/// (KTF `__adf__`, LGT `app_info`, SKT `.msd`, ez-java `.jad`)이 있는 디렉터리를 찾아 그 아래 파일만 남긴다.
pub fn normalize_archive(files: BTreeMap<String, Vec<u8>>) -> BTreeMap<String, Vec<u8>> {
    strip_handset_files(normalize_layout(files))
}

/// `(KTF pid, 경로)`: 아카이브를 뜬 단말에서 딸려온, 게임의 일부가 아닌 파일. 새로 설치한 단말에는 없었고 다른
/// 단말에서는 쓸 수도 없는 것들이라 올리지 않는다. 라이브러리 `manifest.tsv`의 `strip` 열과 같은 목록.
///
/// 티어즈오브메탈(PD004584) `P/res/save.sav`·`savem.sav`: 단말 메모리의 세이브 구조체 덤프라 힙 포인터가 그대로
/// 들어 있고, 타이틀이 192바이트를 구조체로 읽은 뒤 그 안의 포인터를 따라가 그 단말 밖에서는 어디서든 fault.
///
/// 삼국쟁패2(PD005126) `P/sj2_save.bin`: 뜬 사람의 전화번호로 묶인 암호화 세이브. 읽자마자 타이틀이 종료한다.
const HANDSET_FILES: &[(&str, &str)] = &[
    ("PD004584", "P/res/save.sav"),
    ("PD004584", "P/res/savem.sav"),
    ("PD005126", "P/sj2_save.bin"),
];

fn strip_handset_files(mut files: BTreeMap<String, Vec<u8>>) -> BTreeMap<String, Vec<u8>> {
    if let Some(pid) = KtfEmulator::archive_id(&files) {
        for (_, path) in HANDSET_FILES.iter().filter(|(p, _)| *p == pid) {
            if files.remove(*path).is_some() {
                tracing::info!("dropping {path}: carried over from the archive's handset");
            }
        }
    }
    files
}

fn normalize_layout(files: BTreeMap<String, Vec<u8>>) -> BTreeMap<String, Vec<u8>> {
    let recognised = KtfEmulator::loadable_archive(&files)
        || LgtEmulator::loadable_archive(&files)
        || SktEmulator::loadable_archive(&files)
        || J2MEEmulator::loadable_archive(&files);
    if recognised {
        return files;
    }

    let is_marker = |name: &str| name == "__adf__" || name == "app_info" || name.ends_with(".msd") || name.ends_with(".jad");
    let Some(prefix) = files
        .keys()
        .filter_map(|path| path.rfind('/').map(|at| (&path[..=at], &path[at + 1..])))
        .filter(|(_, name)| is_marker(name))
        .map(|(prefix, _)| prefix.to_owned())
        .min_by_key(|prefix| prefix.len())
    else {
        return files;
    };

    files
        .into_iter()
        .filter_map(|(path, data)| path.strip_prefix(&prefix).map(|rest| (rest.to_owned(), data)))
        .filter(|(path, _)| !path.is_empty() && !path.ends_with('/'))
        .collect()
}

/// __adf__는 "Key:Value" 라인 텍스트. 주어진 키의 값을 raw 바이트로 반환한다.
/// 키는 ASCII이므로 EUC-KR 값이어도 바이트 단위 매칭이 안전하다.
fn adf_field(adf: &[u8], key: &[u8]) -> Option<Vec<u8>> {
    adf.split(|&b| b == b'\n' || b == b'\r')
        .find_map(|line| line.strip_prefix(key))
        .map(<[u8]>::to_vec)
        .filter(|v| !v.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_archive_drops_files_carried_over_from_the_handset() {
        let files: BTreeMap<String, Vec<u8>> = [
            ("__adf__", b"PID:PD004584\nAID:0103451A\n".to_vec()),
            ("0103451A.jar", b"j".to_vec()),
            ("P/res/save.sav", b"s".to_vec()),
            ("P/res/data.bin", b"d".to_vec()),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v))
        .collect();

        let files = normalize_archive(files);
        assert!(!files.contains_key("P/res/save.sav"));
        assert!(files.contains_key("P/res/data.bin"));
    }

    #[test]
    fn normalize_archive_strips_the_directory_holding_the_marker() {
        let files: BTreeMap<String, Vec<u8>> = [
            ("W/exe_info", b"x".to_vec()),
            ("W/apps/010100D3/__adf__", b"Name:a\n".to_vec()),
            ("W/apps/010100D3/010100D3.jar", b"j".to_vec()),
            ("W/apps/010100D3/P/010100D3", b"p".to_vec()),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v))
        .collect();
        let files = normalize_archive(files);
        assert!(files.contains_key("__adf__"));
        assert!(files.contains_key("010100D3.jar"));
        assert!(files.contains_key("P/010100D3"));
        assert!(!files.contains_key("W/exe_info"));

        let files: BTreeMap<String, Vec<u8>> = [("이노티아연대기/app_info", b"a".to_vec()), ("이노티아연대기/0001e718.jar", b"j".to_vec())]
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v))
            .collect();
        let files = normalize_archive(files);
        assert!(files.contains_key("app_info") && files.contains_key("0001e718.jar"));

        // an archive a platform already recognises is left alone
        let files: BTreeMap<String, Vec<u8>> = [("__adf__", b"Name:a\n".to_vec()), ("a.jar", b"j".to_vec())]
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v))
            .collect();
        assert_eq!(normalize_archive(files).len(), 2);
    }

    #[test]
    fn declared_screen_is_followed_only_for_listed_titles_unless_the_host_says_otherwise() {
        let archive = |pid: &str, size: &str| -> BTreeMap<String, Vec<u8>> {
            [
                ("__adf__", format!("PID:{pid}\r\nAID:01030535\r\nDisplaySize:{size}\r\n").into_bytes()),
                ("01030535.jar", b"j".to_vec()),
            ]
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v))
            .collect()
        };

        let listed = archive("PD002917", "176*220");
        assert_eq!(declared_screen_of(&listed), Some((176, 220)));
        assert!(runs_at_declared_screen(&listed, ScreenMode::Auto));
        assert!(!runs_at_declared_screen(&listed, ScreenMode::Frame));

        let unlisted = archive("PD000001", "176*220");
        assert_eq!(declared_screen_of(&unlisted), Some((176, 220)));
        assert!(!runs_at_declared_screen(&unlisted, ScreenMode::Auto));
        assert!(runs_at_declared_screen(&unlisted, ScreenMode::Declared));

        // 프레임과 같은 크기를 선언했으면 고를 것이 없다
        assert_eq!(declared_screen_of(&archive("PD004541", "240*320")), None);
        assert!(!runs_at_declared_screen(&archive("PD004541", "240*320"), ScreenMode::Auto));
    }

    /// 실제 게임 패키지에서 표지/이름이 뽑히는지 확인 (env로 경로 지정 시에만 실행).
    #[test]
    fn extract_metadata_smoke() {
        let Ok(path) = std::env::var("WIE_TEST_ROM") else {
            return;
        };
        let buf = std::fs::read(&path).unwrap();
        let meta = extract_metadata(&buf);
        eprintln!(
            "name_euckr: {} bytes, icon_png: {} bytes",
            meta.name_euckr.as_ref().map_or(0, Vec::len),
            meta.icon_png.as_ref().map_or(0, Vec::len),
        );
        // PNG 시그니처 확인
        if let Some(icon) = &meta.icon_png {
            assert_eq!(&icon[..4], &[0x89, b'P', b'N', b'G'], "icon should be PNG");
        }
        assert!(meta.icon_png.is_some(), "expected a cover icon");
    }
}

/// Creates the right emulator for the given file, mirroring the
/// format-detection branch in wie_cli / wie_web.
pub fn create_emulator(platform: Box<dyn Platform>, filename: &str, buf: &[u8], screen_mode: ScreenMode) -> anyhow::Result<Box<dyn Emulator>> {
    // iOS forbids generating code at run time, so both platforms run ARM code through the portable executor:
    // the title is translated once into pre-decoded steps. It counts instructions like the stepping interpreter,
    // so thread scheduling (and every frame under WIE_VCLOCK) is the same; `WIE_AOT=off` selects the interpreter for A/B runs.
    let options = Options {
        enable_gdbserver: false,
        aot: match std::env::var("WIE_AOT").as_deref() {
            Ok("off") => None,
            _ => Some(Box::new(PortableExecutor::new())),
        },
        profile: None,
    };

    if filename.ends_with(".zip") {
        let files: BTreeMap<String, Vec<u8>> = normalize_archive(extract_zip(buf)?);

        if KtfEmulator::loadable_archive(&files) {
            // 로더는 선언 크기로 화면을 바꿔 달라고 요청한다. 래퍼가 있으면 그 크기의 화면이 되고, 없으면 요청이 무시된다.
            let platform: Box<dyn Platform> = if runs_at_declared_screen(&files, screen_mode) {
                Box::new(platform::LcdPlatform::new(platform))
            } else {
                platform
            };
            Ok(Box::new(KtfEmulator::from_archive(platform, files, options)?))
        } else if LgtEmulator::loadable_archive(&files) {
            Ok(Box::new(LgtEmulator::from_archive(platform, files, options)?))
        } else if SktEmulator::loadable_archive(&files) {
            Ok(Box::new(SktEmulator::from_archive(platform, files)?))
        } else if J2MEEmulator::loadable_archive(&files) {
            // ez-java(JAD+JAR): 디스크립터가 정한 LCD 크기로 돌리고 고정 프레임에 맞춰 보여준다.
            let platform = Box::new(platform::LcdPlatform::new(platform));
            Ok(Box::new(J2MEEmulator::from_archive(platform, files)?))
        } else if let [(_, jar)] = files
            .iter()
            .filter(|(name, _)| name.to_ascii_lowercase().ends_with(".jar"))
            .collect::<Vec<_>>()
            .as_slice()
            && files.len() == 1
        {
            // A JAR zipped on its own, often under a name the archiver mangled: it is the game, named
            // after the archive.
            let stem = &filename[filename.rfind(['/', '\\']).map(|x| x + 1).unwrap_or(0)..];
            let jar_name = format!("{}.jar", stem.trim_end_matches(".zip"));
            create_emulator(platform, &jar_name, jar, screen_mode)
        } else {
            anyhow::bail!("Unknown archive format")
        }
    } else if filename.ends_with(".jar") {
        let filename_without_path = &filename[filename.rfind(['/', '\\']).map(|x| x + 1).unwrap_or(0)..];
        let filename_without_ext = filename_without_path.trim_end_matches(".jar");

        if KtfEmulator::loadable_jar(buf) {
            Ok(Box::new(KtfEmulator::from_jar(
                platform,
                filename_without_path,
                buf.to_vec(),
                filename_without_ext,
                filename_without_ext,
                None,
                options,
            )?))
        } else if LgtEmulator::loadable_jar(buf) {
            Ok(Box::new(LgtEmulator::from_jar(
                platform,
                filename_without_path,
                buf.to_vec(),
                filename_without_ext,
                filename_without_ext,
                None,
                options,
            )?))
        } else if SktEmulator::loadable_jar(buf) {
            Ok(Box::new(SktEmulator::from_jar(
                platform,
                filename_without_path,
                buf.to_vec(),
                filename_without_ext,
                None,
            )?))
        } else {
            Ok(Box::new(J2MEEmulator::from_jar(platform, filename_without_path, buf.to_vec())?))
        }
    } else {
        anyhow::bail!("Unknown file format")
    }
}
