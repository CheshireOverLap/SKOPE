//! Memory Resources
//!
//! 노트/메모 및 TODO 리소스

use std::pin::Pin;
use std::future::Future;
use std::sync::RwLock;

use super::{Resource, ResourceResult};
use crate::protocol::ResourceContent;
use crate::{Note, TodoItem, TodoStatus};

/// 메모리(노트) 리소스
pub struct MemoryResource {
    notes: RwLock<Vec<Note>>,
}

impl MemoryResource {
    pub fn new() -> Self {
        Self {
            notes: RwLock::new(Vec::new()),
        }
    }

    /// 노트 추가
    pub fn add_note(&self, title: String, content: String, tags: Vec<String>) -> Note {
        let note = Note {
            id: uuid::Uuid::new_v4().to_string(),
            title,
            content,
            tags,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs_f64(),
            updated_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs_f64(),
        };

        self.notes.write().unwrap().push(note.clone());
        note
    }

    /// 노트 삭제
    pub fn remove_note(&self, id: &str) -> bool {
        let mut notes = self.notes.write().unwrap();
        if let Some(pos) = notes.iter().position(|n| n.id == id) {
            notes.remove(pos);
            true
        } else {
            false
        }
    }

    /// 노트 업데이트
    pub fn update_note(&self, id: &str, content: String) -> bool {
        let mut notes = self.notes.write().unwrap();
        if let Some(note) = notes.iter_mut().find(|n| n.id == id) {
            note.content = content;
            note.updated_at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs_f64();
            true
        } else {
            false
        }
    }

    /// 모든 노트 조회
    pub fn get_all(&self) -> Vec<Note> {
        self.notes.read().unwrap().clone()
    }
}

impl Default for MemoryResource {
    fn default() -> Self {
        Self::new()
    }
}

impl Resource for MemoryResource {
    fn uri(&self) -> &'static str {
        "memory://notes"
    }

    fn name(&self) -> &'static str {
        "Project Notes"
    }

    fn description(&self) -> &'static str {
        "User notes and memos for the current project"
    }

    fn read(&self) -> Pin<Box<dyn Future<Output = ResourceResult> + Send>> {
        let notes = self.get_all();

        Box::pin(async move {
            let markdown = if notes.is_empty() {
                "# Project Notes\n\n_No notes yet._".to_string()
            } else {
                let mut md = "# Project Notes\n\n".to_string();
                for note in &notes {
                    md.push_str(&format!("## {}\n", note.title));
                    if !note.tags.is_empty() {
                        md.push_str(&format!("Tags: {}\n\n", note.tags.join(", ")));
                    }
                    md.push_str(&note.content);
                    md.push_str("\n\n---\n\n");
                }
                md
            };

            Ok(ResourceContent {
                uri: "memory://notes".to_string(),
                mime_type: Some("text/markdown".to_string()),
                text: Some(markdown),
                blob: None,
            })
        })
    }
}

/// TODO 리소스
pub struct TodoResource {
    items: RwLock<Vec<TodoItem>>,
}

impl TodoResource {
    pub fn new() -> Self {
        Self {
            items: RwLock::new(Vec::new()),
        }
    }

    /// TODO 추가
    pub fn add(&self, content: String) -> TodoItem {
        let item = TodoItem {
            id: uuid::Uuid::new_v4().to_string(),
            content,
            status: TodoStatus::Pending,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs_f64(),
            completed_at: None,
        };

        self.items.write().unwrap().push(item.clone());
        item
    }

    /// TODO 완료 처리
    pub fn complete(&self, id: &str) -> bool {
        self.update_status(id, TodoStatus::Completed)
    }

    /// TODO 상태 변경
    pub fn update_status(&self, id: &str, status: TodoStatus) -> bool {
        let mut items = self.items.write().unwrap();
        if let Some(item) = items.iter_mut().find(|i| i.id == id) {
            item.status = status;
            true
        } else {
            false
        }
    }

    /// TODO 삭제
    pub fn remove(&self, id: &str) -> bool {
        let mut items = self.items.write().unwrap();
        if let Some(pos) = items.iter().position(|i| i.id == id) {
            items.remove(pos);
            true
        } else {
            false
        }
    }

    /// 모든 TODO 조회
    pub fn get_all(&self) -> Vec<TodoItem> {
        self.items.read().unwrap().clone()
    }

    /// 상태별 TODO 조회
    pub fn get_by_status(&self, status: TodoStatus) -> Vec<TodoItem> {
        self.items
            .read()
            .unwrap()
            .iter()
            .filter(|i| i.status == status)
            .cloned()
            .collect()
    }
}

impl Default for TodoResource {
    fn default() -> Self {
        Self::new()
    }
}

impl Resource for TodoResource {
    fn uri(&self) -> &'static str {
        "todos://list"
    }

    fn name(&self) -> &'static str {
        "TODO List"
    }

    fn description(&self) -> &'static str {
        "Project task list with status tracking"
    }

    fn read(&self) -> Pin<Box<dyn Future<Output = ResourceResult> + Send>> {
        let items = self.get_all();

        Box::pin(async move {
            let markdown = if items.is_empty() {
                "# TODO List\n\n_No tasks yet._".to_string()
            } else {
                let mut md = "# TODO List\n\n".to_string();

                // 상태별 그룹화
                let pending: Vec<_> = items.iter().filter(|i| i.status == TodoStatus::Pending).collect();
                let in_progress: Vec<_> = items.iter().filter(|i| i.status == TodoStatus::InProgress).collect();
                let completed: Vec<_> = items.iter().filter(|i| i.status == TodoStatus::Completed).collect();

                if !in_progress.is_empty() {
                    md.push_str("## 🔄 In Progress\n");
                    for item in &in_progress {
                        md.push_str(&format!("- [ ] {}\n", item.content));
                    }
                    md.push('\n');
                }

                if !pending.is_empty() {
                    md.push_str("## ⏳ Pending\n");
                    for item in &pending {
                        md.push_str(&format!("- [ ] {}\n", item.content));
                    }
                    md.push('\n');
                }

                if !completed.is_empty() {
                    md.push_str("## ✅ Completed\n");
                    for item in &completed {
                        md.push_str(&format!("- [x] {}\n", item.content));
                    }
                    md.push('\n');
                }

                md
            };

            Ok(ResourceContent {
                uri: "todos://list".to_string(),
                mime_type: Some("text/markdown".to_string()),
                text: Some(markdown),
                blob: None,
            })
        })
    }
}
