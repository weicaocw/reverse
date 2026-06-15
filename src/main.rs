use std::env;
use std::fs;

// 引入我们自己的库 crate(名字就是 Cargo.toml 里的 package name)。
use reverse::detect;

fn main() {
    // args[0] 是程序名,args[1] 才是用户传的文件路径。
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("用法: {} <文件路径>", args[0]);
        std::process::exit(1);
    }
    let path = &args[1];

    // 把整个文件读成一串字节(Vec<u8>)。
    let bytes = fs::read(path).expect("读取文件失败");

    println!("文件: {} ({} 字节)", path, bytes.len());
    print!("前16字节: ");
    for b in bytes.iter().take(16) {
        print!("{:02x} ", b);
    }
    println!();

    // 调用库里的 detect,打印识别出的格式。
    println!("格式: {:?}", detect(&bytes));
}
