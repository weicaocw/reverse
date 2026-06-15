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

/// 一个在字节流上移动的"游标":它记住自己读到哪了,每读一个字节就自动前进,
/// 越界时安全地返回 `None` 而不是让程序崩溃。
///
/// `<'a>` 是"生命周期标注"。它对编译器承诺:这个游标借用的那段字节
/// (`data`)活得至少和游标一样久——绝不会出现"数据没了、游标还指着它"的悬空。
pub struct ByteReader<'a> {
    /// 被借用的整段字节(只读)。
    data: &'a [u8],
    /// 当前读到第几个字节(下一个要读的位置)。
    pos: usize,
}

impl<'a> ByteReader<'a> {
    /// 在一段字节上新建游标,初始位置 0。
    pub fn new(data: &'a [u8]) -> Self {
        ByteReader { data, pos: 0 }
    }

    /// 当前游标位置(下一个要读的字节下标)。
    pub fn position(&self) -> usize {
        self.pos
    }

    /// 还剩多少字节没读。
    pub fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    /// 读出当前字节并让游标前进 1;若已到末尾(越界)返回 `None`。
    pub fn read_u8(&mut self) -> Option<u8> {
        // .get(i) 在越界时返回 None,而不是像 data[i] 那样 panic。
        let byte = *self.data.get(self.pos)?;
        self.pos += 1;
        Some(byte)
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

    #[test]
    fn reader_reads_bytes_in_order() {
        let data = [0xaa, 0xbb, 0xcc];
        let mut r = ByteReader::new(&data);
        assert_eq!(r.position(), 0); // 一开始在第 0 个字节
        assert_eq!(r.read_u8(), Some(0xaa)); // 读出第 1 个,游标前进
        assert_eq!(r.read_u8(), Some(0xbb)); // 读出第 2 个
        assert_eq!(r.position(), 2); // 已经读了 2 个,游标在第 2 位
    }

    #[test]
    fn reader_returns_none_past_the_end() {
        let data = [0x01];
        let mut r = ByteReader::new(&data);
        assert_eq!(r.read_u8(), Some(0x01)); // 读完唯一一个字节
        assert_eq!(r.read_u8(), None); // 再读就越界了:返回 None,而不是崩溃
        assert_eq!(r.read_u8(), None); // 越界后继续读,依然安稳地返回 None
    }
}
