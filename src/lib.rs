//! revx —— 一个用于学习的逆向工程工具库
//!
//! 模块 A 的第一块积木:根据文件开头的"魔数"识别可执行文件格式。

use iced_x86::{Decoder, DecoderOptions, Formatter, Instruction, NasmFormatter};
use std::fmt;
use std::fmt::Write as _;

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

    /// 读出接下来的 `n` 个字节(作为切片)并前进;不够则返回 `None`。
    pub fn read_bytes(&mut self, n: usize) -> Option<&'a [u8]> {
        let end = self.pos.checked_add(n)?;
        let slice = self.data.get(self.pos..end)?;
        self.pos = end;
        Some(slice)
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

/// LC_SEGMENT_64 命令的类型常量。
pub const LC_SEGMENT_64: u32 = 0x19;

/// LC_MAIN 命令的类型常量(记录程序入口)。
pub const LC_MAIN: u32 = 0x8000_0028;

/// LC_SYMTAB 命令的类型常量(符号表)。
pub const LC_SYMTAB: u32 = 0x02;

/// 一个符号:名字 + 它的值(通常是地址)。
#[derive(Debug, PartialEq, Eq)]
pub struct Symbol {
    /// 符号名(函数 / 全局变量的名字,如 "_main")。
    pub name: String,
    /// 符号的值,对函数 / 变量通常是其虚拟地址。
    pub value: u64,
}

/// 从字符串表 `strtab` 的 `off` 处读取一个以 0 结尾的字符串。
fn cstr_from_strtab(strtab: &[u8], off: usize) -> String {
    let Some(rest) = strtab.get(off..) else {
        return String::new();
    };
    let end = rest.iter().position(|&b| b == 0).unwrap_or(rest.len());
    String::from_utf8_lossy(&rest[..end]).into_owned()
}

/// 解析符号表(LC_SYMTAB),返回所有符号的名字与值。无符号表则返回空表。
pub fn parse_symbols(bytes: &[u8]) -> Result<Vec<Symbol>, ParseError> {
    let header = parse_macho_header(bytes)?;
    let mut r = ByteReader::new(bytes);
    r.seek(32).ok_or(ParseError::UnexpectedEof { offset: 32 })?;

    // 第一步:找到 LC_SYMTAB,读出 4 个字段:符号表偏移/数量、字符串表偏移/大小。
    let mut symtab = None;
    for _ in 0..header.ncmds {
        let start = r.position();
        let cmd = read_u32_or_eof(&mut r, Endian::Little)?;
        let cmdsize = read_u32_or_eof(&mut r, Endian::Little)?;
        if cmd == LC_SYMTAB {
            let symoff = read_u32_or_eof(&mut r, Endian::Little)?;
            let nsyms = read_u32_or_eof(&mut r, Endian::Little)?;
            let stroff = read_u32_or_eof(&mut r, Endian::Little)?;
            let strsize = read_u32_or_eof(&mut r, Endian::Little)?;
            symtab = Some((symoff, nsyms, stroff, strsize));
            break;
        }
        let next = start + cmdsize as usize;
        r.seek(next)
            .ok_or(ParseError::UnexpectedEof { offset: next })?;
    }
    let Some((symoff, nsyms, stroff, strsize)) = symtab else {
        return Ok(Vec::new());
    };

    // 第二步:切出字符串表。
    let str_start = stroff as usize;
    let str_end = str_start
        .checked_add(strsize as usize)
        .ok_or(ParseError::UnexpectedEof { offset: str_start })?;
    let strtab = bytes
        .get(str_start..str_end)
        .ok_or(ParseError::UnexpectedEof { offset: str_start })?;

    // 第三步:逐个读 nlist_64(16 字节),用 n_strx 去字符串表查名字。
    let mut sr = ByteReader::new(bytes);
    sr.seek(symoff as usize).ok_or(ParseError::UnexpectedEof {
        offset: symoff as usize,
    })?;
    let mut syms = Vec::new();
    for _ in 0..nsyms {
        let off = sr.position();
        let n_strx = read_u32_or_eof(&mut sr, Endian::Little)?;
        // 跳过 n_type(u8) + n_sect(u8) + n_desc(u16) = 4 字节。
        sr.read_bytes(4)
            .ok_or(ParseError::UnexpectedEof { offset: off + 4 })?;
        let n_value = read_u64_or_eof(&mut sr, Endian::Little)?;
        let name = cstr_from_strtab(strtab, n_strx as usize);
        syms.push(Symbol {
            name,
            value: n_value,
        });
    }
    Ok(syms)
}

/// 查找程序入口偏移(entryoff):`main` 距文件起点的字节偏移。
///
/// 返回 `Ok(Some(off))` 表示找到 LC_MAIN;`Ok(None)` 表示文件没有入口(如动态库)。
pub fn parse_entry_point(bytes: &[u8]) -> Result<Option<u64>, ParseError> {
    let header = parse_macho_header(bytes)?;
    let mut r = ByteReader::new(bytes);
    r.seek(32).ok_or(ParseError::UnexpectedEof { offset: 32 })?;
    for _ in 0..header.ncmds {
        let start = r.position();
        let cmd = read_u32_or_eof(&mut r, Endian::Little)?;
        let cmdsize = read_u32_or_eof(&mut r, Endian::Little)?;
        if cmd == LC_MAIN {
            // LC_MAIN 体:entryoff(u64) + stacksize(u64)。
            let entryoff = read_u64_or_eof(&mut r, Endian::Little)?;
            return Ok(Some(entryoff));
        }
        let next = start + cmdsize as usize;
        r.seek(next)
            .ok_or(ParseError::UnexpectedEof { offset: next })?;
    }
    Ok(None)
}

/// 读 u64 并把 EOF 转成带偏移的错误。
fn read_u64_or_eof(r: &mut ByteReader, endian: Endian) -> Result<u64, ParseError> {
    let offset = r.position();
    r.read_u64(endian)
        .ok_or(ParseError::UnexpectedEof { offset })
}

/// 把定长(16 字节)、以 0 结尾的名字字段转成 `String`(去掉尾随的 0)。
fn cstr16_to_string(raw: &[u8]) -> String {
    let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
    String::from_utf8_lossy(&raw[..end]).into_owned()
}

/// 一个节区(section):段内更细的划分,如 __TEXT 段里的 __text(代码)、__cstring(字符串)。
#[derive(Debug, PartialEq, Eq)]
pub struct Section {
    /// 节区名,如 "__text"。
    pub sectname: String,
    /// 所属段名,如 "__TEXT"。
    pub segname: String,
    /// 虚拟地址。
    pub addr: u64,
    /// 字节大小。
    pub size: u64,
    /// 在文件里的偏移。
    pub offset: u32,
}

/// 一个段(segment):描述程序某一块在内存里的布局(如 __TEXT 代码段、__DATA 数据段)。
#[derive(Debug, PartialEq, Eq)]
pub struct Segment {
    /// 段名,如 "__TEXT"。
    pub name: String,
    /// 装载到内存后的虚拟地址。
    pub vmaddr: u64,
    /// 在内存里占的字节数。
    pub vmsize: u64,
    /// 在文件里的偏移。
    pub fileoff: u64,
    /// 在文件里占的字节数。
    pub filesize: u64,
    /// 段内含多少个节区(section)。
    pub nsects: u32,
    /// 段内的节区列表。
    pub sections: Vec<Section>,
}

/// 解析所有 LC_SEGMENT_64 段。遍历加载命令,遇到段命令就读出它的字段。
pub fn parse_segments(bytes: &[u8]) -> Result<Vec<Segment>, ParseError> {
    let header = parse_macho_header(bytes)?;
    let mut r = ByteReader::new(bytes);
    r.seek(32).ok_or(ParseError::UnexpectedEof { offset: 32 })?;

    let mut segs = Vec::new();
    for _ in 0..header.ncmds {
        let start = r.position();
        let cmd = read_u32_or_eof(&mut r, Endian::Little)?;
        let cmdsize = read_u32_or_eof(&mut r, Endian::Little)?;
        if cmd == LC_SEGMENT_64 {
            let name_off = r.position();
            let name = {
                let raw = r
                    .read_bytes(16)
                    .ok_or(ParseError::UnexpectedEof { offset: name_off })?;
                cstr16_to_string(raw)
            };
            let vmaddr = read_u64_or_eof(&mut r, Endian::Little)?;
            let vmsize = read_u64_or_eof(&mut r, Endian::Little)?;
            let fileoff = read_u64_or_eof(&mut r, Endian::Little)?;
            let filesize = read_u64_or_eof(&mut r, Endian::Little)?;
            let _maxprot = read_u32_or_eof(&mut r, Endian::Little)?;
            let _initprot = read_u32_or_eof(&mut r, Endian::Little)?;
            let nsects = read_u32_or_eof(&mut r, Endian::Little)?;
            let _flags = read_u32_or_eof(&mut r, Endian::Little)?;
            // 紧跟段头之后是 nsects 个 section_64(每个 80 字节)。
            let mut sections = Vec::new();
            for _ in 0..nsects {
                let off = r.position();
                let sectname = cstr16_to_string(
                    r.read_bytes(16)
                        .ok_or(ParseError::UnexpectedEof { offset: off })?,
                );
                let off2 = r.position();
                let segname = cstr16_to_string(
                    r.read_bytes(16)
                        .ok_or(ParseError::UnexpectedEof { offset: off2 })?,
                );
                let addr = read_u64_or_eof(&mut r, Endian::Little)?;
                let size = read_u64_or_eof(&mut r, Endian::Little)?;
                let offset = read_u32_or_eof(&mut r, Endian::Little)?;
                // 跳过 align/reloff/nreloc/flags/reserved1..3 共 7 个 u32。
                for _ in 0..7 {
                    read_u32_or_eof(&mut r, Endian::Little)?;
                }
                sections.push(Section {
                    sectname,
                    segname,
                    addr,
                    size,
                    offset,
                });
            }
            segs.push(Segment {
                name,
                vmaddr,
                vmsize,
                fileoff,
                filesize,
                nsects,
                sections,
            });
        }
        let next = start + cmdsize as usize;
        r.seek(next)
            .ok_or(ParseError::UnexpectedEof { offset: next })?;
    }
    Ok(segs)
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

/// 把字节按经典 hexdump 三栏格式渲染:`地址  十六进制(16 字节)  |ASCII|`。
///
/// `base` 是第一个字节对应的起始地址(打印在最左列)。每行 16 字节,
/// 不可打印字符在 ASCII 列用 `.` 代替。
pub fn hex_dump(bytes: &[u8], base: u64) -> String {
    let mut out = String::new();
    for (i, chunk) in bytes.chunks(16).enumerate() {
        let addr = base + (i * 16) as u64;
        // 十六进制列:每字节 "xx ",在第 8 字节后多一个空格分组。
        let mut hex = String::new();
        for (j, b) in chunk.iter().enumerate() {
            let _ = write!(hex, "{b:02x} ");
            if j == 7 {
                hex.push(' ');
            }
        }
        // ASCII 列:可打印字符原样,其余用 '.'。
        let ascii: String = chunk
            .iter()
            .map(|&b| {
                if b.is_ascii_graphic() || b == b' ' {
                    b as char
                } else {
                    '.'
                }
            })
            .collect();
        // 十六进制列定宽 49(16*3 + 第 8 字节后的额外空格),不足补齐对齐。
        let _ = writeln!(out, "{addr:08x}  {hex:<49}|{ascii}|");
    }
    out
}

/// 扫描字节,提取所有"长度 ≥ `min_len` 的连续可打印字符"片段。
///
/// 返回 `(起始偏移, 字符串)` 列表。复刻经典 `strings` 命令:
/// 程序里的路径、URL、提示文本往往直接暴露意图。
pub fn extract_strings(bytes: &[u8], min_len: usize) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut cur = String::new();
    for (i, &b) in bytes.iter().enumerate() {
        if b.is_ascii_graphic() || b == b' ' {
            if cur.is_empty() {
                start = i; // 记下这一段的起点
            }
            cur.push(b as char);
        } else if cur.len() >= min_len {
            // 遇到不可打印字符:当前片段够长就收下(take 取走并清空 cur)。
            out.push((start, std::mem::take(&mut cur)));
        } else {
            cur.clear(); // 太短,丢弃
        }
    }
    // 文件结尾处可能还攒着一段。
    if cur.len() >= min_len {
        out.push((start, cur));
    }
    out
}

/// 计算一段字节的香农熵(单位 bits/byte,范围 0.0 ~ 8.0)。
///
/// 熵衡量字节分布的"随机程度":全相同 → 0;均匀用满 256 种值 → 8。
/// 压缩 / 加密数据熵接近 8,普通代码 / 文本熵较低。
pub fn shannon_entropy(bytes: &[u8]) -> f64 {
    if bytes.is_empty() {
        return 0.0;
    }
    // 统计每种字节出现的次数。
    let mut counts = [0usize; 256];
    for &b in bytes {
        counts[b as usize] += 1;
    }
    let len = bytes.len() as f64;
    // H = -Σ p·log2(p)
    let mut h = 0.0;
    for &c in counts.iter() {
        if c > 0 {
            let p = c as f64 / len;
            h -= p * p.log2();
        }
    }
    h
}

/// 按 `block_size` 分块计算熵,返回 `(块起始偏移, 熵值)`。用于定位高熵区段。
pub fn entropy_blocks(bytes: &[u8], block_size: usize) -> Vec<(usize, f64)> {
    bytes
        .chunks(block_size)
        .enumerate()
        .map(|(i, chunk)| (i * block_size, shannon_entropy(chunk)))
        .collect()
}

/// 把一段 x86-64 机器码反汇编成 `(地址, 汇编文本)` 列表。
///
/// `rip` 是这段代码的起始虚拟地址(指令里的相对跳转 / 取址会据此算出绝对地址)。
/// 借助成熟的 `iced-x86` 库——反汇编器极其复杂,绝不该自己手写。
pub fn disassemble(code: &[u8], rip: u64) -> Vec<(u64, String)> {
    // 64 = 64 位模式;with_ip 告诉解码器这段代码的起始地址。
    let mut decoder = Decoder::with_ip(64, code, rip, DecoderOptions::NONE);
    let mut formatter = NasmFormatter::new();
    let mut instr = Instruction::default();
    let mut text = String::new();
    let mut out = Vec::new();
    while decoder.can_decode() {
        decoder.decode_out(&mut instr); // 复用同一个 instr,避免反复分配
        text.clear();
        formatter.format(&instr, &mut text);
        out.push((instr.ip(), text.clone()));
    }
    out
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

    /// 构造一个含单个 __TEXT 段(无节区)的最小 Mach-O。
    fn macho_with_one_segment() -> Vec<u8> {
        let mut v = Vec::new();
        // 头:ncmds=1, sizeofcmds=72
        v.extend_from_slice(&[0xcf, 0xfa, 0xed, 0xfe]);
        v.extend_from_slice(&0x0100_0007u32.to_le_bytes());
        v.extend_from_slice(&3u32.to_le_bytes());
        v.extend_from_slice(&2u32.to_le_bytes());
        v.extend_from_slice(&1u32.to_le_bytes()); // ncmds
        v.extend_from_slice(&72u32.to_le_bytes()); // sizeofcmds
        v.extend_from_slice(&0u32.to_le_bytes());
        v.extend_from_slice(&0u32.to_le_bytes());
        // LC_SEGMENT_64,cmdsize=72
        v.extend_from_slice(&0x19u32.to_le_bytes());
        v.extend_from_slice(&72u32.to_le_bytes());
        let mut name = [0u8; 16];
        name[..6].copy_from_slice(b"__TEXT");
        v.extend_from_slice(&name); // segname[16]
        v.extend_from_slice(&0x1_0000_0000u64.to_le_bytes()); // vmaddr
        v.extend_from_slice(&0x1000u64.to_le_bytes()); // vmsize
        v.extend_from_slice(&0u64.to_le_bytes()); // fileoff
        v.extend_from_slice(&0x1000u64.to_le_bytes()); // filesize
        v.extend_from_slice(&5u32.to_le_bytes()); // maxprot
        v.extend_from_slice(&5u32.to_le_bytes()); // initprot
        v.extend_from_slice(&0u32.to_le_bytes()); // nsects
        v.extend_from_slice(&0u32.to_le_bytes()); // flags
        v
    }

    #[test]
    fn parses_one_segment() {
        let segs = parse_segments(&macho_with_one_segment()).unwrap();
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].name, "__TEXT");
        assert_eq!(segs[0].vmaddr, 0x1_0000_0000);
        assert_eq!(segs[0].vmsize, 0x1000);
        assert_eq!(segs[0].nsects, 0);
    }

    #[test]
    fn segments_skip_non_segment_commands() {
        // 在"一个段 + 一条 LC_SYMTAB"里,只有段被计入
        let mut data = macho_with_one_segment();
        // 改成 2 条命令、sizeofcmds = 72 + 16
        data[16..20].copy_from_slice(&2u32.to_le_bytes()); // ncmds
        data[20..24].copy_from_slice(&88u32.to_le_bytes()); // sizeofcmds
                                                            // 追加一条 LC_SYMTAB(cmdsize=16)
        data.extend_from_slice(&0x02u32.to_le_bytes());
        data.extend_from_slice(&16u32.to_le_bytes());
        data.extend_from_slice(&[0u8; 8]);
        let segs = parse_segments(&data).unwrap();
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].name, "__TEXT");
    }

    /// 含一个 __TEXT 段、段内一个 __text 节区的最小 Mach-O。
    fn macho_with_one_section() -> Vec<u8> {
        let mut v = Vec::new();
        // 头:ncmds=1, sizeofcmds = 72 + 80 = 152
        v.extend_from_slice(&[0xcf, 0xfa, 0xed, 0xfe]);
        v.extend_from_slice(&0x0100_0007u32.to_le_bytes());
        v.extend_from_slice(&3u32.to_le_bytes());
        v.extend_from_slice(&2u32.to_le_bytes());
        v.extend_from_slice(&1u32.to_le_bytes()); // ncmds
        v.extend_from_slice(&152u32.to_le_bytes()); // sizeofcmds
        v.extend_from_slice(&0u32.to_le_bytes());
        v.extend_from_slice(&0u32.to_le_bytes());
        // LC_SEGMENT_64,cmdsize=152,nsects=1
        v.extend_from_slice(&0x19u32.to_le_bytes());
        v.extend_from_slice(&152u32.to_le_bytes());
        let mut sname = [0u8; 16];
        sname[..6].copy_from_slice(b"__TEXT");
        v.extend_from_slice(&sname);
        v.extend_from_slice(&0x1_0000_0000u64.to_le_bytes()); // vmaddr
        v.extend_from_slice(&0x1000u64.to_le_bytes()); // vmsize
        v.extend_from_slice(&0u64.to_le_bytes()); // fileoff
        v.extend_from_slice(&0x1000u64.to_le_bytes()); // filesize
        v.extend_from_slice(&5u32.to_le_bytes()); // maxprot
        v.extend_from_slice(&5u32.to_le_bytes()); // initprot
        v.extend_from_slice(&1u32.to_le_bytes()); // nsects = 1
        v.extend_from_slice(&0u32.to_le_bytes()); // flags
                                                  // section_64(80 字节)
        let mut secn = [0u8; 16];
        secn[..6].copy_from_slice(b"__text");
        v.extend_from_slice(&secn); // sectname
        v.extend_from_slice(&sname); // segname __TEXT
        v.extend_from_slice(&0x1_0000_0f00u64.to_le_bytes()); // addr
        v.extend_from_slice(&0x100u64.to_le_bytes()); // size
        v.extend_from_slice(&0xf00u32.to_le_bytes()); // offset
        for _ in 0..7 {
            v.extend_from_slice(&0u32.to_le_bytes()); // align/reloff/nreloc/flags/reserved1..3
        }
        v
    }

    #[test]
    fn parses_symbol_table() {
        // 头 ncmds=1, sizeofcmds=24;LC_SYMTAB 指向后面的符号项与字符串表。
        let mut v = Vec::new();
        v.extend_from_slice(&[0xcf, 0xfa, 0xed, 0xfe]);
        v.extend_from_slice(&0x0100_0007u32.to_le_bytes());
        v.extend_from_slice(&3u32.to_le_bytes());
        v.extend_from_slice(&2u32.to_le_bytes());
        v.extend_from_slice(&1u32.to_le_bytes()); // ncmds
        v.extend_from_slice(&24u32.to_le_bytes()); // sizeofcmds
        v.extend_from_slice(&0u32.to_le_bytes());
        v.extend_from_slice(&0u32.to_le_bytes());
        // LC_SYMTAB(cmdsize=24):symoff=56, nsyms=1, stroff=72, strsize=7
        v.extend_from_slice(&0x02u32.to_le_bytes());
        v.extend_from_slice(&24u32.to_le_bytes());
        v.extend_from_slice(&56u32.to_le_bytes()); // symoff
        v.extend_from_slice(&1u32.to_le_bytes()); // nsyms
        v.extend_from_slice(&72u32.to_le_bytes()); // stroff
        v.extend_from_slice(&7u32.to_le_bytes()); // strsize
                                                  // 偏移 56:一个 nlist_64(16 字节)
        v.extend_from_slice(&1u32.to_le_bytes()); // n_strx = 1(指向字符串表偏移 1)
        v.push(0x0f); // n_type
        v.push(0x01); // n_sect
        v.extend_from_slice(&0u16.to_le_bytes()); // n_desc
        v.extend_from_slice(&0x1_0000_1fe0u64.to_le_bytes()); // n_value
                                                              // 偏移 72:字符串表 "\0_main\0"
        v.extend_from_slice(b"\0_main\0");
        let syms = parse_symbols(&v).unwrap();
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "_main");
        assert_eq!(syms[0].value, 0x1_0000_1fe0);
    }

    #[test]
    fn no_symbols_when_no_symtab() {
        assert_eq!(
            parse_symbols(&macho_with_one_segment()).unwrap(),
            Vec::new()
        );
    }

    #[test]
    fn finds_entry_point_from_lc_main() {
        // 头 ncmds=1, sizeofcmds=24;一条 LC_MAIN(cmdsize=24, entryoff=0x1234)
        let mut v = Vec::new();
        v.extend_from_slice(&[0xcf, 0xfa, 0xed, 0xfe]);
        v.extend_from_slice(&0x0100_0007u32.to_le_bytes());
        v.extend_from_slice(&3u32.to_le_bytes());
        v.extend_from_slice(&2u32.to_le_bytes());
        v.extend_from_slice(&1u32.to_le_bytes()); // ncmds
        v.extend_from_slice(&24u32.to_le_bytes()); // sizeofcmds
        v.extend_from_slice(&0u32.to_le_bytes());
        v.extend_from_slice(&0u32.to_le_bytes());
        v.extend_from_slice(&0x8000_0028u32.to_le_bytes()); // LC_MAIN
        v.extend_from_slice(&24u32.to_le_bytes()); // cmdsize
        v.extend_from_slice(&0x1234u64.to_le_bytes()); // entryoff
        v.extend_from_slice(&0u64.to_le_bytes()); // stacksize
        assert_eq!(parse_entry_point(&v).unwrap(), Some(0x1234));
    }

    #[test]
    fn no_entry_point_when_no_lc_main() {
        // 只有一个段、没有 LC_MAIN → Ok(None)
        assert_eq!(parse_entry_point(&macho_with_one_segment()).unwrap(), None);
    }

    #[test]
    fn parses_section_inside_segment() {
        let segs = parse_segments(&macho_with_one_section()).unwrap();
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].nsects, 1);
        assert_eq!(segs[0].sections.len(), 1);
        let sec = &segs[0].sections[0];
        assert_eq!(sec.sectname, "__text");
        assert_eq!(sec.segname, "__TEXT");
        assert_eq!(sec.addr, 0x1_0000_0f00);
        assert_eq!(sec.size, 0x100);
        assert_eq!(sec.offset, 0xf00);
    }

    #[test]
    fn disassembles_nop_and_ret() {
        let out = disassemble(&[0x90, 0xc3], 0x1000);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0], (0x1000, "nop".to_string()));
        assert_eq!(out[1], (0x1001, "ret".to_string()));
    }

    #[test]
    fn disassembles_mov_eax_imm() {
        // b8 01 00 00 00 = mov eax, 1
        let out = disassemble(&[0xb8, 0x01, 0x00, 0x00, 0x00], 0x2000);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].0, 0x2000);
        assert!(out[0].1.starts_with("mov eax"), "实际: {}", out[0].1);
    }

    #[test]
    fn entropy_of_empty_is_zero() {
        assert_eq!(shannon_entropy(&[]), 0.0);
    }

    #[test]
    fn entropy_of_uniform_bytes_is_zero() {
        // 全是同一个字节 → 毫无随机性 → 熵 0
        assert_eq!(shannon_entropy(&[0x41; 100]), 0.0);
    }

    #[test]
    fn entropy_of_all_256_values_is_eight() {
        // 0..=255 各出现一次 → 完全均匀 → 熵正好 8 bits/byte
        let data: Vec<u8> = (0..=255).collect();
        assert!((shannon_entropy(&data) - 8.0).abs() < 1e-9);
    }

    #[test]
    fn entropy_blocks_split_correctly() {
        let data = vec![0u8; 10];
        let blocks = entropy_blocks(&data, 4); // 10 → 块 0,4,8(末块 2 字节)
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].0, 0);
        assert_eq!(blocks[1].0, 4);
        assert_eq!(blocks[2].0, 8);
        // 全 0 → 每块熵都是 0
        assert!(blocks.iter().all(|&(_, h)| h == 0.0));
    }

    #[test]
    fn extracts_strings_with_offsets() {
        let data = b"\0hello\0\0world!\0";
        let s = extract_strings(data, 3);
        assert_eq!(s, vec![(1, "hello".to_string()), (8, "world!".to_string())]);
    }

    #[test]
    fn extract_strings_filters_short_runs() {
        // "ab"(2) 太短被过滤;"hello"(5) 保留
        let s = extract_strings(b"ab\0hello", 3);
        assert_eq!(s, vec![(3, "hello".to_string())]);
    }

    #[test]
    fn extract_strings_catches_trailing_run() {
        // 结尾没有终止符,也要能收到
        let s = extract_strings(b"\0\0tail", 3);
        assert_eq!(s, vec![(2, "tail".to_string())]);
    }

    #[test]
    fn hex_dump_one_line() {
        let d = hex_dump(b"ABCD", 0);
        assert!(d.starts_with("00000000  41 42 43 44"), "实际: {d}");
        assert!(d.contains("|ABCD|"), "实际: {d}");
        assert_eq!(d.lines().count(), 1);
    }

    #[test]
    fn hex_dump_non_printable_becomes_dot() {
        let d = hex_dump(&[0x00, 0x41, 0xff], 0);
        // 0x00 和 0xff 不可打印 → '.';0x41 = 'A'
        assert!(d.contains("|.A.|"), "实际: {d}");
    }

    #[test]
    fn hex_dump_multiple_lines_addresses() {
        let data = vec![0u8; 20]; // 20 字节 → 2 行(16 + 4)
        let d = hex_dump(&data, 0x1000);
        let lines: Vec<&str> = d.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("00001000"));
        assert!(lines[1].starts_with("00001010")); // 第二行地址 = 0x1000 + 16
    }
}
