//! 실행 중인 게임 메모리에서 값을 찾아 고치는 치트 (값 검색 → 좁히기 → 쓰기/고정).
//! ARM 코어로 도는 타이틀(KTF·LGT)만 대상 — 순수 자바 타이틀은 평평한 게스트 메모리가 없다.

use std::collections::BTreeMap;

use wie_backend::GuestMemory;

/// 검색 한 번에 보관하는 후보 수 상한. 0처럼 흔한 값은 메모리 대부분과 일치하므로 여기서 자르고,
/// 호스트가 더 드문 값으로 다시 찾으라고 안내한다 (search가 돌려주는 일치 수는 잘리기 전 값).
pub const MAX_CANDIDATES: usize = 1_000_000;

const CHUNK: usize = 0x10000;

#[derive(Default)]
pub struct Cheats {
    /// 현재 검색의 값 폭(1·2·4바이트)과 후보 주소
    width: usize,
    candidates: Vec<u32>,
    /// 고정 중인 주소 → (폭, 값). 매 tick 전에 다시 써 넣는다.
    frozen: BTreeMap<u32, (usize, u32)>,
}

fn sanitize_width(width: usize) -> usize {
    if matches!(width, 1 | 2) { width } else { 4 }
}

fn mask(value: u32, width: usize) -> u32 {
    match width {
        1 => value & 0xff,
        2 => value & 0xffff,
        _ => value,
    }
}

impl Cheats {
    /// 새 검색: 게임 데이터 영역에서 `width`바이트 정렬 위치의 값이 `value`인 주소를 모두 찾는다.
    /// 일치 수를 반환한다 (MAX_CANDIDATES를 넘으면 앞쪽만 보관).
    pub fn search(&mut self, memory: &dyn GuestMemory, value: u32, width: usize) -> u64 {
        let width = sanitize_width(width);
        let needle = mask(value, width).to_le_bytes();
        let needle = &needle[..width];

        self.width = width;
        self.candidates.clear();

        let mut matches = 0u64;
        let mut buffer = vec![0u8; CHUNK];
        for range in memory.data_ranges().unwrap_or_default() {
            let mut address = range.start.next_multiple_of(width as u32);
            while address < range.end {
                let length = ((range.end - address) as usize).min(CHUNK) / width * width;
                if length == 0 || memory.read_bytes(address, &mut buffer[..length]).is_err() {
                    break;
                }
                for (index, value) in buffer[..length].chunks_exact(width).enumerate() {
                    if value == needle {
                        matches += 1;
                        if self.candidates.len() < MAX_CANDIDATES {
                            self.candidates.push(address + (index * width) as u32);
                        }
                    }
                }
                address += length as u32;
            }
        }

        matches
    }

    /// 좁히기: 직전 검색의 후보 중 지금 값이 `value`인 것만 남긴다. 남은 수를 반환한다.
    pub fn refine(&mut self, memory: &dyn GuestMemory, value: u32) -> u64 {
        let width = self.width;
        let value = mask(value, width);
        self.candidates.retain(|&address| Self::read(memory, address, width) == Some(value));

        self.candidates.len() as u64
    }

    /// 후보 앞쪽 `limit`개의 (주소, 현재 값)
    pub fn results(&self, memory: &dyn GuestMemory, limit: usize) -> Vec<(u32, u32)> {
        self.candidates
            .iter()
            .take(limit)
            .filter_map(|&address| Some((address, Self::read(memory, address, self.width)?)))
            .collect()
    }

    pub fn read(memory: &dyn GuestMemory, address: u32, width: usize) -> Option<u32> {
        let mut bytes = [0u8; 4];
        memory.read_bytes(address, &mut bytes[..sanitize_width(width)]).ok()?;

        Some(u32::from_le_bytes(bytes))
    }

    pub fn write(memory: &mut dyn GuestMemory, address: u32, width: usize, value: u32) -> bool {
        memory.write_bytes(address, &value.to_le_bytes()[..sanitize_width(width)]).is_ok()
    }

    /// 값을 쓰고, 풀 때까지 매 tick 다시 써서 고정한다. 쓸 수 없는 주소면 false.
    pub fn freeze(&mut self, memory: &mut dyn GuestMemory, address: u32, width: usize, value: u32) -> bool {
        if !Self::write(memory, address, width, value) {
            return false;
        }
        self.frozen.insert(address, (width, value));

        true
    }

    pub fn unfreeze(&mut self, address: u32) {
        self.frozen.remove(&address);
    }

    pub fn has_frozen(&self) -> bool {
        !self.frozen.is_empty()
    }

    /// 고정 중인 값을 다시 써 넣는다.
    pub fn apply(&self, memory: &mut dyn GuestMemory) {
        for (&address, &(width, value)) in &self.frozen {
            Self::write(memory, address, width, value);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ops::Range;

    use wie_util::{ByteRead, ByteWrite, Result, WieError};

    use super::*;

    /// 0x1000에서 시작하는 메모리 한 덩어리. `ranges`만 게임 데이터로 알린다.
    struct TestMemory {
        bytes: Vec<u8>,
        ranges: Vec<Range<u32>>,
    }

    const BASE: u32 = 0x1000;

    impl TestMemory {
        fn slice(&self, address: u32, length: usize) -> Result<Range<usize>> {
            let start = address.checked_sub(BASE).ok_or(WieError::InvalidMemoryAccess(address))? as usize;
            if start + length > self.bytes.len() {
                return Err(WieError::InvalidMemoryAccess(address));
            }
            Ok(start..start + length)
        }
    }

    impl ByteRead for TestMemory {
        fn read_bytes(&self, address: u32, result: &mut [u8]) -> Result<usize> {
            let range = self.slice(address, result.len())?;
            result.copy_from_slice(&self.bytes[range]);
            Ok(result.len())
        }
    }

    impl ByteWrite for TestMemory {
        fn write_bytes(&mut self, address: u32, data: &[u8]) -> Result<()> {
            let range = self.slice(address, data.len())?;
            self.bytes[range].copy_from_slice(data);
            Ok(())
        }
    }

    impl GuestMemory for TestMemory {
        fn data_ranges(&self) -> Result<Vec<Range<u32>>> {
            Ok(self.ranges.clone())
        }
    }

    #[test]
    fn search_refine_write_and_freeze() {
        // 청크 경계(0x10000)를 넘는 영역 하나와 정렬이 어긋난 영역 하나
        let mut memory = TestMemory {
            bytes: vec![0; 0x20000],
            ranges: vec![BASE..BASE + 0x18000, BASE + 0x18002..BASE + 0x18100],
        };
        let (gold, decoy, far, outside) = (BASE + 0x40, BASE + 0x80, BASE + 0x17ffc, BASE + 0x19000);
        for address in [gold, decoy, far, outside] {
            assert!(Cheats::write(&mut memory, address, 4, 1234));
        }
        // 정렬이 어긋난 위치의 같은 바이트열은 4바이트 검색에 잡히지 않는다
        assert!(Cheats::write(&mut memory, BASE + 0x102, 4, 1234));

        let mut cheats = Cheats::default();
        assert_eq!(cheats.search(&memory, 1234, 4), 3);
        assert_eq!(cheats.results(&memory, 10), [(gold, 1234), (decoy, 1234), (far, 1234)]);
        assert_eq!(cheats.results(&memory, 2).len(), 2);

        // 게임이 값을 바꾼 뒤 좁히기
        Cheats::write(&mut memory, gold, 4, 1200);
        assert_eq!(cheats.refine(&memory, 1200), 1);
        assert_eq!(cheats.results(&memory, 10), [(gold, 1200)]);

        // 고정: 게임이 덮어써도 apply가 되돌린다
        assert!(cheats.freeze(&mut memory, gold, 4, 9999));
        Cheats::write(&mut memory, gold, 4, 5);
        cheats.apply(&mut memory);
        assert_eq!(Cheats::read(&memory, gold, 4), Some(9999));
        cheats.unfreeze(gold);
        assert!(!cheats.has_frozen());

        // 매핑되지 않은 주소
        assert!(!cheats.freeze(&mut memory, 0x10, 4, 1));
        assert_eq!(Cheats::read(&memory, 0x10, 4), None);

        // 1·2바이트 폭은 값의 하위 바이트만 본다
        Cheats::write(&mut memory, BASE + 0x200, 2, 0x1_00ff);
        assert_eq!(Cheats::read(&memory, BASE + 0x200, 2), Some(0xff));
        assert_eq!(cheats.search(&memory, 0x1_00ff, 2), 1);
        assert_eq!(cheats.search(&memory, 0xff, 1), 1);
    }
}
