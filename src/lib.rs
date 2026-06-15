//! revx —— 一个用于学习的逆向工程工具库
//!
//! 模块 A 的第一块积木:根据文件开头的"魔数"识别可执行文件格式。

/// 可执行文件格式。
///
/// `enum`(枚举)表示"一个值只能是这几种之一",非常适合表达"格式"这种封闭集合。
#[derive(Debug, PartialEq, Eq)]
pub enum Format {
    /// Linux 的可执行 / 目标文件格式
    Elf,
    /// macOS 64 位
    MachO64,
    /// macOS 32 位
    MachO32,
    /// Windows 的 .exe / .dll
    Pe,
    /// 认不出来
    Unknown,
}

/// 根据文件开头的魔数(magic number)判断格式。
///
/// `bytes` 是文件内容的前若干字节;`&[u8]` 是"字节切片",即"借用一段连续的字节"。
pub fn detect(bytes: &[u8]) -> Format {
    if bytes.starts_with(&[0x7f, b'E', b'L', b'F']) {
        Format::Elf
    } else if bytes.starts_with(&[0xcf, 0xfa, 0xed, 0xfe]) {
        Format::MachO64
    } else if bytes.starts_with(&[0xce, 0xfa, 0xed, 0xfe]) {
        Format::MachO32
    } else if bytes.starts_with(&[0x4d, 0x5a]) {
        // 4d 5a == 'M' 'Z',Windows PE 文件的开头
        Format::Pe
    } else {
        Format::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_macho64() {
        // cf fa ed fe 是 macOS 64 位可执行文件的魔数(小端存储)
        let bytes = [0xcf, 0xfa, 0xed, 0xfe, 0x07, 0x00];
        assert_eq!(detect(&bytes), Format::MachO64);
    }

    #[test]
    fn detects_elf() {
        // 7f 45 4c 46 == 0x7f 'E' 'L' 'F'
        assert_eq!(detect(&[0x7f, b'E', b'L', b'F']), Format::Elf);
    }

    #[test]
    fn unknown_for_random_bytes() {
        assert_eq!(detect(&[0x00, 0x01, 0x02]), Format::Unknown);
    }
}
