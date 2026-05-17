from __future__ import annotations

import argparse
import os
import random
import sqlite3
from datetime import date, datetime, time, timedelta
from pathlib import Path

DEMO_MARKER = "[FORGERUST_DEMO_SEED]"

TOPICS = [
    ("Rust", "Rust language interview topic"),
    ("Java", "Java language and JVM interview topic"),
    ("操作系统", "Operating system interview topic"),
    ("计算机网络", "Computer networking interview topic"),
    ("数据库", "Database interview topic"),
    ("数据结构", "Data structure interview topic"),
    ("Linux", "Linux command line and server interview topic"),
]

QUESTIONS = [
    ("SINGLE", "Rust 中用于声明可变变量的关键字组合是？", ["A. let", "B. let mut", "C. var", "D. mutable"], "Rust", 1, "B", "Rust 默认变量不可变，需要使用 let mut 声明可变绑定。"),
    ("ESSAY", "解释 Rust 所有权、借用和生命周期分别解决什么问题。", None, "Rust", 4, "所有权负责资源释放，借用允许不转移所有权地访问数据，生命周期帮助编译器确认引用有效范围。", "回答应覆盖内存安全、悬垂引用和无 GC 的资源管理。"),
    ("SINGLE", "Rust 中 Result<T, E> 通常用于表达什么？", ["A. 可空值", "B. 错误处理", "C. 多线程", "D. 宏展开"], "Rust", 2, "B", "Result 用 Ok/Err 表示可能失败的计算。"),
    ("ESSAY", "为什么 Rust 可以在没有垃圾回收的情况下保证内存安全？", None, "Rust", 5, "编译期所有权检查、借用规则和生命周期约束共同避免悬垂引用、数据竞争和重复释放。", "重点说明编译期检查，而不是运行时 GC。"),
    ("SINGLE", "Java 中 HashMap 默认允许几个 null key？", ["A. 0", "B. 1", "C. 多个", "D. 取决于泛型"], "Java", 2, "B", "HashMap 允许一个 null key，多个 null value。"),
    ("ESSAY", "简述 JVM 内存区域中堆和栈的主要区别。", None, "Java", 3, "堆存放对象实例并由 GC 管理；栈存放方法调用帧、局部变量表和操作数栈，随线程和方法调用变化。", "可补充线程私有/共享、生命周期和异常。"),
    ("SINGLE", "Java 中 synchronized 主要依赖哪类机制保证互斥？", ["A. 对象监视器", "B. 类加载器", "C. 字节码解释器", "D. 泛型擦除"], "Java", 3, "A", "synchronized 基于 monitorenter/monitorexit 和对象监视器实现。"),
    ("ESSAY", "谈谈 ArrayList 和 LinkedList 在查询、插入上的差异。", None, "Java", 2, "ArrayList 基于连续数组，随机访问快；LinkedList 基于链表，定位慢但已知节点附近插入删除成本较低。", "答题应避免简单说 LinkedList 插入一定快。"),
    ("SINGLE", "进程和线程的主要区别是？", ["A. 线程拥有独立地址空间", "B. 进程是资源分配单位，线程是调度单位", "C. 进程不能并发", "D. 线程不能共享内存"], "操作系统", 2, "B", "进程拥有独立资源，线程共享进程资源并作为 CPU 调度基本单位。"),
    ("ESSAY", "说明死锁产生的四个必要条件，以及常见预防方法。", None, "操作系统", 4, "互斥、占有并等待、不可抢占、循环等待。预防可破坏其中一个条件，如资源有序分配或一次性申请。", "答题可结合银行家算法、超时回退。"),
    ("SINGLE", "虚拟内存主要解决的问题是？", ["A. CPU 频率不足", "B. 进程地址空间隔离和内存扩展", "C. 硬盘坏道", "D. 网络延迟"], "操作系统", 3, "B", "虚拟内存提供独立地址空间、保护和按需调页能力。"),
    ("ESSAY", "系统调用和普通函数调用有什么区别？", None, "操作系统", 3, "系统调用会从用户态切换到内核态，请求内核提供受保护资源；普通函数调用一般只在同一特权级内跳转。", "可提到上下文切换成本和权限边界。"),
    ("SINGLE", "TCP 三次握手的主要目的是什么？", ["A. 加密数据", "B. 确认双方收发能力并同步序列号", "C. 压缩报文", "D. 清空缓存"], "计算机网络", 2, "B", "三次握手确认双方发送和接收能力，并协商初始序列号。"),
    ("ESSAY", "为什么 TCP 需要 TIME_WAIT 状态？", None, "计算机网络", 4, "TIME_WAIT 确保最后 ACK 可重传，并让旧连接报文在网络中自然消失，避免影响后续相同四元组连接。", "回答应说明 2MSL 和可靠关闭。"),
    ("SINGLE", "HTTP 状态码 304 表示什么？", ["A. 永久重定向", "B. 未修改，可使用缓存", "C. 请求错误", "D. 服务端异常"], "计算机网络", 2, "B", "304 Not Modified 表示资源未变化，客户端可使用缓存。"),
    ("ESSAY", "HTTPS 相比 HTTP 增加了哪些安全能力？", None, "计算机网络", 3, "通过 TLS 提供身份认证、加密传输和完整性校验，防止窃听、篡改和部分中间人攻击。", "可补充证书链和密钥协商。"),
    ("SINGLE", "数据库索引最常见的底层结构是？", ["A. B+ 树", "B. 栈", "C. 队列", "D. 哈夫曼树"], "数据库", 2, "A", "关系型数据库常用 B+ 树索引以支持范围查询和磁盘友好访问。"),
    ("ESSAY", "事务 ACID 分别代表什么？", None, "数据库", 3, "原子性、一致性、隔离性、持久性。", "答题应结合回滚、约束、并发隔离和日志持久化。"),
    ("SINGLE", "SQL 中 HAVING 通常用于哪里？", ["A. 过滤分组后的结果", "B. 排序", "C. 建表", "D. 删除索引"], "数据库", 2, "A", "WHERE 过滤行，HAVING 过滤 GROUP BY 后的聚合结果。"),
    ("ESSAY", "解释数据库中的脏读、不可重复读和幻读。", None, "数据库", 4, "脏读读到未提交数据；不可重复读同一行两次读取结果不同；幻读同一条件两次查询出现新增或删除的行。", "可联系隔离级别说明。"),
    ("SINGLE", "二分查找要求数据满足什么条件？", ["A. 随机分布", "B. 有序并可随机访问", "C. 必须是链表", "D. 必须无重复"], "数据结构", 1, "B", "二分查找依赖有序性和中间位置快速访问。"),
    ("ESSAY", "比较栈和队列的访问规则，并举例应用场景。", None, "数据结构", 2, "栈是后进先出，适合函数调用、括号匹配；队列是先进先出，适合任务调度和 BFS。", "可说明 deque 的扩展。"),
    ("SINGLE", "红黑树属于哪类数据结构？", ["A. 自平衡二叉搜索树", "B. 堆", "C. 图", "D. 哈希表"], "数据结构", 3, "A", "红黑树通过颜色和旋转保持近似平衡。"),
    ("ESSAY", "为什么哈希表平均查询复杂度是 O(1)，最坏情况可能退化？", None, "数据结构", 3, "良好哈希函数和低负载因子下冲突少，平均 O(1)；大量冲突时桶内查找可能退化。", "可提及链地址、开放寻址和扩容。"),
    ("SINGLE", "Linux 中查看当前目录文件列表常用命令是？", ["A. ls", "B. cd", "C. pwd", "D. ps"], "Linux", 1, "A", "ls 用于列出目录内容。"),
    ("ESSAY", "如何排查 Linux 服务端 CPU 占用过高？", None, "Linux", 4, "先用 top/htop 定位进程，再用 ps、pidstat、perf、日志和线程栈分析热点，结合业务时间线判断原因。", "答题应体现分层定位思路。"),
    ("SINGLE", "Linux 中 grep 的主要用途是什么？", ["A. 搜索文本", "B. 压缩文件", "C. 修改权限", "D. 管理进程"], "Linux", 1, "A", "grep 用于按模式搜索文本。"),
    ("ESSAY", "解释 chmod 755 的含义。", None, "Linux", 2, "所有者可读写执行，组用户和其他用户可读执行。", "数字 7=4+2+1，5=4+1。"),
]

SESSION_PLAN = [
    (-18, ["Rust", "数据结构"], [92, 86, 80, 78, 88, 84]),
    (-16, ["Java", "数据库"], [70, 66, 58, 74, 80, 62]),
    (-14, ["操作系统"], [55, 60, 68, 72, 50, 64]),
    (-12, ["计算机网络"], [62, 75, 70, 58, 82, 78]),
    (-10, ["Rust", "Linux"], [88, 91, 84, 76, 80, 72]),
    (-8, ["数据库", "数据结构"], [74, 82, 86, 68, 90, 72]),
    (-7, ["Java"], [64, 72, 78, 66, 70, 58]),
    (-6, ["操作系统", "Linux"], [52, 68, 60, 74, 57, 65]),
    (-5, ["计算机网络", "数据库"], [76, 81, 69, 88, 72, 84]),
    (-4, ["Rust"], [90, 94, 82, 86, 78, 91]),
    (-3, ["数据结构", "Java"], [85, 73, 79, 88, 92, 68]),
    (-2, ["操作系统", "计算机网络"], [61, 64, 77, 83, 59, 72]),
    (-1, ["Linux", "数据库"], [80, 76, 91, 88, 70, 84]),
    (0, ["Rust", "计算机网络"], [94, 88, 82, 76, 90, 86]),
]


def ensure_schema(conn: sqlite3.Connection) -> None:
    conn.executescript(
        """
        CREATE TABLE IF NOT EXISTS questions (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            question_type   TEXT    NOT NULL,
            content         TEXT    NOT NULL UNIQUE,
            options         TEXT,
            tags            TEXT    NOT NULL,
            difficulty      INTEGER NOT NULL,
            standard_answer TEXT    NOT NULL DEFAULT '',
            explanation     TEXT    NOT NULL DEFAULT ''
        );

        CREATE TABLE IF NOT EXISTS training_sessions (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            created_at    TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
            total_count   INTEGER NOT NULL,
            correct_count INTEGER NOT NULL,
            average_score REAL    NOT NULL,
            skipped_count INTEGER NOT NULL DEFAULT 0,
            tags          TEXT    NOT NULL DEFAULT ''
        );

        CREATE TABLE IF NOT EXISTS training_records (
            id             INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id     INTEGER NOT NULL REFERENCES training_sessions(id) ON DELETE CASCADE,
            question_id    INTEGER NOT NULL REFERENCES questions(id) ON DELETE CASCADE,
            user_answer    TEXT    NOT NULL DEFAULT '',
            score          INTEGER NOT NULL DEFAULT 0,
            is_correct     INTEGER,
            skipped        INTEGER NOT NULL DEFAULT 0,
            manually_added INTEGER NOT NULL DEFAULT 0,
            time_spent     INTEGER NOT NULL DEFAULT 0,
            created_at     TEXT    NOT NULL DEFAULT (datetime('now', 'localtime'))
        );

        CREATE TABLE IF NOT EXISTS topics (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            name        TEXT    NOT NULL UNIQUE,
            description TEXT    NOT NULL DEFAULT '',
            created_at  TEXT    NOT NULL DEFAULT (datetime('now', 'localtime'))
        );

        CREATE TABLE IF NOT EXISTS mock_interviews (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            created_at    TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
            ended_at      TEXT,
            tags          TEXT    NOT NULL DEFAULT '',
            question_count INTEGER NOT NULL,
            average_score REAL    NOT NULL DEFAULT 0,
            summary       TEXT    NOT NULL DEFAULT '',
            status        TEXT    NOT NULL DEFAULT 'active'
        );

        CREATE TABLE IF NOT EXISTS mock_interview_turns (
            id               INTEGER PRIMARY KEY AUTOINCREMENT,
            interview_id     INTEGER NOT NULL REFERENCES mock_interviews(id) ON DELETE CASCADE,
            question_id      INTEGER NOT NULL REFERENCES questions(id) ON DELETE CASCADE,
            question_content TEXT    NOT NULL,
            user_answer      TEXT    NOT NULL DEFAULT '',
            ai_comment       TEXT    NOT NULL DEFAULT '',
            follow_up        TEXT    NOT NULL DEFAULT '',
            follow_up_answer TEXT    NOT NULL DEFAULT '',
            score            INTEGER NOT NULL DEFAULT 0,
            created_at       TEXT    NOT NULL DEFAULT (datetime('now', 'localtime'))
        );
        """
    )


def cleanup_demo_data(conn: sqlite3.Connection) -> None:
    demo_question_ids = [
        row[0]
        for row in conn.execute(
            "SELECT id FROM questions WHERE explanation LIKE ?",
            (f"%{DEMO_MARKER}%",),
        ).fetchall()
    ]
    if not demo_question_ids:
        return

    placeholders = ",".join("?" for _ in demo_question_ids)
    demo_session_ids = [
        row[0]
        for row in conn.execute(
            f"SELECT DISTINCT session_id FROM training_records WHERE question_id IN ({placeholders})",
            demo_question_ids,
        ).fetchall()
    ]
    demo_interview_ids = [
        row[0]
        for row in conn.execute(
            f"SELECT DISTINCT interview_id FROM mock_interview_turns WHERE question_id IN ({placeholders})",
            demo_question_ids,
        ).fetchall()
    ]

    if demo_session_ids:
        session_placeholders = ",".join("?" for _ in demo_session_ids)
        conn.execute(
            f"DELETE FROM training_records WHERE session_id IN ({session_placeholders})",
            demo_session_ids,
        )
        conn.execute(
            f"DELETE FROM training_sessions WHERE id IN ({session_placeholders})",
            demo_session_ids,
        )

    if demo_interview_ids:
        interview_placeholders = ",".join("?" for _ in demo_interview_ids)
        conn.execute(
            f"DELETE FROM mock_interview_turns WHERE interview_id IN ({interview_placeholders})",
            demo_interview_ids,
        )
        conn.execute(
            f"DELETE FROM mock_interviews WHERE id IN ({interview_placeholders})",
            demo_interview_ids,
        )

    conn.execute(f"DELETE FROM questions WHERE id IN ({placeholders})", demo_question_ids)


def insert_topics(conn: sqlite3.Connection) -> None:
    conn.executemany(
        """
        INSERT INTO topics (name, description)
        VALUES (?, ?)
        ON CONFLICT(name) DO UPDATE SET description = excluded.description
        """,
        TOPICS,
    )


def insert_questions(conn: sqlite3.Connection) -> dict[str, list[int]]:
    by_tag: dict[str, list[int]] = {topic: [] for topic, _ in TOPICS}
    for qtype, content, options, tag, difficulty, answer, explanation in QUESTIONS:
        options_json = None
        if options is not None:
            escaped = [item.replace('"', '\\"') for item in options]
            options_json = "[" + ", ".join(f'"{item}"' for item in escaped) + "]"
        conn.execute(
            """
            INSERT INTO questions
                (question_type, content, options, tags, difficulty, standard_answer, explanation)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            """,
            (
                qtype,
                content,
                options_json,
                tag,
                difficulty,
                answer,
                f"{explanation}\n\n{DEMO_MARKER}",
            ),
        )
        by_tag[tag].append(conn.execute("SELECT last_insert_rowid()").fetchone()[0])
    return by_tag


def created_at_for(day_offset: int, index: int) -> str:
    base = datetime.combine(date.today() + timedelta(days=day_offset), time(9, 10))
    return (base + timedelta(hours=index % 5, minutes=7 * index)).strftime("%Y-%m-%d %H:%M:%S")


def insert_training_data(conn: sqlite3.Connection, by_tag: dict[str, list[int]]) -> None:
    rng = random.Random(20260517)
    for session_index, (day_offset, tags, scores) in enumerate(SESSION_PLAN):
        question_pool = [qid for tag in tags for qid in by_tag[tag]]
        if len(question_pool) < len(scores):
            question_pool = question_pool * 2
        rng.shuffle(question_pool)
        selected_questions = question_pool[: len(scores)]
        correct_count = sum(1 for score in scores if score >= 60)
        skipped_count = 1 if session_index in {2, 11} else 0
        average_score = sum(scores) / len(scores)
        session_time = created_at_for(day_offset, session_index)

        cursor = conn.execute(
            """
            INSERT INTO training_sessions
                (created_at, total_count, correct_count, average_score, skipped_count, tags)
            VALUES (?, ?, ?, ?, ?, ?)
            """,
            (session_time, len(scores), correct_count, average_score, skipped_count, ",".join(tags)),
        )
        session_id = cursor.lastrowid

        for record_index, (question_id, score) in enumerate(zip(selected_questions, scores)):
            skipped = 1 if skipped_count and record_index == len(scores) - 1 else 0
            is_correct = 0 if skipped else int(score >= 60)
            user_answer = "演示回答：先给出核心概念，再结合项目或场景补充取舍。"
            if score < 60:
                user_answer = "演示回答：只答出了部分关键词，还缺少边界条件和完整推理。"
            record_time = created_at_for(day_offset, session_index + record_index)
            conn.execute(
                """
                INSERT INTO training_records
                    (session_id, question_id, user_answer, score, is_correct, skipped, manually_added, time_spent, created_at)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    session_id,
                    question_id,
                    user_answer,
                    score,
                    is_correct,
                    skipped,
                    int(score < 60 and record_index % 2 == 0),
                    rng.randint(35, 180),
                    record_time,
                ),
            )


def insert_mock_interviews(conn: sqlite3.Connection, by_tag: dict[str, list[int]]) -> None:
    rust_question = by_tag["Rust"][1]
    os_question = by_tag["操作系统"][1]
    network_question = by_tag["计算机网络"][1]
    question_rows = {
        row[0]: row[1]
        for row in conn.execute(
            "SELECT id, content FROM questions WHERE id IN (?, ?, ?)",
            (rust_question, os_question, network_question),
        ).fetchall()
    }
    started = created_at_for(-1, 20)
    ended = created_at_for(-1, 23)
    cursor = conn.execute(
        """
        INSERT INTO mock_interviews
            (created_at, ended_at, tags, question_count, average_score, summary, status)
        VALUES (?, ?, ?, ?, ?, ?, 'finished')
        """,
        (
            started,
            ended,
            "Rust,操作系统,计算机网络",
            3,
            82.3,
            "整体表达清晰，Rust 所有权和网络连接关闭解释较好；操作系统死锁部分还可以补充预防策略和实际案例。",
        ),
    )
    interview_id = cursor.lastrowid
    turns = [
        (rust_question, "从所有权、借用和生命周期三个层面解释内存安全。", "回答结构完整，能联系无 GC 资源释放。", "如果发生可变借用和不可变借用同时存在，编译器为什么要拒绝？", "因为会破坏别名可变性规则，可能导致数据竞争或读写不一致。", 88),
        (os_question, "死锁需要互斥、占有并等待、不可抢占和循环等待。", "必要条件说全了，但预防手段可以更具体。", "资源有序分配破坏的是哪个条件？", "破坏循环等待条件。", 76),
        (network_question, "TIME_WAIT 用于处理最后 ACK 和旧报文残留。", "说明准确，可以补充 2MSL 的含义。", "为什么主动关闭方通常进入 TIME_WAIT？", "因为主动关闭方发送最后 ACK，需要负责可能的重传。", 83),
    ]
    for index, (question_id, answer, comment, follow_up, follow_answer, score) in enumerate(turns):
        conn.execute(
            """
            INSERT INTO mock_interview_turns
                (interview_id, question_id, question_content, user_answer, ai_comment, follow_up, follow_up_answer, score, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                interview_id,
                question_id,
                question_rows[question_id],
                answer,
                comment,
                follow_up,
                follow_answer,
                score,
                created_at_for(-1, 20 + index),
            ),
        )


def seed_database(db_path: Path) -> dict[str, int]:
    db_path.parent.mkdir(parents=True, exist_ok=True)
    with sqlite3.connect(db_path) as conn:
        conn.execute("PRAGMA foreign_keys = ON")
        ensure_schema(conn)
        cleanup_demo_data(conn)
        insert_topics(conn)
        by_tag = insert_questions(conn)
        insert_training_data(conn, by_tag)
        insert_mock_interviews(conn, by_tag)
        conn.commit()

        demo_question_count = conn.execute(
            "SELECT COUNT(*) FROM questions WHERE explanation LIKE ?",
            (f"%{DEMO_MARKER}%",),
        ).fetchone()[0]
        demo_record_count = conn.execute(
            """
            SELECT COUNT(*)
            FROM training_records r
            JOIN questions q ON q.id = r.question_id
            WHERE q.explanation LIKE ?
            """,
            (f"%{DEMO_MARKER}%",),
        ).fetchone()[0]
        demo_session_count = conn.execute(
            """
            SELECT COUNT(DISTINCT r.session_id)
            FROM training_records r
            JOIN questions q ON q.id = r.question_id
            WHERE q.explanation LIKE ?
            """,
            (f"%{DEMO_MARKER}%",),
        ).fetchone()[0]
        wrong_count = conn.execute(
            """
            SELECT COUNT(DISTINCT q.id)
            FROM training_records r
            JOIN questions q ON q.id = r.question_id
            WHERE q.explanation LIKE ?
              AND (r.score < 60 OR r.is_correct = 0 OR r.manually_added = 1)
            """,
            (f"%{DEMO_MARKER}%",),
        ).fetchone()[0]
        interview_count = conn.execute(
            """
            SELECT COUNT(DISTINCT i.id)
            FROM mock_interviews i
            JOIN mock_interview_turns t ON t.interview_id = i.id
            JOIN questions q ON q.id = t.question_id
            WHERE q.explanation LIKE ?
            """,
            (f"%{DEMO_MARKER}%",),
        ).fetchone()[0]

    return {
        "questions": demo_question_count,
        "sessions": demo_session_count,
        "records": demo_record_count,
        "wrong_questions": wrong_count,
        "mock_interviews": interview_count,
    }


def known_database_paths(repo_root: Path) -> list[Path]:
    paths = [repo_root / "src-tauri" / "forgerust.db"]
    appdata = os.environ.get("APPDATA")
    if appdata:
        paths.append(Path(appdata) / "com.asus.forgerust" / "forgerust.db")
    return paths


def main() -> None:
    parser = argparse.ArgumentParser(description="Seed ForgeRust demo data for midterm presentation.")
    parser.add_argument("--db", type=Path, help="Seed a specific SQLite database path.")
    parser.add_argument("--all-known", action="store_true", help="Seed src-tauri and Tauri app data databases.")
    args = parser.parse_args()

    repo_root = Path(__file__).resolve().parents[1]
    targets = [args.db] if args.db else known_database_paths(repo_root)
    if not args.all_known and args.db is None:
        targets = [repo_root / "src-tauri" / "forgerust.db"]

    seen: set[Path] = set()
    for target in targets:
        if target is None:
            continue
        target = target.resolve()
        if target in seen:
            continue
        seen.add(target)
        summary = seed_database(target)
        print(
            f"{target}: "
            f"{summary['questions']} questions, "
            f"{summary['sessions']} sessions, "
            f"{summary['records']} records, "
            f"{summary['wrong_questions']} wrong-book items, "
            f"{summary['mock_interviews']} mock interview"
        )


if __name__ == "__main__":
    main()
