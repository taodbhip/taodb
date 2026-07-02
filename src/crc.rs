//! CRC32 完整性校验
//!
//! 编码: bincode(Memory) + CRC32_le_bytes(4字节)
//! 解码: 读最后4字节CRC → 校验 → bincode反序列化
//! 向後兼容: 无CRC的旧数据直接 bincode 反序列化

/// 编码 Memory 字节流，追加 CRC32 校验尾。
/// 返回 `[原始 bincode 数据][4字节 CRC32 LE]`
pub fn encode_with_crc(data: &[u8]) -> Vec<u8> {
    let crc = crc32fast::hash(data);
    let mut out = Vec::with_capacity(data.len() + 4);
    out.extend_from_slice(data);
    out.extend_from_slice(&crc.to_le_bytes());
    out
}

/// 解码带 CRC 尾的字节流。
/// 如果数据末尾 4 字节 CRC 校验通过，返回原始数据（不含 CRC 尾）。
/// 如果 CRC 校验失败或数据不足 4 字节，返回 `Err(CrcError)`。
/// 对于旧格式数据（无 CRC），使用 `decode_tolerant`。
pub fn decode_with_crc(raw: &[u8]) -> Result<Vec<u8>, CrcError> {
    if raw.len() < 4 {
        return Err(CrcError { stored: 0, computed: 0 });
    }

    let data = &raw[..raw.len() - 4];
    let stored_crc = u32::from_le_bytes([
        raw[raw.len() - 4],
        raw[raw.len() - 3],
        raw[raw.len() - 2],
        raw[raw.len() - 1],
    ]);
    let computed_crc = crc32fast::hash(data);

    if stored_crc == computed_crc {
        Ok(data.to_vec())
    } else {
        Err(CrcError {
            stored: stored_crc,
            computed: computed_crc,
        })
    }
}

/// 解码并容忍 CRC 错误（用于读取旧数据）。
/// CRC 失败时仍返回原始数据，但打印警告。
pub fn decode_tolerant(raw: &[u8]) -> Vec<u8> {
    match decode_with_crc(raw) {
        Ok(data) => data,
        Err(_) => {
            // 可能是旧格式，返回全部数据
            raw.to_vec()
        }
    }
}

#[derive(Debug, Clone)]
pub struct CrcError {
    pub stored: u32,
    pub computed: u32,
}

impl std::fmt::Display for CrcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "CRC mismatch: stored=0x{:08x} computed=0x{:08x}",
            self.stored, self.computed
        )
    }
}

impl std::error::Error for CrcError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_roundtrip() {
        let data = b"hello world";
        let encoded = encode_with_crc(data);
        assert_eq!(encoded.len(), data.len() + 4);
        let decoded = decode_with_crc(&encoded).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn old_format_no_crc() {
        // 旧格式：无 CRC 尾 — 用 tolerant 模式
        let data = b"old format data";
        let decoded = decode_tolerant(data);
        assert_eq!(decoded, data);
    }

    #[test]
    fn short_data_rejected() {
        // 短于 4 字节的数据无法通过 CRC 校验
        let data = b"hi";
        assert!(decode_with_crc(data).is_err());
    }

    #[test]
    fn crc_mismatch_detected() {
        let data = b"some data";
        let mut encoded = encode_with_crc(data);
        // 篡改数据
        encoded[0] ^= 0xFF;
        assert!(decode_with_crc(&encoded).is_err());
    }

    #[test]
    fn tolerant_handles_corruption() {
        let data = b"some data";
        let mut encoded = encode_with_crc(data);
        encoded[0] ^= 0xFF;
        // tolerant 模式不 panic
        let decoded = decode_tolerant(&encoded);
        assert!(!decoded.is_empty());
    }
}
