//! Platform-agnostic mobile host for the wie emulator core.
//! Android(JNI)와 iOS(C ABI) 브리지가 공유하는 platform 구현과 세션 로직.

pub mod platform;
pub mod session;

use std::collections::BTreeMap;

use wie_backend::{Emulator, Options, Platform, extract_zip};
use wie_j2me::J2MEEmulator;
use wie_ktf::KtfEmulator;
use wie_lgt::LgtEmulator;
use wie_skt::SktEmulator;

pub const SCREEN_WIDTH: u32 = 240;
pub const SCREEN_HEIGHT: u32 = 320;

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
pub fn create_emulator(platform: Box<dyn Platform>, filename: &str, buf: &[u8]) -> anyhow::Result<Box<dyn Emulator>> {
    let options = Options {
        enable_gdbserver: false,
        profile: None,
    };

    if filename.ends_with(".zip") {
        let files: BTreeMap<String, Vec<u8>> = normalize_archive(extract_zip(buf)?);

        if KtfEmulator::loadable_archive(&files) {
            Ok(Box::new(KtfEmulator::from_archive(platform, files, options)?))
        } else if LgtEmulator::loadable_archive(&files) {
            Ok(Box::new(LgtEmulator::from_archive(platform, files, options)?))
        } else if SktEmulator::loadable_archive(&files) {
            Ok(Box::new(SktEmulator::from_archive(platform, files)?))
        } else if J2MEEmulator::loadable_archive(&files) {
            // ez-java(JAD+JAR): 디스크립터가 정한 LCD 크기로 돌리고 고정 프레임에 맞춰 보여준다.
            let platform = Box::new(platform::LcdPlatform::new(platform));
            Ok(Box::new(J2MEEmulator::from_archive(platform, files)?))
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
