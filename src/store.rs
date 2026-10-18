use crate::{
    config::DownloadSettings,
    model::{Task, now, safe_name, validate_url},
};
use rusqlite::{Connection, params};
use std::{
    collections::{BTreeMap, HashSet},
    path::Path,
    sync::Mutex,
};

pub struct Store(Mutex<Connection>);
impl Store {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        let db = Connection::open(path)?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;
        CREATE TABLE IF NOT EXISTS tasks(id INTEGER PRIMARY KEY AUTOINCREMENT, url TEXT NOT NULL, name TEXT NOT NULL, status TEXT NOT NULL, message TEXT NOT NULL, created INTEGER NOT NULL, updated INTEGER NOT NULL, attempt INTEGER NOT NULL DEFAULT 0, bytes INTEGER NOT NULL DEFAULT 0, output TEXT, headers TEXT NOT NULL, retry_from TEXT, retry_message TEXT);
        CREATE INDEX IF NOT EXISTS tasks_status ON tasks(status);
        CREATE TABLE IF NOT EXISTS settings(id INTEGER PRIMARY KEY CHECK(id=1), value TEXT NOT NULL);")?;
        ensure_retry_columns(&db)?;
        // 下载进程随服务退出；仅恢复未结束的任务。
        db.execute("UPDATE tasks SET status='pending',message='Server restarted; task requeued' WHERE status IN ('resolving','downloading','verifying')", [])?;
        Ok(Self(Mutex::new(db)))
    }
    pub fn load_settings(&self) -> anyhow::Result<Option<DownloadSettings>> {
        use rusqlite::OptionalExtension;
        let value: Option<String> = self
            .0
            .lock()
            .unwrap()
            .query_row("SELECT value FROM settings WHERE id=1", [], |row| {
                row.get(0)
            })
            .optional()?;
        value
            .map(|value| serde_json::from_str(&value))
            .transpose()
            .map_err(Into::into)
    }
    pub fn save_settings(&self, settings: &DownloadSettings) -> anyhow::Result<()> {
        self.0.lock().unwrap().execute(
            "INSERT INTO settings(id,value) VALUES(1,?) ON CONFLICT(id) DO UPDATE SET value=excluded.value",
            [serde_json::to_string(settings)?],
        )?;
        Ok(())
    }
    fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Task> {
        let headers: String = r.get(10)?;
        Ok(Task {
            id: r.get(0)?,
            url: r.get(1)?,
            name: r.get(2)?,
            status: r.get(3)?,
            message: r.get(4)?,
            created: r.get(5)?,
            updated: r.get(6)?,
            attempt: r.get(7)?,
            bytes: r.get(8)?,
            output: r.get(9)?,
            headers: serde_json::from_str(&headers).unwrap_or_default(),
        })
    }
    pub fn list(&self) -> anyhow::Result<Vec<Task>> {
        let db = self.0.lock().unwrap();
        let mut q = db.prepare("SELECT * FROM tasks ORDER BY id DESC LIMIT 1000")?;
        Ok(q.query_map([], Self::row)?.collect::<Result<_, _>>()?)
    }
    pub fn get(&self, id: i64) -> anyhow::Result<Option<Task>> {
        use rusqlite::OptionalExtension;
        Ok(self
            .0
            .lock()
            .unwrap()
            .query_row("SELECT * FROM tasks WHERE id=?", [id], Self::row)
            .optional()?)
    }
    pub fn add(
        &self,
        raw: &str,
        headers: &BTreeMap<String, String>,
    ) -> anyhow::Result<Option<i64>> {
        let u = validate_url(raw)?;
        let db = self.0.lock().unwrap();
        let mut query=db.prepare("SELECT status,output FROM tasks WHERE url=? AND status IN ('pending','resolving','downloading','verifying','ok','duplicate')")?;
        let rows = query.query_map([raw], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?))
        })?;
        for row in rows {
            let (status, output) = row?;
            if !matches!(status.as_str(), "ok" | "duplicate")
                || output.is_some_and(|p| Path::new(&p).is_file())
            {
                return Ok(None);
            }
        }
        drop(query);
        let name = safe_name(
            u.path_segments()
                .and_then(|mut s| s.next_back())
                .unwrap_or("video"),
        );
        db.execute("INSERT INTO tasks(url,name,status,message,created,updated,headers) VALUES(?,?,'pending','Queued',?,?,?)", params![raw,name,now(),now(),serde_json::to_string(headers)?])?;
        Ok(Some(db.last_insert_rowid()))
    }
    pub fn claim(&self) -> anyhow::Result<Option<Task>> {
        let mut db = self.0.lock().unwrap();
        let tx = db.transaction()?;
        use rusqlite::OptionalExtension;
        let t = tx
            .query_row(
                "SELECT * FROM tasks WHERE status='pending' ORDER BY id LIMIT 1",
                [],
                Self::row,
            )
            .optional()?;
        if let Some(ref t) = t {
            tx.execute(
                "UPDATE tasks SET status='resolving',message='Resolving the video page',updated=? WHERE id=?",
                params![now(), t.id],
            )?;
        }
        tx.commit()?;
        Ok(t)
    }
    pub fn update(
        &self,
        id: i64,
        status: &str,
        msg: &str,
        attempt: u16,
        bytes: u64,
        output: Option<&str>,
    ) -> anyhow::Result<()> {
        self.0.lock().unwrap().execute("UPDATE tasks SET status=?,message=?,updated=?,attempt=?,bytes=?,output=? WHERE id=? AND status!='cancelled'",params![status,msg,now(),attempt,bytes,output,id])?;
        Ok(())
    }
    /// 取消：人工从 `fail`/`dead` 重试过来的任务回到原状态，连原失败原因一起还原；
    /// 其余情况才进入 `cancelled` 终态。
    pub fn cancel(&self, id: i64) -> anyhow::Result<bool> {
        use rusqlite::OptionalExtension;
        let mut db = self.0.lock().unwrap();
        let tx = db.transaction()?;
        let previous = tx
            .query_row(
                "SELECT retry_from,retry_message FROM tasks WHERE id=? AND status IN ('pending','resolving','downloading','verifying')",
                [id],
                |r| Ok((r.get::<_, Option<String>>(0)?, r.get::<_, Option<String>>(1)?)),
            )
            .optional()?;
        let Some((origin, reason)) = previous else {
            return Ok(false);
        };
        let (status, message) = if matches!(origin.as_deref(), Some("fail") | Some("dead")) {
            (
                origin.unwrap_or_default(),
                reason.unwrap_or_else(|| "Cancelled".into()),
            )
        } else {
            ("cancelled".to_string(), "Cancelled".to_string())
        };
        let changed = tx.execute(
            "UPDATE tasks SET status=?,message=?,updated=?,retry_from=NULL,retry_message=NULL WHERE id=? AND status IN ('pending','resolving','downloading','verifying')",
            params![status, message, now(), id],
        )?;
        tx.commit()?;
        Ok(changed > 0)
    }
    /// 重试：一并记下来源状态与原因，`cancel` 才能把任务放回它来的地方。
    pub fn retry(&self, id: i64) -> anyhow::Result<bool> {
        Ok(self.0.lock().unwrap().execute("UPDATE tasks SET status='pending',message='Requeued',updated=?,attempt=0,bytes=0,output=NULL,retry_from=status,retry_message=message WHERE id=? AND status IN ('fail','cancelled','dead')",params![now(),id])?>0)
    }
    /// 人工把失败任务归入失效死链：只有 fail 能转，转完调度器不会再碰它，
    /// 除非人工重试（`retry` 允许 dead 回到 pending）。
    pub fn mark_dead(&self, id: i64) -> anyhow::Result<bool> {
        Ok(self
            .0
            .lock()
            .unwrap()
            .execute(
                "UPDATE tasks SET status='dead',message='Marked as dead',updated=? WHERE id=? AND status='fail'",
                params![now(), id],
            )?
            > 0)
    }
    pub fn delete(&self, id: i64) -> anyhow::Result<bool> {
        use rusqlite::OptionalExtension;
        let db = self.0.lock().unwrap();
        let status: Option<String> = db
            .query_row("SELECT status FROM tasks WHERE id=?", [id], |row| {
                row.get(0)
            })
            .optional()?;
        let Some(status) = status else {
            return Ok(false);
        };
        anyhow::ensure!(
            matches!(status.as_str(), "ok" | "duplicate" | "fail" | "cancelled"),
            "Cancel the task before deleting its record"
        );
        Ok(db.execute("DELETE FROM tasks WHERE id=?", [id])? > 0)
    }
}
/// 老库补列：`cancel` 只有知道"取消前是什么状态、什么原因"，才能把人工重试的任务放回原处。
fn ensure_retry_columns(db: &Connection) -> anyhow::Result<()> {
    let mut columns = HashSet::new();
    let mut statement = db.prepare("PRAGMA table_info(tasks)")?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        columns.insert(row.get::<_, Option<String>>(1)?.unwrap_or_default());
    }
    drop(rows);
    drop(statement);
    for column in ["retry_from", "retry_message"] {
        if !columns.contains(column) {
            db.execute(&format!("ALTER TABLE tasks ADD COLUMN {column} TEXT"), [])?;
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn persistence_and_exact_url_dedup() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("db");
        let s = Store::open(&p).unwrap();
        let h = BTreeMap::new();
        let id = s.add("https://example.com/play?a=1", &h).unwrap().unwrap();
        assert!(s.add("https://example.com/play?a=1", &h).unwrap().is_none());
        assert!(s.add("https://example.com/play?a=2", &h).unwrap().is_some());
        s.claim().unwrap();
        drop(s);
        let s = Store::open(&p).unwrap();
        assert_eq!(s.get(id).unwrap().unwrap().status, "pending");
        s.cancel(id).unwrap();
        s.update(id, "ok", "late result", 1, 0, None).unwrap();
        assert_eq!(s.get(id).unwrap().unwrap().status, "cancelled");
        assert!(s.retry(id).unwrap());
    }
    #[test]
    fn failed_tasks_move_to_dead_and_only_come_back_by_hand() {
        let d = tempfile::tempdir().unwrap();
        let s = Store::open(&d.path().join("db")).unwrap();
        let h = BTreeMap::new();
        let id = s.add("https://example.com/play?a=1", &h).unwrap().unwrap();
        // 只有失败的记录才能被标记为死链。
        assert!(!s.mark_dead(id).unwrap());
        s.update(id, "fail", "No valid media playlist found", 3, 0, None)
            .unwrap();
        assert!(s.mark_dead(id).unwrap());
        assert_eq!(s.get(id).unwrap().unwrap().status, "dead");
        // 调度器只认 pending：死链不会被自动重跑。
        assert!(s.claim().unwrap().is_none());
        // 人工重试把它放回 pending。
        assert!(s.retry(id).unwrap());
        assert_eq!(s.get(id).unwrap().unwrap().status, "pending");
        assert!(s.claim().unwrap().is_some());
    }
    #[test]
    fn cancelling_a_retried_failure_returns_it_to_failed() {
        let d = tempfile::tempdir().unwrap();
        let s = Store::open(&d.path().join("db")).unwrap();
        let h = BTreeMap::new();
        let id = s.add("https://example.com/play?a=1", &h).unwrap().unwrap();
        s.update(id, "fail", "No valid media playlist found", 3, 4096, None)
            .unwrap();
        // 人工重试：失败任务进入待下载队列。
        assert!(s.retry(id).unwrap());
        assert_eq!(s.get(id).unwrap().unwrap().status, "pending");
        // 又取消：应该回到失败列表，并带回原来的失败原因。
        assert!(s.cancel(id).unwrap());
        let task = s.get(id).unwrap().unwrap();
        assert_eq!(task.status, "fail");
        assert_eq!(task.message, "No valid media playlist found");
        // 回到 fail 之后，人工重试仍然可用。
        assert!(s.retry(id).unwrap());
        assert!(s.cancel(id).unwrap());
        assert_eq!(s.get(id).unwrap().unwrap().status, "fail");
    }
    #[test]
    fn cancelling_a_retried_dead_task_returns_it_to_dead() {
        let d = tempfile::tempdir().unwrap();
        let s = Store::open(&d.path().join("db")).unwrap();
        let h = BTreeMap::new();
        let id = s.add("https://example.com/play?a=1", &h).unwrap().unwrap();
        s.update(id, "fail", "Signature expired", 3, 0, None)
            .unwrap();
        assert!(s.mark_dead(id).unwrap());
        assert!(s.retry(id).unwrap());
        assert!(s.cancel(id).unwrap());
        let task = s.get(id).unwrap().unwrap();
        assert_eq!(task.status, "dead");
        // 死链不会被调度器自动重跑。
        assert!(s.claim().unwrap().is_none());
    }
    #[test]
    fn cancelling_a_task_that_never_failed_stays_cancelled() {
        let d = tempfile::tempdir().unwrap();
        let s = Store::open(&d.path().join("db")).unwrap();
        let h = BTreeMap::new();
        let direct = s.add("https://example.com/play?a=1", &h).unwrap().unwrap();
        assert!(s.cancel(direct).unwrap());
        assert_eq!(s.get(direct).unwrap().unwrap().status, "cancelled");
        // 从 cancelled 再重试再取消，还是 cancelled，不会凭空变成 fail。
        assert!(s.retry(direct).unwrap());
        assert!(s.cancel(direct).unwrap());
        assert_eq!(s.get(direct).unwrap().unwrap().status, "cancelled");
        // 已经失败的记录本来就不需要"取消"，取消对它无效，状态保持不变。
        let failed = s.add("https://example.com/play?a=2", &h).unwrap().unwrap();
        s.update(failed, "fail", "boom", 1, 0, None).unwrap();
        assert!(!s.cancel(failed).unwrap());
        assert_eq!(s.get(failed).unwrap().unwrap().status, "fail");
    }
    #[test]
    fn legacy_database_gets_the_retry_columns() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("db");
        {
            let db = Connection::open(&path).unwrap();
            db.execute_batch("CREATE TABLE tasks(id INTEGER PRIMARY KEY AUTOINCREMENT, url TEXT NOT NULL, name TEXT NOT NULL, status TEXT NOT NULL, message TEXT NOT NULL, created INTEGER NOT NULL, updated INTEGER NOT NULL, attempt INTEGER NOT NULL DEFAULT 0, bytes INTEGER NOT NULL DEFAULT 0, output TEXT, headers TEXT NOT NULL);").unwrap();
            db.execute("INSERT INTO tasks(url,name,status,message,created,updated,attempt,bytes,headers) VALUES('https://example.com/play?a=1','a','fail','boom',1,1,3,0,'{}')", []).unwrap();
        }
        let s = Store::open(&path).unwrap();
        let id = s.list().unwrap()[0].id;
        assert!(s.retry(id).unwrap());
        assert!(s.cancel(id).unwrap());
        assert_eq!(s.get(id).unwrap().unwrap().status, "fail");
    }
}
