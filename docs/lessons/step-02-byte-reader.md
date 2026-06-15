# Step 02：造一个"字节游标" `ByteReader`

> 模块：A 基础 ｜ 对应提交：`ad07490` ｜ 测试：✅ 通过 ｜ 上一步：[Step 01](step-01-format-detect.md)

## 0. 一句话目标
做一个能在字节流上"边读边自动前进"的游标 `ByteReader`,并且**越界时返回 `None` 而不是让程序崩溃**。

## 1. 前置回顾
Step 01 我们能识别格式了,但读字节还是"从头数"(`bytes.iter().take(16)`)。真正解析文件头是一连串"接着往下读"的动作:读 4 字节当 magic、再读 4 字节当 cputype……我们需要一个**记得"读到哪了"的对象**,每读一次自动挪位。这就是本步的 `ByteReader`,它是后面所有解析步骤的底座。

更关键的是:逆向工具吃进来的文件是**不可信的**——可能被故意截断、被改畸形。如果读越界就崩溃,工具就废了。所以游标必须**安全**:越界返回"没有"(`None`),而不是 panic。

## 2. 先写测试(TDD·红)
先描述我们想要的两种行为。

**行为一:按顺序读,游标会前进**
```rust
let data = [0xaa, 0xbb, 0xcc];
let mut r = ByteReader::new(&data);
assert_eq!(r.position(), 0);
assert_eq!(r.read_u8(), Some(0xaa));
assert_eq!(r.read_u8(), Some(0xbb));
assert_eq!(r.position(), 2);
```

**行为二:读到末尾后越界,返回 `None` 而不崩**
```rust
let data = [0x01];
let mut r = ByteReader::new(&data);
assert_eq!(r.read_u8(), Some(0x01));
assert_eq!(r.read_u8(), None); // 越界
assert_eq!(r.read_u8(), None); // 再越界也安稳
```

此时 `ByteReader` 还不存在,`cargo test` → **红**:
```
error[E0433]: cannot find type `ByteReader` in this scope
```

## 3. 实现到通过(TDD·绿)
分两部分:先定义"它长什么样"(struct),再定义"它会什么"(impl)。

### 3.1 它长什么样 —— `struct`
```rust
pub struct ByteReader<'a> {
    data: &'a [u8],   // 借用的整段字节(只读)
    pos: usize,       // 当前读到第几个字节
}
```
- **`struct`(结构体)**:把几个相关数据**捆成一个新类型**。`ByteReader` 就是"一段字节 + 一个位置"的组合。可以把它想成一个表格的一行,有两列:`data` 和 `pos`。
- **`data: &'a [u8]`**:这个字段不是自己拥有那段字节,而是**借用**(`&`)别人的。`[u8]` 是字节序列,`&[u8]` 是"借来看的一段字节"。
- **`pos: usize`**:`usize` 是"和机器位宽一样大的无符号整数",专门用来当下标 / 长度。位置不可能是负数,所以用无符号。
- **`<'a>` 是什么?见 §6**,这是本步最需要慢慢嚼的点。

### 3.2 它会什么 —— `impl`
```rust
impl<'a> ByteReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        ByteReader { data, pos: 0 }
    }

    pub fn position(&self) -> usize {
        self.pos
    }

    pub fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    pub fn read_u8(&mut self) -> Option<u8> {
        let byte = *self.data.get(self.pos)?;
        self.pos += 1;
        Some(byte)
    }
}
```
逐个讲:
- **`impl ByteReader { ... }`**:`impl` = implementation(实现),里面写这个类型的**方法**(它"会做的事")。`struct` 是名词,`impl` 里是动词。
- **`fn new(data) -> Self`**:`new` 是约定俗成的"构造函数"名(Rust 没有专门的 constructor 关键字,就用一个返回 `Self` 的普通函数)。`Self` = "当前这个类型",即 `ByteReader`。`ByteReader { data, pos: 0 }` 创建一个实例:`data` 字段用传进来的 `data`(字段名和变量名同名时可简写),`pos` 设为 0。
- **`fn position(&self)`**:`&self` 表示"借用自己、只读"。读取位置不改动游标,所以用不可变借用 `&self`。
- **`fn read_u8(&mut self)`**:`&mut self` 表示"**可变**地借用自己"——因为读一个字节要把 `pos` 往前挪,会**改**到自己,所以必须 `&mut`(mutable,可变)。这也是为什么测试里要写 `let mut r = ...`:只有声明成 `mut` 的变量,才能调用会改它的方法。
- **`self.data.get(self.pos)?`**:`get(i)` 安全地取第 `i` 个元素——存在返回 `Some(&值)`,越界返回 `None`(对比 `self.data[i]` 越界会直接 panic)。结尾那个 **`?`** 是"问号运算符":如果是 `None` 就**立刻让整个函数返回 `None`**;如果是 `Some(x)` 就取出里面的 `x` 继续。一行顶一个 if 判断。
- **`*self.data.get(...)`**:`get` 返回的是 `&u8`(借来的字节),前面的 `*` 是"解引用",把"借来的字节"变成"字节本身"的拷贝(`u8` 很小,拷贝零成本)。
- **`Some(byte)`**:成功时,把读到的字节装进 `Some(...)` 返回。

`cargo test` → **✅ 绿**:
```
running 5 tests
test tests::reader_reads_bytes_in_order ... ok
test tests::reader_returns_none_past_the_end ... ok
...
test result: ok. 5 passed; 0 failed
```

## 4. 改了哪些文件 / 加了什么
- `src/lib.rs`:① 新增 `struct ByteReader<'a>`;② 新增 `impl` 块,含 `new`/`position`/`remaining`/`read_u8`;③ 新增 2 个测试。

## 5. 学到的语法 / 技巧
- **`struct`**:把多个字段捆成一个新类型(我们的"一段字节 + 位置")。
- **`impl`**:给类型写方法。`new`(构造)、`position`/`remaining`(读)、`read_u8`(改)。
- **`&self` vs `&mut self`**:方法以何种方式借用自己。只读用 `&self`;要改自己用 `&mut self`。
- **`Self`**:在 `impl` 里指代"当前类型"的简写。
- **`Option<u8>`**:Rust 表达"可能有、可能没有"的类型,只有两种值:`Some(值)` 或 `None`。
- **`?` 运算符**:遇到 `None`(或后面会讲的错误)就提前返回,否则取出里面的值。是 Rust 处理"可能失败"的招牌写法。
- **`.get(i)` vs `a[i]`**:前者越界返回 `None`(安全),后者越界 panic(崩溃)。
- **`usize`**:用于下标 / 长度的无符号整数。
- **`*`(解引用)**:把"借用"还原成"值本身"。
- **`let mut`**:只有声明为 `mut` 的变量才能被修改、才能调用 `&mut self` 方法。

## 6. 语言设计巧思:生命周期 `<'a>` 到底在防什么
这是 Rust 最独特、新手最懵、但**理解了就豁然开朗**的设计,值得花点篇幅。

**先看它防的灾难**。在 C 语言里你可以这样写:一个结构体里存一个指针,指向某段内存;结果那段内存被释放了,指针还指着它——这叫"悬空指针(dangling pointer)",一访问就是崩溃或被黑客利用。这是几十年来无数安全漏洞的根源。

**Rust 的对策:让编译器在编译期就堵死这种情况**,办法就是生命周期标注 `<'a>`。

把 `<'a>` 读成一句承诺:**"我 `ByteReader` 借用的这段 `data`,它的存活时间不短于我自己。"** 编译器拿着这句承诺去检查所有调用点:只要你试图让游标活得比它借的数据还久(数据先没了、游标还在),编译**直接报错**,根本到不了运行。

打个比方:你借了图书馆一本书(`data`),做了一张读书笔记卡(`ByteReader`),卡上写着"详见那本书第 30 页"。`<'a>` 就是图书馆的规定:**"卡片不能比书还'长寿'"**——如果书要还了(被释放),你的卡片也必须先作废,绝不允许"书没了、卡还指着它"。

为什么字段 `data: &'a [u8]` 和 `impl<'a>` 上都要写 `'a`?因为这是同一个承诺要贯穿"类型定义"和"它的方法",名字对上,编译器才知道说的是同一段借用。

**你现在只需记住**:只要一个 struct 里存了"借来的引用"(`&`),就得给它标生命周期 `<'a>`,意思是"我借的东西比我活得久"。绝大多数时候照着写就对了,编译器会在你违背承诺时精确地告诉你哪里错。这就是 Rust"没有垃圾回收、也没有手动 free,却依然内存安全"的秘密之一。

**另一个设计点:`Option` 取代了"空指针"**。很多语言用 `null` / `nil` 表示"没有值",但你常常忘了检查,一访问就崩(著名的"十亿美元错误")。Rust 没有 `null`,用 `Option<T>`:要么 `Some(值)`、要么 `None`。**编译器强制你处理 `None` 的情况**——你不可能"忘记检查",因为 `Option<u8>` 和 `u8` 是不同类型,不先拆开 `Option` 根本拿不到里面的 `u8`。我们的 `read_u8` 返回 `Option<u8>`,调用者必须面对"可能读不到"这件事。

## 7. 领域知识
**解析 = 在字节流上移动的游标(parser cursor)**。几乎所有二进制 / 协议解析器内部都有这么个"当前位置"的概念:从某偏移读一个字段、前进、再读下一个字段。文件头里的字段是**按固定顺序、固定宽度**排布的(比如 Mach-O 头:magic 4 字节、cputype 4 字节、cpusubtype 4 字节……),游标正是按这个顺序"啃"过去。

**为什么"越界返回 None"在逆向里至关重要**:你分析的样本经常是恶意的或损坏的——故意把文件截断、把长度字段填成超大值,引诱你的解析器去读不存在的内存。用 `.get()` + `Option` 的游标天然抵抗这类攻击:读不到就是 `None`,工具优雅地报告"文件不完整",而不是崩溃或被利用。

## 8. 软件设计理念
**封装状态 + 单一职责**。"读到哪了"这个易错的可变状态(`pos`),被**封装**进 `ByteReader` 内部:外界只能通过 `read_u8()` 这种受控方法去动它,不能随手把 `pos` 改成乱七八糟的值。每个方法只干一件小事(读一个字节 / 报告位置 / 报告剩余)。后面读 u16/u32 都会**复用** `read_u8`,而不是到处重写"取下标 + 边界检查"。把"危险的细节"关进一个小盒子、对外只给安全接口——这就是封装的价值。

## 9. 小测验(自测)
1. `read_u8` 用 `&mut self` 而 `position` 用 `&self`,区别是什么?为什么 `read_u8` 必须 `&mut`?
2. 测试里为什么要写 `let mut r = ...`,把 `r` 声明成 `mut`?
3. `self.data.get(self.pos)?` 里的 `?` 做了什么?如果换成 `self.data[self.pos]` 会有什么不同后果?
4. 结构体字段 `data: &'a [u8]` 上的 `'a` 是在向编译器承诺什么?它防住了哪一类经典 bug?
5. `Option<u8>` 和 `u8` 是同一个类型吗?这对"忘记检查空值"有什么好处?

## 10. 参考答案
1. `&self` = 只读地借用自己,`&mut self` = 可变地借用自己。`read_u8` 要把 `pos += 1`,改动了自身状态,所以必须用 `&mut self`;`position` 只是读 `pos`,不改,用 `&self` 即可。Rust 用这个区分,在编译期防止"本以为只读、却偷偷改了"的错误。
2. 因为 `read_u8(&mut self)` 要求"可变地借用 `r`",而只有声明成 `mut` 的变量才允许被可变借用 / 修改。不写 `mut`,调用 `read_u8` 会编译报错。Rust 默认变量不可变,可变是你要显式声明的——这让"哪里会变"一目了然。
3. `?` 在 `get` 返回 `None`(越界)时,**立刻让 `read_u8` 整个返回 `None`**;否则取出 `Some` 里的 `&u8` 继续。换成 `self.data[self.pos]`:越界时不是返回 `None`,而是直接 **panic 崩溃**——对付恶意 / 损坏样本就危险了。
4. `'a` 承诺"`ByteReader` 借用的这段字节,活得至少和游标一样久"。它防住了 **悬空引用 / 悬空指针**(数据已释放、引用还指着它)这一类经典且危险的内存 bug,而且是在**编译期**就堵死,运行期不会发生。
5. **不是**同一个类型。`Option<u8>` 要么 `Some(x)` 要么 `None`,你必须先把它"拆开"(用 `?`、`match`、`if let` 等)才能拿到里面的 `u8`。正因类型不同,编译器**强制**你面对"可能没有值"的情况,从根上消灭了"忘记检查 null 然后崩溃"这个老大难问题。

## 11. 下一步预告
Step 03:用 `read_u8` 这块积木,拼出 `read_u16` / `read_u32` / `read_u64`——一次读多个字节、拼成一个更大的整数。这里会正面遭遇 **字节序(大端 / 小端)**:同样四个字节,顺序不同拼出的数值天差地别。我们会用 `from_le_bytes` / `from_be_bytes` 优雅解决,并把 Step 01 见过的 `cf fa ed fe` 真正还原成 `0xFEEDFACF`。
