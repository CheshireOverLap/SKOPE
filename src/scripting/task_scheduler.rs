//! Coroutine-based Task Scheduler
//!
//! Implements `task.wait(seconds)`, `task.spawn(fn)`, `task.delay(seconds, fn)`,
//! `task.defer(fn)`, and `task.cancel(id)`.
//!
//! Lua 5.4 yield rules: `coroutine.yield()` cannot cross C-call boundaries,
//! so `task.wait()` is a pure Lua function that wraps `coroutine.yield(seconds)`.
//! `task.spawn()` is a Rust function that creates a coroutine, resumes it,
//! and registers it with the scheduler if it yields.

use mlua::{Lua, Result as LuaResult, Value, Function, Thread, RegistryKey};

/// A coroutine waiting to be resumed.
struct WaitingThread {
    /// Registry key holding the Lua coroutine
    registry_key: RegistryKey,
    /// Absolute time (elapsed seconds) when this thread should be resumed
    resume_at: f64,
    /// Entity that owns this coroutine (for cleanup on entity destroy)
    owner_entity: Option<u64>,
    /// Unique thread ID (for cancellation)
    thread_id: u64,
}

/// A function deferred to the next frame.
struct DeferredThread {
    registry_key: RegistryKey,
    owner_entity: Option<u64>,
}

/// Coroutine scheduler — manages waiting/deferred threads.
pub struct TaskScheduler {
    waiting: Vec<WaitingThread>,
    deferred: Vec<DeferredThread>,
    next_thread_id: u64,
}

impl Default for TaskScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskScheduler {
    pub fn new() -> Self {
        Self {
            waiting: Vec::new(),
            deferred: Vec::new(),
            next_thread_id: 1,
        }
    }

    /// Tick the scheduler — resume all threads whose wait time has elapsed.
    ///
    /// Called once per frame from the ECS scripting system chain.
    pub fn tick(&mut self, lua: &Lua, elapsed: f64) -> LuaResult<()> {
        // 1. Process deferred threads (scheduled from previous frame)
        let deferred = std::mem::take(&mut self.deferred);
        for deferred_info in deferred {
            let thread: Thread = lua.registry_value(&deferred_info.registry_key)?;
            if thread.status() == mlua::ThreadStatus::Resumable {
                match thread.resume::<Value>(()) {
                    Ok(Value::Number(wait_seconds)) => {
                        // Re-yielded with a wait time
                        self.waiting.push(WaitingThread {
                            registry_key: deferred_info.registry_key,
                            resume_at: elapsed + wait_seconds,
                            owner_entity: deferred_info.owner_entity,
                            thread_id: self.next_thread_id,
                        });
                        self.next_thread_id += 1;
                        continue; // Keep registry key alive
                    }
                    Ok(Value::Integer(wait_seconds)) => {
                        self.waiting.push(WaitingThread {
                            registry_key: deferred_info.registry_key,
                            resume_at: elapsed + wait_seconds as f64,
                            owner_entity: deferred_info.owner_entity,
                            thread_id: self.next_thread_id,
                        });
                        self.next_thread_id += 1;
                        continue;
                    }
                    Ok(_) => {} // Coroutine finished
                    Err(e) => log::warn!("[task] Deferred coroutine error: {}", e),
                }
            }
            lua.remove_registry_value(deferred_info.registry_key)?;
        }

        // 2. Process waiting threads whose time has come
        let (ready, still_waiting): (Vec<_>, Vec<_>) = std::mem::take(&mut self.waiting)
            .into_iter()
            .partition(|w| elapsed >= w.resume_at);

        self.waiting = still_waiting;

        for thread_info in ready {
            let thread: Thread = lua.registry_value(&thread_info.registry_key)?;
            if thread.status() == mlua::ThreadStatus::Resumable {
                match thread.resume::<Value>(()) {
                    Ok(Value::Number(wait_seconds)) => {
                        // Re-yielded with another wait
                        self.waiting.push(WaitingThread {
                            registry_key: thread_info.registry_key,
                            resume_at: elapsed + wait_seconds,
                            owner_entity: thread_info.owner_entity,
                            thread_id: thread_info.thread_id,
                        });
                        continue; // Keep registry key alive
                    }
                    Ok(Value::Integer(wait_seconds)) => {
                        self.waiting.push(WaitingThread {
                            registry_key: thread_info.registry_key,
                            resume_at: elapsed + wait_seconds as f64,
                            owner_entity: thread_info.owner_entity,
                            thread_id: thread_info.thread_id,
                        });
                        continue;
                    }
                    Ok(_) => {} // Coroutine finished
                    Err(e) => log::warn!("[task] Coroutine error: {}", e),
                }
            }
            lua.remove_registry_value(thread_info.registry_key)?;
        }

        Ok(())
    }

    /// Spawn a new coroutine from a Lua function with a pre-allocated thread ID.
    ///
    /// Creates a coroutine, immediately resumes it. If it yields with a wait time,
    /// registers it in the scheduler.
    pub fn spawn(
        &mut self,
        lua: &Lua,
        func: Function,
        owner_entity: Option<u64>,
        elapsed: f64,
        thread_id: Option<u64>,
    ) -> LuaResult<u64> {
        let thread_id = thread_id.unwrap_or_else(|| {
            let id = self.next_thread_id;
            self.next_thread_id += 1;
            id
        });

        let thread = lua.create_thread(func)?;

        // Immediately resume — the function runs until first yield or completion
        match thread.resume::<Value>(()) {
            Ok(Value::Number(wait_seconds)) => {
                // Yielded with wait time → register
                let key = lua.create_registry_value(thread)?;
                self.waiting.push(WaitingThread {
                    registry_key: key,
                    resume_at: elapsed + wait_seconds,
                    owner_entity,
                    thread_id,
                });
            }
            Ok(Value::Integer(wait_seconds)) => {
                let key = lua.create_registry_value(thread)?;
                self.waiting.push(WaitingThread {
                    registry_key: key,
                    resume_at: elapsed + wait_seconds as f64,
                    owner_entity,
                    thread_id,
                });
            }
            Ok(_) => {
                // Completed immediately — nothing to schedule
            }
            Err(e) => {
                log::warn!("[task.spawn] Coroutine error: {}", e);
            }
        }

        Ok(thread_id)
    }

    /// Schedule a function to run after a delay (creates coroutine internally).
    pub fn delay(
        &mut self,
        lua: &Lua,
        seconds: f64,
        func: Function,
        owner_entity: Option<u64>,
        elapsed: f64,
        thread_id: Option<u64>,
    ) -> LuaResult<u64> {
        let thread_id = thread_id.unwrap_or_else(|| {
            let id = self.next_thread_id;
            self.next_thread_id += 1;
            id
        });

        let thread = lua.create_thread(func)?;
        let key = lua.create_registry_value(thread)?;

        self.waiting.push(WaitingThread {
            registry_key: key,
            resume_at: elapsed + seconds,
            owner_entity,
            thread_id,
        });

        Ok(thread_id)
    }

    /// Defer a function to run next frame.
    pub fn defer(
        &mut self,
        lua: &Lua,
        func: Function,
        owner_entity: Option<u64>,
    ) -> LuaResult<()> {
        let thread = lua.create_thread(func)?;
        let key = lua.create_registry_value(thread)?;

        self.deferred.push(DeferredThread {
            registry_key: key,
            owner_entity,
        });

        Ok(())
    }

    /// Cancel a thread by ID.
    pub fn cancel(&mut self, lua: &Lua, thread_id: u64) -> LuaResult<bool> {
        if let Some(pos) = self.waiting.iter().position(|w| w.thread_id == thread_id) {
            let removed = self.waiting.remove(pos);
            lua.remove_registry_value(removed.registry_key)?;
            return Ok(true);
        }
        Ok(false)
    }

    /// Cancel all threads owned by an entity (called on entity destroy).
    pub fn cancel_entity(&mut self, lua: &Lua, entity_bits: u64) -> LuaResult<()> {
        let (to_cancel, keep): (Vec<_>, Vec<_>) = std::mem::take(&mut self.waiting)
            .into_iter()
            .partition(|w| w.owner_entity == Some(entity_bits));

        self.waiting = keep;

        for thread_info in to_cancel {
            lua.remove_registry_value(thread_info.registry_key)?;
        }

        // Also cancel deferred
        let (to_cancel_deferred, keep_deferred): (Vec<_>, Vec<_>) =
            std::mem::take(&mut self.deferred)
                .into_iter()
                .partition(|d| d.owner_entity == Some(entity_bits));

        self.deferred = keep_deferred;

        for deferred_info in to_cancel_deferred {
            lua.remove_registry_value(deferred_info.registry_key)?;
        }

        Ok(())
    }

    /// Get the number of active waiting threads (for debugging).
    pub fn active_count(&self) -> usize {
        self.waiting.len() + self.deferred.len()
    }
}
