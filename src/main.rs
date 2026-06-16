use std::error::Error;
use std::fs;

use clap::{Parser, Subcommand};
use reverse::{
    disassemble_view, entropy_blocks, extract_strings, hex_dump, identify, parse_entry_point,
    parse_macho_header, parse_segments, parse_symbols, shannon_entropy, Format,
};

/// revx —— 一个学习用的逆向工程命令行工具。
#[derive(Parser)]
#[command(name = "revx", version, about = "一个学习用的逆向工程工具", long_about = None)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// 显示文件格式与 Mach-O 头(架构 / 类型 / 入口点)
    Info { path: String },
    /// 列出段与节区
    Sections { path: String },
    /// 列出符号(函数 / 全局变量)
    Symbols {
        path: String,
        /// 最多显示多少个
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// 提取可见字符串
    Strings {
        path: String,
        /// 最小长度阈值
        #[arg(long, default_value_t = 6)]
        min: usize,
    },
    /// 十六进制转储
    Hexdump {
        path: String,
        /// 转储多少字节
        #[arg(long, default_value_t = 128)]
        len: usize,
    },
    /// 熵分析(标记疑似加壳)
    Entropy { path: String },
    /// 反汇编 __text 节区
    Disasm {
        path: String,
        /// 反汇编多少条指令
        #[arg(long, default_value_t = 20)]
        count: usize,
    },
}

fn main() {
    // 顶层只负责:跑 run(),出错则友好报告并以非 0 退出码结束。
    if let Err(e) = run() {
        eprintln!("revx: 错误: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Info { path } => cmd_info(&path)?,
        Cmd::Sections { path } => cmd_sections(&path)?,
        Cmd::Symbols { path, limit } => cmd_symbols(&path, limit)?,
        Cmd::Strings { path, min } => cmd_strings(&path, min)?,
        Cmd::Hexdump { path, len } => cmd_hexdump(&path, len)?,
        Cmd::Entropy { path } => cmd_entropy(&path)?,
        Cmd::Disasm { path, count } => cmd_disasm(&path, count)?,
    }
    Ok(())
}

/// 读文件,失败时给出包含路径的友好错误信息。
fn read_file(path: &str) -> Result<Vec<u8>, String> {
    fs::read(path).map_err(|e| format!("无法读取 {path}:{e}"))
}

fn cmd_info(path: &str) -> Result<(), Box<dyn Error>> {
    let bytes = read_file(path)?;
    match identify(&bytes) {
        Ok(fmt) => {
            println!("格式: {fmt:?}");
            if fmt == Format::MachO64 {
                if let Ok(h) = parse_macho_header(&bytes) {
                    println!("  架构    : {:?}", h.arch());
                    println!("  文件类型: {:?}", h.file_type());
                    println!("  加载命令: {}", h.ncmds);
                }
                match parse_entry_point(&bytes) {
                    Ok(Some(off)) => println!("  入口点  : entryoff={off:#x}"),
                    Ok(None) => println!("  入口点  : 无"),
                    Err(e) => println!("  入口点  : (解析失败:{e})"),
                }
            }
        }
        Err(e) => println!("无法解析:{e}"),
    }
    Ok(())
}

fn cmd_sections(path: &str) -> Result<(), Box<dyn Error>> {
    let bytes = read_file(path)?;
    let segs = parse_segments(&bytes)?;
    for s in &segs {
        println!(
            "{:<12} vmaddr={:#018x} vmsize={:#x}",
            s.name, s.vmaddr, s.vmsize
        );
        for sec in &s.sections {
            println!(
                "    {:<16} addr={:#018x} size={:#x}",
                sec.sectname, sec.addr, sec.size
            );
        }
    }
    Ok(())
}

fn cmd_symbols(path: &str, limit: usize) -> Result<(), Box<dyn Error>> {
    let bytes = read_file(path)?;
    let syms = parse_symbols(&bytes)?;
    let named: Vec<_> = syms.iter().filter(|s| !s.name.is_empty()).collect();
    println!("符号: 共 {} 个(有名字 {} 个)", syms.len(), named.len());
    for s in named.iter().take(limit) {
        println!("  {:#018x}  {}", s.value, s.name);
    }
    Ok(())
}

fn cmd_strings(path: &str, min: usize) -> Result<(), Box<dyn Error>> {
    let bytes = read_file(path)?;
    let strings = extract_strings(&bytes, min);
    println!("字符串: 共 {} 条(长度≥{min})", strings.len());
    for (off, s) in &strings {
        println!("  {off:#08x}  {s}");
    }
    Ok(())
}

fn cmd_hexdump(path: &str, len: usize) -> Result<(), Box<dyn Error>> {
    let bytes = read_file(path)?;
    let n = bytes.len().min(len);
    print!("{}", hex_dump(&bytes[..n], 0));
    Ok(())
}

fn cmd_entropy(path: &str) -> Result<(), Box<dyn Error>> {
    let bytes = read_file(path)?;
    println!("整体熵: {:.3} bits/byte", shannon_entropy(&bytes));
    let high: Vec<_> = entropy_blocks(&bytes, 4096)
        .into_iter()
        .filter(|&(_, h)| h > 7.2)
        .collect();
    println!("高熵块(>7.2,4KB/块): {} 个", high.len());
    for (off, h) in high.iter().take(10) {
        println!("  {off:#08x}  熵={h:.3}");
    }
    Ok(())
}

fn cmd_disasm(path: &str, count: usize) -> Result<(), Box<dyn Error>> {
    let bytes = read_file(path)?;
    let segs = parse_segments(&bytes)?;
    let Some(sec) = segs
        .iter()
        .flat_map(|s| &s.sections)
        .find(|s| s.sectname == "__text")
    else {
        println!("未找到 __text 节区");
        return Ok(());
    };
    let start = sec.offset as usize;
    // 每条指令最多 15 字节,留足切片再交给反汇编器按 count 截断。
    let n = (sec.size as usize).min(count * 16);
    if let Some(code) = bytes.get(start..start + n) {
        println!("反汇编 __text(起始 {:#x},前 {count} 条):", sec.addr);
        print!("{}", disassemble_view(code, sec.addr, count));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        // clap 提供的自检:验证我们的命令行定义没有冲突 / 错误。
        Cli::command().debug_assert();
    }

    #[test]
    fn read_file_missing_gives_friendly_error() {
        let e = read_file("/no/such/revx/file/here").unwrap_err();
        assert!(e.contains("无法读取"), "实际: {e}");
    }
}
