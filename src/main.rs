use std::env;
use std::error::Error;
use std::fs;

// 引入我们自己的库 crate(名字就是 Cargo.toml 里的 package name)。
use reverse::{identify, parse_macho_header, Format};

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
                        println!("  cputype   : {:#010x}", h.cputype);
                        println!("  filetype  : {}", h.filetype);
                        println!("  加载命令数: {}", h.ncmds);
                        println!("  命令总大小: {} 字节", h.sizeofcmds);
                    }
                    Err(e) => println!("  (头解析失败:{e})"),
                }
            }
        }
        Err(e) => println!("格式: 无法解析 —— {e}"),
    }

    Ok(())
}
