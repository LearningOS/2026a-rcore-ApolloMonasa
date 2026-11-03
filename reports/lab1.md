# lab1 实验报告（ch3：多道程序与分时多任务）



## 功能总结

本次实现的系统调用主要用于对 task 信息的读取：通过全局单例 TASK_MANAGER 提供的方法查询当前任务的系统调用计数——只要 TCB 里有的信息理论上都能读取。为此我们扩展了 TCB 数据结构，添加了按系统调用号索引的调用次数数组，并新增了最大系统调用号的配置常量，方便后续修改只改一处。另外两个功能分别涉及读写裸指针处的数据，也让我学到了 unsafe 的实际应用。

## 问答题

### 1. 三个 bad 测例的出错行为

运行环境：RustSBI 0.4.1（随仓库 `bootloader/rustsbi-qemu.bin`，版本记录见 `bootloader/rustsbi-qemu.toml`）；QEMU 10.2.2。

- **bad_address**：意图向地址 `0x0` 写入一个字节（空指针解引用）。CPU 触发 **StorePageFault**，内核读出出错地址（stval=0x0）与出错指令位置（sepc=0x804003a4），打印 `[kernel] PageFault in application, bad addr = 0x0, bad instruction = 0x804003a4, kernel killed it.`
- **bad_instructions**：在 U 态执行 S 态特权指令 `sret`，触发 **IllegalInstruction**。
- **bad_register**：在 U 态用 `csrr` 读取 S 态 CSR `sstatus`，按 RISC-V 规定（访问高于当前特权级的 CSR 一律按非法指令报告）同样触发 **IllegalInstruction**。

三者处置相同：`trap_handler` 判定异常后杀死该应用并调度下一个。值得注意的完整行为是：三个"作死"程序被逐个处决后，power/write 等后续测例**全部正常跑完**——这说明特权级隔离与异常隔离有效：U 态的一切越权行为都被硬件拦截为异常、陷入 S 态内核裁决，单个应用的错误不会拖垮整个系统。

### 2. trap.S 精读（行号对应 `os/src/trap/trap.S`）

**(1) L40：刚进入 `__restore` 时 sp 代表什么？`__restore` 的两种使用情景？**

sp 指向内核栈上的 TrapContext。ch3 版 `__restore` 不再从 a0 取地址，入口统一约定"sp 已指向 TrapContext"。两种情景：① 陷阱处理完成后返回用户态（`__alltraps` → `trap_handler` 返回后顺序落入 `__restore`）；② 内核首次启动一个任务——通过"伪造的 TrapContext + 两层跳板"（`TaskContext::goto_restore` 把 ra 设为 `__restore`、sp 设为该任务 TrapContext 的地址），借中断返回路径**首次**进入 U 态。

**(2) L43-L48：特殊处理了哪些寄存器？对进入用户态的意义？**

`sstatus`、`sepc`、`sscratch` 三个 CSR（先从 TrapContext 读入 t0/t1/t2，再 `csrw` 写回）。它们不属于通用寄存器组、无法用 `ld` 恢复；且直接决定 `sret` 的行为：`sepc` 是 `sret` 的跳转目标（应用断点）；`sstatus` 中的 SPP 等状态位决定 `sret` 后的特权级与中断状态；`sscratch` 换回用户栈指针，供 L60 的交换使用。

**(3) L50-L56：恢复通用寄存器时为何跳过 x2 和 x4？**

x2 是 sp——此刻它正指向内核栈上的 TrapContext，后面 L58 的 `addi`（释放现场）和 L60 的 `csrrw`（换栈）还要用它，不能提前覆盖；用户 sp 由 L60 的交换恢复。x4 是 tp（线程指针），应用不使用，无需恢复（其在 TrapContext 中的槽位仅占位）。

**(4) L60（`csrrw sp, sscratch, sp`）之后，sp 和 sscratch 各有什么意义？**

sp 变为**用户栈指针**（被打断时的应用 sp，此前保存在 sscratch 中）；sscratch 变为**内核栈指针**（此前的 sp）——它成为"下次陷入时找回内核栈"的便签，与 `__alltraps` 首句（L13）的交换互为镜像操作。

**(5) `__restore` 中状态切换发生在哪条指令？为何该指令后进入用户态？**

`sret`（L61）。其硬件语义为：pc ← sepc；特权级 ← sstatus.SPP 记录的模式，随后 SPP 清零。恢复的现场中 SPP=User、sepc=应用断点，故 `sret` 执行后 CPU 在 U 态从断点处继续执行——特权级下降由硬件指令语义完成，无需软件参与。

**(6) L13（`__alltraps` 的同一句 `csrrw`）之后，sp 和 sscratch 的意义？**

交换前：sp=用户栈、sscratch=内核栈；交换后：sp=**内核栈**（可以开始压栈保存现场），sscratch=**用户栈**（暂存用户 sp，随后 L34-L35 将其读出、存入 TrapContext 的 x2 槽位，实现"晚保存 sp"）。

**(7) 从 U 态进入 S 态由哪条指令发生？**

`ecall`（应用主动发起系统调用时）。此外，异常与中断同样引发 U→S 陷入，但指令意义上的入口是 `ecall`。

## 荣誉准则

在完成本次实验的过程（含此前学习的过程）中，我曾分别与以下各位就（与本次实验相关的）以下方面做过交流，还在代码中对应的位置以注释形式记录了具体的交流对象及内容：

AI 编程助教（ZCode，GLM 模型）：概念讲解（Rust 模块系统、RefCell 内部可变性、place expression、块表达式与 as 的解析歧义等）、设计讨论（计数数组的数据归属、统计点位置选择）、代码 review（指出 req=2 分支重复记账的 bug）与报告结构建议。全部代码与报告文字由本人独立编写。

此外，我也参考了以下资料，还在代码中对应的位置以注释形式记录了具体的参考来源及内容：

rCore-Tutorial-Book-v3 教程（rcore-os.github.io）、LearningOS rCore-Tutorial-Guide 实验指导书、课程作业仓库随附的上游代码（rCore-Tutorial-Code ch3 分支）与测例（rCore-Tutorial-Test）。

3. 我独立完成了本次实验除以上方面之外的所有工作，包括代码与文档。
我清楚地知道，从以上方面获得的信息在一定程度上降低了实验难度，可能会影响起评分。

4. 我从未使用过他人的代码，不管是原封不动地复制，还是经过了某些等价转换。
我未曾也不会向他人（含此后各届同学）复制或公开我的实验代码，我有义务妥善保管好它们。
我提交至本实验的评测系统的代码，均无意于破坏或妨碍任何计算机系统的正常运转。
我清楚地知道，以上情况均为本课程纪律所禁止，若违反，对应的实验成绩将按"-100"分计。

## 实验反馈（optional）

工作量适中。测例设计有巧思：行缓冲导致的"两次 write"、`count_syscall` 对自身的统计，都在逼着实现者真正理解"计数发生在分发入口"这件事，比直接断言数值更有教学价值。一个可改进点：指导书问答题引用的 trap.S 行号若能注明对应仓库版本，可减少对不上行号的困惑。
