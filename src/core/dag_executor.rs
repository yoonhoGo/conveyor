use anyhow::Result;
use petgraph::algo::toposort;
use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{error, info, warn};

use crate::core::error::ConveyorError;
use crate::core::stage::StageRef;
use crate::core::strategy::ErrorStrategy;
use crate::core::traits::DataFormat;

/// Node in the DAG representing a pipeline stage
#[derive(Clone)]
pub struct StageNode {
    pub id: String,
    pub stage: StageRef,
    pub config: HashMap<String, toml::Value>,
}

/// DAG-based pipeline executor
pub struct DagExecutor {
    graph: DiGraph<StageNode, ()>,
    node_map: HashMap<String, NodeIndex>,
    error_strategy: ErrorStrategy,
}

impl DagExecutor {
    pub fn new(error_strategy: ErrorStrategy) -> Self {
        Self {
            graph: DiGraph::new(),
            node_map: HashMap::new(),
            error_strategy,
        }
    }

    /// Add a stage to the DAG
    pub fn add_stage(
        &mut self,
        id: String,
        stage: StageRef,
        config: HashMap<String, toml::Value>,
    ) -> Result<()> {
        if self.node_map.contains_key(&id) {
            return Err(
                ConveyorError::PipelineError(format!("Stage '{}' already exists", id)).into(),
            );
        }

        let node = StageNode {
            id: id.clone(),
            stage,
            config,
        };
        let node_index = self.graph.add_node(node);
        self.node_map.insert(id, node_index);

        Ok(())
    }

    /// Add a dependency edge from `from_id` to `to_id`
    /// (to_id depends on from_id)
    pub fn add_dependency(&mut self, from_id: &str, to_id: &str) -> Result<()> {
        let from_index = self.node_map.get(from_id).ok_or_else(|| {
            ConveyorError::PipelineError(format!("Stage '{}' not found", from_id))
        })?;

        let to_index = self
            .node_map
            .get(to_id)
            .ok_or_else(|| ConveyorError::PipelineError(format!("Stage '{}' not found", to_id)))?;

        self.graph.add_edge(*from_index, *to_index, ());

        Ok(())
    }

    /// Validate the DAG (check for cycles)
    pub fn validate(&self) -> Result<()> {
        // Try topological sort to detect cycles
        match toposort(&self.graph, None) {
            Ok(_) => Ok(()),
            Err(cycle) => Err(ConveyorError::PipelineError(format!(
                "Cycle detected in pipeline at stage '{}'",
                self.graph[cycle.node_id()].id
            ))
            .into()),
        }
    }

    /// Execute the DAG pipeline
    pub async fn execute(&self) -> Result<()> {
        // Get topologically sorted order
        let sorted = toposort(&self.graph, None).map_err(|_| {
            ConveyorError::PipelineError("Cannot execute pipeline with cycles".to_string())
        })?;

        // Calculate execution levels for parallel execution
        let levels = self.calculate_execution_levels(&sorted);

        info!("Executing pipeline with {} levels", levels.len());

        // Store outputs from each stage
        let mut outputs: HashMap<String, DataFormat> = HashMap::new();

        // Execute each level
        for (level_idx, level) in levels.iter().enumerate() {
            info!(
                "Executing level {} with {} stage(s)",
                level_idx,
                level.len()
            );

            // Execute all stages in this level in parallel
            let mut tasks = Vec::new();

            for &node_index in level {
                let node = &self.graph[node_index];
                let stage = Arc::clone(&node.stage);
                let config = node.config.clone();
                let id = node.id.clone();

                // Collect inputs from predecessor stages
                let mut inputs = HashMap::new();
                let predecessors = self
                    .graph
                    .neighbors_directed(node_index, petgraph::Direction::Incoming);

                for pred_idx in predecessors {
                    let pred_id = &self.graph[pred_idx].id;
                    info!("Stage '{}': looking for input from '{}'", id, pred_id);
                    if let Some(data) = outputs.get(pred_id) {
                        let cloned_data = data.try_clone().map_err(|e| {
                            anyhow::anyhow!(
                                "Cannot clone input from '{}' for stage '{}': {}. Streaming data can only be consumed once.",
                                pred_id, id, e
                            )
                        })?;
                        inputs.insert(pred_id.clone(), cloned_data);
                        info!("Stage '{}': found input from '{}'", id, pred_id);
                    } else {
                        warn!(
                            "Stage '{}': input from '{}' not found in outputs",
                            id, pred_id
                        );
                    }
                }

                info!("Stage '{}': has {} input(s)", id, inputs.len());

                let error_strategy = self.error_strategy.clone();
                let inputs_clone = inputs;
                let config_clone = config.clone();
                let stage_clone = Arc::clone(&stage);
                let id_clone = id.clone();

                // Spawn task for this stage
                let task = tokio::spawn(async move {
                    info!("Executing stage '{}'", id_clone);
                    let result = stage_clone.execute(inputs_clone, &config_clone).await;

                    let result = match result {
                        Ok(data) => {
                            info!("Stage '{}' completed successfully", id_clone);
                            Ok(data)
                        }
                        Err(e) => Err(e),
                    };

                    let final_result = match result {
                        Ok(data) => Ok((id, data)),
                        Err(e) => {
                            if error_strategy.should_continue_on_error() {
                                warn!("Stage '{}' failed: {}. Continuing...", id, e);
                                Ok((
                                    id,
                                    DataFormat::DataFrame(polars::prelude::DataFrame::empty()),
                                ))
                            } else {
                                Err(e)
                            }
                        }
                    };

                    final_result
                });

                tasks.push(task);
            }

            // Wait for all tasks in this level to complete
            let results = futures::future::join_all(tasks).await;

            // Collect outputs
            for result in results {
                match result {
                    Ok(Ok((stage_id, data))) => {
                        outputs.insert(stage_id, data);
                    }
                    Ok(Err(e)) => {
                        error!("Stage execution failed: {}", e);
                        return Err(e);
                    }
                    Err(e) => {
                        error!("Task join failed: {}", e);
                        return Err(ConveyorError::PipelineError(format!(
                            "Task execution failed: {}",
                            e
                        ))
                        .into());
                    }
                }
            }
        }

        info!("Pipeline execution completed successfully");
        Ok(())
    }

    /// Calculate execution levels for parallel execution
    /// Stages in the same level can be executed in parallel
    fn calculate_execution_levels(&self, sorted: &[NodeIndex]) -> Vec<Vec<NodeIndex>> {
        let mut levels: Vec<Vec<NodeIndex>> = Vec::new();
        let mut level_map: HashMap<NodeIndex, usize> = HashMap::new();

        for &node_index in sorted {
            // Find the maximum level of all predecessors
            let predecessors = self
                .graph
                .neighbors_directed(node_index, petgraph::Direction::Incoming);

            let max_pred_level = predecessors
                .filter_map(|pred_idx| level_map.get(&pred_idx))
                .max();

            let current_level = match max_pred_level {
                Some(&level) => level + 1, // Predecessor exists, place at level + 1
                None => 0,                 // No predecessor, place at level 0
            };

            // Ensure we have enough levels
            while levels.len() <= current_level {
                levels.push(Vec::new());
            }

            levels[current_level].push(node_index);
            level_map.insert(node_index, current_level);
        }

        levels
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::stage::{SourceStageAdapter, Stage};
    use crate::core::traits::DataSource;
    use async_trait::async_trait;
    use polars::prelude::DataFrame;

    struct MockSource;

    #[async_trait]
    impl DataSource for MockSource {
        async fn name(&self) -> &str {
            "mock"
        }

        async fn read(&self, _config: &HashMap<String, toml::Value>) -> Result<DataFormat> {
            Ok(DataFormat::DataFrame(DataFrame::empty()))
        }

        async fn validate_config(&self, _config: &HashMap<String, toml::Value>) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_dag_executor_basic() {
        let mut executor = DagExecutor::new(ErrorStrategy::Stop);

        let stage1 = Arc::new(SourceStageAdapter::new(
            "source1".to_string(),
            Arc::new(MockSource),
        ));

        executor
            .add_stage("stage1".to_string(), stage1, HashMap::new())
            .unwrap();

        assert!(executor.validate().is_ok());
        assert!(executor.execute().await.is_ok());
    }

    #[tokio::test]
    async fn test_dag_executor_cycle_detection() {
        let mut executor = DagExecutor::new(ErrorStrategy::Stop);

        let stage1 = Arc::new(SourceStageAdapter::new(
            "source1".to_string(),
            Arc::new(MockSource),
        ));
        let stage2 = Arc::new(SourceStageAdapter::new(
            "source2".to_string(),
            Arc::new(MockSource),
        ));

        executor
            .add_stage("stage1".to_string(), stage1, HashMap::new())
            .unwrap();
        executor
            .add_stage("stage2".to_string(), stage2, HashMap::new())
            .unwrap();

        // Create a cycle
        executor.add_dependency("stage1", "stage2").unwrap();
        executor.add_dependency("stage2", "stage1").unwrap();

        assert!(executor.validate().is_err());
    }
}
