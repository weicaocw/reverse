//! revx —— 一个用于学习的逆向工程工具库
//!
//! 模块 A 的第一块积木:根据文件开头的"魔数"识别可执行文件格式。

use std::fmt;

/// 解析过程中可能出现的错误。每个变体都**携带足够的信息**说明"为什么失败"。
#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    /// 文件在还没读够时就结束了;`offset` 指出在哪个偏移处缺字节。
    UnexpectedEof { offset: usize },
    /// 魔数不认识,无法识别格式。
    UnknownFormat,
}

/// 让 `ParseError` 能被打印成给人看的话(`println!("{}", err)`)。
impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ParseError::UnexpectedEof { offset } => {
                write!(f, "文件意外结束:在偏移 {offset} 处还需要更多字节")
            }
            ParseError::UnknownFormat => write!(f, "无法识别的文件格式(未知魔数)"),
        }
    }
}

/// 让 `ParseError` 成为"标准错误类型",可被 `?`、`Box<dyn Error>` 等通用机制接纳。
impl std::error::Error for ParseError {}

/// 识别文件格式。和 `detect` 不同,它用 `Result` **解释失败原因**:
/// 文件太短 → `UnexpectedEof`;魔数不认识 → `UnknownFormat`。
pub fn identify(bytes: &[u8]) -> Result<Format, ParseError> {
    // 最短的魔数(PE 的 "MZ")也要 2 字节;不足就是文件太短。
    if bytes.len() < 2 {
        return Err(ParseError::UnexpectedEof {
            offset: bytes.len(),
        });
    }
    match detect(bytes) {
        Format::Unknown => Err(ParseError::UnknownFormat),
        known => Ok(known),
    }
}

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

/// 字节序:多字节整数在文件里是"低位在前"(小端)还是"高位在前"(大端)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endian {
    /// 小端:最低有效字节排在最前面(x86 / ARM 默认)。
    Little,
    /// 大端:最高有效字节排在最前面(网络字节序 / 部分架构)。
    Big,
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

    /// 读 2 个字节,按指定字节序拼成一个 u16;字节不够返回 None。
    pub fn read_u16(&mut self, endian: Endian) -> Option<u16> {
        let bytes = [self.read_u8()?, self.read_u8()?];
        Some(match endian {
            Endian::Little => u16::from_le_bytes(bytes),
            Endian::Big => u16::from_be_bytes(bytes),
        })
    }

    /// 读 4 个字节,按指定字节序拼成一个 u32;字节不够返回 None。
    pub fn read_u32(&mut self, endian: Endian) -> Option<u32> {
        let bytes = [
            self.read_u8()?,
            self.read_u8()?,
            self.read_u8()?,
            self.read_u8()?,
        ];
        Some(match endian {
            Endian::Little => u32::from_le_bytes(bytes),
            Endian::Big => u32::from_be_bytes(bytes),
        })
    }

    /// 读 8 个字节,按指定字节序拼成一个 u64;字节不够返回 None。
    pub fn read_u64(&mut self, endian: Endian) -> Option<u64> {
        let mut bytes = [0u8; 8];
        for slot in bytes.iter_mut() {
            *slot = self.read_u8()?;
        }
        Some(match endian {
            Endian::Little => u64::from_le_bytes(bytes),
            Endian::Big => u64::from_be_bytes(bytes),
        })
    }

    /// 把游标跳到绝对位置 `pos`;越过末尾返回 `None`(允许正好停在末尾)。
    pub fn seek(&mut self, pos: usize) -> Option<()> {
        if pos > self.data.len() {
            return None;
        }
        self.pos = pos;
        Some(())
    }
}

/// Mach-O 64 位文件头(对应 C 里的 `mach_header_64`,共 8 个 u32 字段 = 32 字节)。
#[derive(Debug, PartialEq, Eq)]
pub struct MachHeader {
    /// 魔数,Mach-O 64 位为 0xFEEDFACF。
    pub magic: u32,
    /// CPU 架构(如 0x01000007 = x86_64)。
    pub cputype: u32,
    /// CPU 子型号。
    pub cpusubtype: u32,
    /// 文件类型(2 = 可执行文件,6 = 动态库 …)。
    pub filetype: u32,
    /// 后面跟着多少条加载命令(load command)。
    pub ncmds: u32,
    /// 所有加载命令合计多少字节。
    pub sizeofcmds: u32,
    /// 标志位。
    pub flags: u32,
    /// 64 位头特有的保留字段。
    pub reserved: u32,
}

impl MachHeader {
    /// 可读的 CPU 架构。
    pub fn arch(&self) -> Arch {
        Arch::from_cputype(self.cputype)
    }

    /// 可读的文件类型。
    pub fn file_type(&self) -> FileType {
        FileType::from_u32(self.filetype)
    }
}

/// CPU 架构,从 Mach-O 的 `cputype` 翻译而来。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arch {
    X86,
    X86_64,
    Arm,
    Arm64,
    /// 不认识的 cputype,保留原值不丢信息。
    Other(u32),
}

impl Arch {
    /// 把裸 `cputype` 数字映射成可读架构。
    pub fn from_cputype(cputype: u32) -> Arch {
        match cputype {
            0x0000_0007 => Arch::X86,
            0x0100_0007 => Arch::X86_64,
            0x0000_000C => Arch::Arm,
            0x0100_000C => Arch::Arm64,
            other => Arch::Other(other),
        }
    }
}

/// 文件类型,从 Mach-O 的 `filetype` 翻译而来。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    Object,
    Executable,
    Dylib,
    Bundle,
    /// 不认识的 filetype,保留原值。
    Other(u32),
}

impl FileType {
    /// 把裸 `filetype` 数字映射成可读类型。
    pub fn from_u32(v: u32) -> FileType {
        match v {
            1 => FileType::Object,
            2 => FileType::Executable,
            6 => FileType::Dylib,
            8 => FileType::Bundle,
            other => FileType::Other(other),
        }
    }
}

/// 把 `Option`(读到/没读到)转成 `Result`(读到/EOF 并记下偏移)的小助手。
fn read_u32_or_eof(r: &mut ByteReader, endian: Endian) -> Result<u32, ParseError> {
    let offset = r.position();
    r.read_u32(endian)
        .ok_or(ParseError::UnexpectedEof { offset })
}

/// 一条加载命令(load command)的通用头部:类型 + 本条总长度。
#[derive(Debug, PartialEq, Eq)]
pub struct LoadCommand {
    /// 命令类型(LC_* 常量,如 0x19 = LC_SEGMENT_64)。
    pub cmd: u32,
    /// 本条命令的总字节数(含这 8 字节头)。
    pub cmdsize: u32,
}

impl LoadCommand {
    /// 把常见的 LC_* 数字翻译成名字;未知则返回 "LC_UNKNOWN"。
    pub fn name(&self) -> &'static str {
        match self.cmd {
            0x19 => "LC_SEGMENT_64",
            0x01 => "LC_SEGMENT",
            0x02 => "LC_SYMTAB",
            0x0B => "LC_DYSYMTAB",
            0x0C => "LC_LOAD_DYLIB",
            0x1B => "LC_UUID",
            0x80000028 => "LC_MAIN",
            0x80000022 => "LC_DYLD_INFO_ONLY",
            _ => "LC_UNKNOWN",
        }
    }
}

/// 遍历 Mach-O 的所有加载命令,返回每条的类型与长度。
///
/// 加载命令紧跟在 32 字节文件头之后,是一串**变长记录**:每条以
/// `cmd`(类型)+ `cmdsize`(本条总长度)开头,我们读完头就按 `cmdsize` 跳到下一条。
pub fn parse_load_commands(bytes: &[u8]) -> Result<Vec<LoadCommand>, ParseError> {
    let header = parse_macho_header(bytes)?;
    let mut r = ByteReader::new(bytes);
    // 加载命令从文件头之后(偏移 32)开始。
    r.seek(32).ok_or(ParseError::UnexpectedEof { offset: 32 })?;

    let mut cmds = Vec::new();
    for _ in 0..header.ncmds {
        let start = r.position();
        let cmd = read_u32_or_eof(&mut r, Endian::Little)?;
        let cmdsize = read_u32_or_eof(&mut r, Endian::Little)?;
        cmds.push(LoadCommand { cmd, cmdsize });
        // 跳到本条命令末尾 = 本条起点 + cmdsize,即下一条的起点。
        let next = start + cmdsize as usize;
        r.seek(next)
            .ok_or(ParseError::UnexpectedEof { offset: next })?;
    }
    Ok(cmds)
}

/// 解析 Mach-O 64 位文件头。当前支持最常见的小端 64 位变体;
/// 不是 Mach-O 64 → `UnknownFormat`;字节不够 → `UnexpectedEof`。
pub fn parse_macho_header(bytes: &[u8]) -> Result<MachHeader, ParseError> {
    // 复用 Step 01 的 detect:先确认确实是 Mach-O 64 位,再动手解析。
    if detect(bytes) != Format::MachO64 {
        return Err(ParseError::UnknownFormat);
    }
    let mut r = ByteReader::new(bytes);
    // Mach-O 64 位头全部按小端读取。逐字段顺序读 8 个 u32。
    Ok(MachHeader {
        magic: read_u32_or_eof(&mut r, Endian::Little)?,
        cputype: read_u32_or_eof(&mut r, Endian::Little)?,
        cpusubtype: read_u32_or_eof(&mut r, Endian::Little)?,
        filetype: read_u32_or_eof(&mut r, Endian::Little)?,
        ncmds: read_u32_or_eof(&mut r, Endian::Little)?,
        sizeofcmds: read_u32_or_eof(&mut r, Endian::Little)?,
        flags: read_u32_or_eof(&mut r, Endian::Little)?,
        reserved: read_u32_or_eof(&mut r, Endian::Little)?,
    })
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

    #[test]
    fn read_u32_little_endian_restores_macho_magic() {
        // 文件里看到的字节顺序就是 Step 01 的 cf fa ed fe
        let data = [0xcf, 0xfa, 0xed, 0xfe];
        let mut r = ByteReader::new(&data);
        // 小端解读 → 真实数值 0xFEEDFACF(Mach-O 64 位魔数)
        assert_eq!(r.read_u32(Endian::Little), Some(0xFEED_FACF));
        assert_eq!(r.position(), 4); // 读了 4 个字节
    }

    #[test]
    fn read_u32_big_endian_gives_different_value() {
        let data = [0xcf, 0xfa, 0xed, 0xfe];
        let mut r = ByteReader::new(&data);
        // 同样的字节,大端解读 → 完全不同的数值
        assert_eq!(r.read_u32(Endian::Big), Some(0xCFFA_EDFE));
    }

    #[test]
    fn read_u16_and_u64_work_too() {
        let data = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        let mut r = ByteReader::new(&data);
        assert_eq!(r.read_u16(Endian::Big), Some(0x0102)); // 读 2 字节
        assert_eq!(r.read_u16(Endian::Little), Some(0x0403)); // 再读 2 字节(小端:低位在前)
        assert_eq!(r.read_u32(Endian::Big), Some(0x0506_0708)); // 再读 4 字节
    }

    #[test]
    fn read_u32_returns_none_if_not_enough_bytes() {
        let data = [0x01, 0x02]; // 只有 2 字节,凑不齐 u32 的 4 字节
        let mut r = ByteReader::new(&data);
        assert_eq!(r.read_u32(Endian::Little), None); // 安全地失败
    }

    #[test]
    fn identify_returns_ok_for_known_format() {
        assert_eq!(identify(&[0x7f, b'E', b'L', b'F']), Ok(Format::Elf));
    }

    #[test]
    fn identify_errors_when_too_short() {
        // 只有 1 个字节,连魔数都凑不齐 → 报"文件太短",并指出在偏移 1 处缺字节
        assert_eq!(
            identify(&[0x7f]),
            Err(ParseError::UnexpectedEof { offset: 1 })
        );
    }

    #[test]
    fn identify_errors_on_unknown_magic() {
        assert_eq!(
            identify(&[0x12, 0x34, 0x56, 0x78]),
            Err(ParseError::UnknownFormat)
        );
    }

    #[test]
    fn parse_error_has_human_readable_message() {
        // 错误类型能被打印成人话(实现了 Display)
        let msg = format!("{}", ParseError::UnknownFormat);
        assert!(msg.contains("无法识别"), "实际信息: {msg}");
    }

    // 一个手工构造的 Mach-O 64 位文件头(32 字节,8 个小端 u32)。
    // 字段值对应一个 x86_64 的可执行文件。
    const MACHO64_HEADER: [u8; 32] = [
        0xcf, 0xfa, 0xed, 0xfe, // magic    = 0xFEEDFACF
        0x07, 0x00, 0x00, 0x01, // cputype  = 0x01000007 (x86_64)
        0x03, 0x00, 0x00, 0x00, // cpusubtype = 3
        0x02, 0x00, 0x00, 0x00, // filetype = 2 (MH_EXECUTE 可执行文件)
        0x10, 0x00, 0x00, 0x00, // ncmds    = 16
        0x00, 0x01, 0x00, 0x00, // sizeofcmds = 256
        0x85, 0x00, 0x20, 0x00, // flags    = 0x00200085
        0x00, 0x00, 0x00, 0x00, // reserved = 0
    ];

    #[test]
    fn parses_macho64_header_fields() {
        let h = parse_macho_header(&MACHO64_HEADER).unwrap();
        assert_eq!(h.magic, 0xFEED_FACF);
        assert_eq!(h.cputype, 0x0100_0007);
        assert_eq!(h.filetype, 2);
        assert_eq!(h.ncmds, 16);
        assert_eq!(h.sizeofcmds, 256);
    }

    #[test]
    fn macho_header_errors_when_truncated() {
        // 只给前 10 个字节,凑不齐 32 字节的头
        let err = parse_macho_header(&MACHO64_HEADER[..10]).unwrap_err();
        assert!(matches!(err, ParseError::UnexpectedEof { .. }));
    }

    #[test]
    fn macho_header_rejects_non_macho() {
        // ELF 的开头,不是 Mach-O
        let err = parse_macho_header(&[0x7f, b'E', b'L', b'F', 0, 0, 0, 0]).unwrap_err();
        assert_eq!(err, ParseError::UnknownFormat);
    }

    #[test]
    fn arch_maps_known_cputypes() {
        assert_eq!(Arch::from_cputype(0x0100_0007), Arch::X86_64);
        assert_eq!(Arch::from_cputype(0x0100_000C), Arch::Arm64);
        assert_eq!(Arch::from_cputype(0x0000_0007), Arch::X86);
        // 不认识的 cputype 保留原值,不丢信息
        assert_eq!(Arch::from_cputype(0x1234), Arch::Other(0x1234));
    }

    #[test]
    fn filetype_maps_known_values() {
        assert_eq!(FileType::from_u32(2), FileType::Executable);
        assert_eq!(FileType::from_u32(6), FileType::Dylib);
        assert_eq!(FileType::from_u32(99), FileType::Other(99));
    }

    #[test]
    fn header_exposes_readable_arch_and_filetype() {
        let h = parse_macho_header(&MACHO64_HEADER).unwrap();
        assert_eq!(h.arch(), Arch::X86_64);
        assert_eq!(h.file_type(), FileType::Executable);
    }

    /// 构造一个带 2 条加载命令的最小 Mach-O:头(ncmds=2, sizeofcmds=32)+ 两条 16 字节命令。
    fn macho_with_two_load_commands() -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&[0xcf, 0xfa, 0xed, 0xfe]); // magic
        v.extend_from_slice(&0x0100_0007u32.to_le_bytes()); // cputype x86_64
        v.extend_from_slice(&3u32.to_le_bytes()); // cpusubtype
        v.extend_from_slice(&2u32.to_le_bytes()); // filetype 可执行
        v.extend_from_slice(&2u32.to_le_bytes()); // ncmds = 2
        v.extend_from_slice(&32u32.to_le_bytes()); // sizeofcmds = 32
        v.extend_from_slice(&0u32.to_le_bytes()); // flags
        v.extend_from_slice(&0u32.to_le_bytes()); // reserved
                                                  // 命令 1:LC_SEGMENT_64(0x19),长度 16(头 8 + 填充 8)
        v.extend_from_slice(&0x19u32.to_le_bytes());
        v.extend_from_slice(&16u32.to_le_bytes());
        v.extend_from_slice(&[0u8; 8]);
        // 命令 2:LC_SYMTAB(0x02),长度 16
        v.extend_from_slice(&0x02u32.to_le_bytes());
        v.extend_from_slice(&16u32.to_le_bytes());
        v.extend_from_slice(&[0u8; 8]);
        v
    }

    #[test]
    fn parses_two_load_commands() {
        let data = macho_with_two_load_commands();
        let cmds = parse_load_commands(&data).unwrap();
        assert_eq!(cmds.len(), 2);
        assert_eq!(cmds[0].cmd, 0x19);
        assert_eq!(cmds[0].cmdsize, 16);
        assert_eq!(cmds[0].name(), "LC_SEGMENT_64");
        assert_eq!(cmds[1].name(), "LC_SYMTAB");
    }

    #[test]
    fn load_commands_error_when_truncated() {
        // 头声称有 2 条命令,却没有命令数据 → EOF
        let mut data = macho_with_two_load_commands();
        data.truncate(32); // 只留头
        assert!(matches!(
            parse_load_commands(&data),
            Err(ParseError::UnexpectedEof { .. })
        ));
    }
}
