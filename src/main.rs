use std::env;
use std::fs;

fn main() {
    // 1. 收集命令行参数。args[0] 是程序自己的名字,args[1] 才是用户传的文件路径。
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("用法: {} <文件路径>", args[0]);
        return;
    }
    let path = &args[1];

    // 2. 把整个文件读进内存,变成一串字节(Vec<u8>)。
    //    u8 = 无符号 8 位整数 = 正好 1 个字节(0~255)。
    let bytes = fs::read(path).expect("读取文件失败");

    // 3. 打印基本信息和前 16 个字节(十六进制)。
    println!("文件: {} ({} 字节)", path, bytes.len());
    print!("前16字节: ");
    for b in bytes.iter().take(16) {
        print!("{:02x} ", b);
    }
    println!();
}
