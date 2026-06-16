use std::env;
use std::error::Error;
use std::fs;

// 引入我们自己的库 crate(名字就是 Cargo.toml 里的 package name)。
use reverse::{
    disassemble, entropy_blocks, extract_strings, hex_dump, identify, parse_entry_point,
    parse_load_commands, parse_macho_header, parse_segments, parse_symbols, shannon_entropy,
    Format,
};

// main 现在返回 Result:出错时可以用 ? 直接向上传播,Rust 会帮我们打印错误并以非 0 退出。
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("用法: {} <文件路径>", args[0]);
        std::process::exit(1);
    }
    let path = &args[1];

    // ? :读文件失败(文件不存在 / 没权限)就把 io 错误向上抛,main 结束并报告。
    let bytes = fs::read(path)?;

    println!("文件: {} ({} 字节)", path, bytes.len());
    print!("前16字节: ");
    for b in bytes.iter().take(16) {
        print!("{:02x} ", b);
    }
    println!();

    // identify 返回 Result:成功打印格式,失败打印"为什么"。
    match identify(&bytes) {
        Ok(fmt) => {
            println!("格式: {fmt:?}");
            // 若是 Mach-O 64,进一步解析并打印文件头。
            if fmt == Format::MachO64 {
                match parse_macho_header(&bytes) {
                    Ok(h) => {
                        println!("  magic     : {:#010x}", h.magic);
                        println!("  架构      : {:?}", h.arch());
                        println!("  文件类型  : {:?}", h.file_type());
                        println!("  加载命令数: {}", h.ncmds);
                        println!("  命令总大小: {} 字节", h.sizeofcmds);
                    }
                    Err(e) => println!("  (头解析失败:{e})"),
                }
                // 列出加载命令
                match parse_load_commands(&bytes) {
                    Ok(cmds) => {
                        println!("加载命令:");
                        for (i, c) in cmds.iter().enumerate() {
                            println!("  [{i:2}] {:<20} ({} 字节)", c.name(), c.cmdsize);
                        }
                    }
                    Err(e) => println!("加载命令解析失败:{e}"),
                }
                // 列出段
                match parse_segments(&bytes) {
                    Ok(segs) => {
                        println!("段:");
                        for s in &segs {
                            println!(
                                "  {:<12} vmaddr={:#018x} vmsize={:#x} 节区数={}",
                                s.name, s.vmaddr, s.vmsize, s.nsects
                            );
                            for sec in &s.sections {
                                println!(
                                    "      {:<16} addr={:#018x} size={:#x}",
                                    sec.sectname, sec.addr, sec.size
                                );
                            }
                        }
                    }
                    Err(e) => println!("段解析失败:{e}"),
                }
                // 入口点
                match parse_entry_point(&bytes) {
                    Ok(Some(off)) => println!("入口点: entryoff={off:#x}"),
                    Ok(None) => println!("入口点: 无(可能是动态库)"),
                    Err(e) => println!("入口点解析失败:{e}"),
                }
                // 符号表
                match parse_symbols(&bytes) {
                    Ok(syms) => {
                        let named: Vec<_> = syms.iter().filter(|s| !s.name.is_empty()).collect();
                        println!(
                            "符号: 共 {} 个(有名字 {} 个),前 10 个:",
                            syms.len(),
                            named.len()
                        );
                        for s in named.iter().take(10) {
                            println!("  {:#018x}  {}", s.value, s.name);
                        }
                    }
                    Err(e) => println!("符号表解析失败:{e}"),
                }
            }
        }
        Err(e) => println!("格式: 无法解析 —— {e}"),
    }

    // 文件开头的 hex dump(最多 64 字节)。
    let n = bytes.len().min(64);
    println!("\nhex dump(前 {n} 字节):");
    print!("{}", hex_dump(&bytes[..n], 0));

    // 提取可见字符串(长度 ≥ 6),打印前 15 条。
    let strings = extract_strings(&bytes, 6);
    println!("\n字符串: 共 {} 条(长度≥6),前 15 条:", strings.len());
    for (off, s) in strings.iter().take(15) {
        println!("  {off:#08x}  {s}");
    }

    // 熵分析:整体熵 + 标记高熵块(可能加壳/加密/压缩)。
    println!("\n熵: 整体 {:.3} bits/byte", shannon_entropy(&bytes));
    let high: Vec<_> = entropy_blocks(&bytes, 4096)
        .into_iter()
        .filter(|&(_, h)| h > 7.2)
        .collect();
    println!("  高熵块(>7.2,4KB/块): {} 个", high.len());
    for (off, h) in high.iter().take(5) {
        println!("    {off:#08x}  熵={h:.3}");
    }

    // 反汇编 __text 节区开头的若干字节(若能找到)。
    if let Ok(segs) = parse_segments(&bytes) {
        if let Some(sec) = segs
            .iter()
            .flat_map(|s| &s.sections)
            .find(|s| s.sectname == "__text")
        {
            let start = sec.offset as usize;
            let n = (sec.size as usize).min(48);
            if let Some(code) = bytes.get(start..start + n) {
                println!("\n反汇编 __text(前 {n} 字节,起始地址 {:#x}):", sec.addr);
                for (addr, asm) in disassemble(code, sec.addr) {
                    println!("  {addr:#012x}  {asm}");
                }
            }
        }
    }

    Ok(())
}
