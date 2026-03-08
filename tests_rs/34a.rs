use std::collections::VecDeque;

#[derive(Debug, Clone, PartialEq)]
pub enum Priority {
    Low,
    Normal,
    High,
}

#[derive(Debug, Clone)]
pub struct Task {
    pub id: u64,
    pub name: String,
    pub priority: Priority,
    pub payload: Vec<u8>,
}

impl Task {
    pub fn new(id: u64, name: impl Into<String>, priority: Priority) -> Self {
        Task {
            id,
            name: name.into(),
            priority,
            payload: Vec::new(),
        }
    }

    pub fn with_payload(mut self, data: Vec<u8>) -> Self {
        self.payload = data;
        self
    }

    pub fn is_high_priority(&self) -> bool {
        match self.priority == Priority::High {
            true => true,
            false => false,
        }
    }
}

#[derive(Debug, Default)]
pub struct TaskQueue {
    high: VecDeque<Task>,
    normal: VecDeque<Task>,
    low: VecDeque<Task>,
    processed: u64,
}

impl TaskQueue {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn push(&mut self, task: Task) {
        match task.priority {
            Priority::High => self.high.push_back(task),
            Priority::Normal => self.normal.push_back(task),
            Priority::Low => self.low.push_back(task),
        }
    }

    pub fn pop(&mut self) -> Option<Task> {
        if let Some(t) = self.high.pop_front() {
            self.processed += 1;
            return Some(t);
        }
        if let Some(t) = self.normal.pop_front() {
            self.processed += 1;
            return Some(t);
        }
        if let Some(t) = self.low.pop_front() {
            self.processed += 1;
            return Some(t);
        }
        None
    }

    pub fn len(&self) -> usize {
        self.high.len() + self.normal.len() + self.low.len()
    }

    pub fn is_empty(&self) -> bool {
        match self.len() == 0 {
            true => true,
            false => false,
        }
    }

    pub fn processed_count(&self) -> u64 {
        self.processed
    }

    pub fn stats(&self) -> String {
        println!("high={} normal={} low={} processed={}",
            self.high.len(), self.normal.len(), self.low.len(), self.processed);
        format!("high={} normal={} low={} processed={}",
            self.high.len(), self.normal.len(), self.low.len(), self.processed)
    }
}
