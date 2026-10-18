use crate::id::EntityId;
use crate::model::task::Task;
use std::collections::{HashMap, HashSet};

/// In-memory graph that derives reverse relationships from canonical entity fields.
///
/// Ensures no duplicate canonical storage in YAML:
/// - Canonical: `Task.depends_on` -> Derived: `blocked_by` (which tasks are blocked by a given task)
/// - Canonical: `Task.milestone_id` -> Derived: `milestone_tasks`
/// - Canonical: `Task.parent_id` -> Derived: `subtasks`
#[derive(Debug, Default, Clone)]
pub struct RelationshipGraph {
    /// Maps task_id -> set of tasks that THIS task depends on (forward, canonical)
    dependencies: HashMap<EntityId, HashSet<EntityId>>,

    /// Maps task_id -> set of tasks that are BLOCKED by THIS task (derived reverse)
    dependents: HashMap<EntityId, HashSet<EntityId>>,

    /// Maps milestone_id -> set of task_ids in that milestone (derived reverse)
    milestone_tasks: HashMap<EntityId, HashSet<EntityId>>,

    /// Maps parent_task_id -> set of subtask_ids (derived reverse)
    subtasks: HashMap<EntityId, HashSet<EntityId>>,
}

impl RelationshipGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Rebuilds the graph from an iterator of tasks.
    pub fn build<'a>(tasks: impl IntoIterator<Item = &'a Task>) -> Self {
        let mut graph = Self::new();
        for task in tasks {
            graph.add_task(task);
        }
        graph
    }

    /// Registers a task and indexes its relationships.
    pub fn add_task(&mut self, task: &Task) {
        // Canonical forward dependencies
        let deps = self.dependencies.entry(task.id).or_default();
        for &dep in &task.depends_on {
            deps.insert(dep);
            // Reverse: dep blocks task.id
            self.dependents.entry(dep).or_default().insert(task.id);
        }

        // Milestone relationship
        if let Some(milestone_id) = task.milestone_id {
            self.milestone_tasks
                .entry(milestone_id)
                .or_default()
                .insert(task.id);
        }

        // Parent-child subtask relationship
        if let Some(parent_id) = task.parent_id {
            self.subtasks.entry(parent_id).or_default().insert(task.id);
        }
    }

    /// Returns the task IDs that `task_id` depends on (canonical forward).
    pub fn dependencies_of(&self, task_id: EntityId) -> Vec<EntityId> {
        self.dependencies
            .get(&task_id)
            .map(|set| set.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Derived: Returns the task IDs that are blocked by `task_id` (reverse relationship).
    pub fn blocked_by(&self, task_id: EntityId) -> Vec<EntityId> {
        self.dependents
            .get(&task_id)
            .map(|set| set.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Derived: Returns all tasks assigned to a given milestone.
    pub fn tasks_in_milestone(&self, milestone_id: EntityId) -> Vec<EntityId> {
        self.milestone_tasks
            .get(&milestone_id)
            .map(|set| set.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Derived: Returns all subtasks of a given parent task.
    pub fn subtasks_of(&self, parent_id: EntityId) -> Vec<EntityId> {
        self.subtasks
            .get(&parent_id)
            .map(|set| set.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Detects if adding a dependency from `from` to `to` would create a cycle.
    pub fn would_create_cycle(&self, from: EntityId, to: EntityId) -> bool {
        if from == to {
            return true;
        }
        let mut visited = HashSet::new();
        let mut queue = vec![to];

        while let Some(current) = queue.pop() {
            if current == from {
                return true;
            }
            if visited.insert(current) {
                if let Some(deps) = self.dependencies.get(&current) {
                    for &next_dep in deps {
                        queue.push(next_dep);
                    }
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derived_relationships() {
        let mut task1 = Task::new("Task 1");
        let mut task2 = Task::new("Task 2");
        let milestone_id = EntityId::new();

        // task2 depends on task1 canonically
        task2.depends_on.push(task1.id);
        task2.milestone_id = Some(milestone_id);
        task1.milestone_id = Some(milestone_id);

        let graph = RelationshipGraph::build(&[task1.clone(), task2.clone()]);

        // Forward dependency
        assert_eq!(graph.dependencies_of(task2.id), vec![task1.id]);
        assert!(graph.dependencies_of(task1.id).is_empty());

        // Derived reverse dependency (task1 blocks task2)
        assert_eq!(graph.blocked_by(task1.id), vec![task2.id]);
        assert!(graph.blocked_by(task2.id).is_empty());

        // Derived milestone tasks
        let mut ms_tasks = graph.tasks_in_milestone(milestone_id);
        ms_tasks.sort();
        let mut expected = vec![task1.id, task2.id];
        expected.sort();
        assert_eq!(ms_tasks, expected);

        // Cycle detection: task1 depending on task2 would create a cycle
        assert!(graph.would_create_cycle(task1.id, task2.id));
        // task2 depending on some new task3 would NOT create a cycle
        let task3_id = EntityId::new();
        assert!(!graph.would_create_cycle(task2.id, task3_id));
    }
}
