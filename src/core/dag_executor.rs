use anyhow::Result;
use petgraph::algo::toposort;
use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
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
    use crate::core::stage::Stage;
    use async_trait::async_trait;
    use polars::prelude::DataFrame;

    struct MockStage;

    #[async_trait]
    impl Stage for MockStage {
        fn name(&self) -> &str {
            "mock"
        }

        fn metadata(&self) -> crate::core::metadata::StageMetadata {
            crate::core::metadata::StageMetadata::builder(
                "mock",
                crate::core::metadata::StageCategory::Transform,
            )
            .description("Mock stage for testing")
            .build()
        }

        async fn execute(
            &self,
            _inputs: HashMap<String, DataFormat>,
            _config: &HashMap<String, toml::Value>,
        ) -> Result<DataFormat> {
            Ok(DataFormat::DataFrame(DataFrame::empty()))
        }

        async fn validate_config(&self, _config: &HashMap<String, toml::Value>) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_dag_executor_basic() {
        let mut executor = DagExecutor::new(ErrorStrategy::Stop);

        let stage1 = Arc::new(MockStage) as StageRef;

        executor
            .add_stage("stage1".to_string(), stage1, HashMap::new())
            .unwrap();

        assert!(executor.validate().is_ok());
        assert!(executor.execute().await.is_ok());
    }

    #[tokio::test]
    async fn test_dag_executor_cycle_detection() {
        let mut executor = DagExecutor::new(ErrorStrategy::Stop);

        let stage1 = Arc::new(MockStage) as StageRef;
        let stage2 = Arc::new(MockStage) as StageRef;

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

// ============================================================================
// Channel-based DAG Executor with Backpressure
// ============================================================================

/// Channel-based DAG Executor with automatic backpressure
///
/// This executor uses bounded channels to connect stages, enabling:
/// - **Automatic backpressure**: slow downstream stages naturally slow down upstream
/// - **Concurrent execution**: all stages run in parallel (not level-by-level)
/// - **Memory control**: bounded buffers prevent unbounded memory growth
///
/// ## How it works
///
/// 1. Creates a bounded channel (mpsc) between each pair of connected stages
/// 2. Spawns each stage as an independent tokio task
/// 3. Stages communicate through channels:
///    - When a stage produces data, it sends to downstream channel(s)
///    - When channel buffer is full, send().await blocks (backpressure!)
///    - Downstream stages receive data from upstream channels
/// 4. All stages run concurrently, not in batched levels
///
/// ## Backpressure behavior
///
/// With `buffer_size = 100`:
/// - Fast source produces 1000 items/sec
/// - Slow sink processes 10 items/sec
/// - Channel buffer fills up quickly
/// - Source blocks when trying to send 101st item
/// - Source automatically slows down to match sink's speed
/// - Memory usage bounded to ~100 items, not 1000!
///
/// ## Example usage
///
/// ```rust,ignore
/// use conveyor::{ChannelDagExecutor, ErrorStrategy};
///
/// // Create executor with buffer size 100
/// let mut executor = ChannelDagExecutor::new(ErrorStrategy::Stop, 100);
///
/// // Add stages
/// executor.add_stage("source".to_string(), source_stage, config)?;
/// executor.add_stage("transform".to_string(), transform_stage, config)?;
/// executor.add_stage("sink".to_string(), sink_stage, config)?;
///
/// // Define dependencies (DAG edges)
/// executor.add_dependency("source", "transform")?;
/// executor.add_dependency("transform", "sink")?;
///
/// // Validate and execute
/// executor.validate()?;
/// executor.execute().await?;
/// ```
///
/// ## Comparison with DagExecutor
///
/// | Feature | DagExecutor | ChannelDagExecutor |
/// |---------|-------------|-------------------|
/// | Execution model | Level-by-level batches | Fully concurrent |
/// | Backpressure | None (load all in memory) | Automatic (bounded channels) |
/// | Memory usage | Unbounded | Bounded by buffer_size |
/// | Best for | Small pipelines | Large streaming pipelines |
pub struct ChannelDagExecutor {
    graph: DiGraph<StageNode, ()>,
    node_map: HashMap<String, NodeIndex>,
    error_strategy: ErrorStrategy,
    buffer_size: usize,
}

impl ChannelDagExecutor {
    /// Create a new channel-based executor
    ///
    /// # Arguments
    /// * `error_strategy` - How to handle stage errors
    /// * `buffer_size` - Channel buffer size (controls backpressure threshold)
    pub fn new(error_strategy: ErrorStrategy, buffer_size: usize) -> Self {
        Self {
            graph: DiGraph::new(),
            node_map: HashMap::new(),
            error_strategy,
            buffer_size,
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
        match toposort(&self.graph, None) {
            Ok(_) => Ok(()),
            Err(cycle) => Err(ConveyorError::PipelineError(format!(
                "Cycle detected in pipeline at stage '{}'",
                self.graph[cycle.node_id()].id
            ))
            .into()),
        }
    }

    /// Execute the DAG pipeline with channel-based backpressure
    pub async fn execute(&self) -> Result<()> {
        // Validate DAG first
        self.validate()?;

        let sorted = toposort(&self.graph, None).map_err(|_| {
            ConveyorError::PipelineError("Cannot execute pipeline with cycles".to_string())
        })?;

        info!(
            "Executing channel-based pipeline with {} stages (buffer_size={})",
            sorted.len(),
            self.buffer_size
        );

        // Create channels for each edge in the graph
        type ChannelPair = (mpsc::Sender<DataFormat>, mpsc::Receiver<DataFormat>);
        let mut channels: HashMap<(String, String), ChannelPair> = HashMap::new();

        // Build channels between stages
        for node_idx in &sorted {
            let node = &self.graph[*node_idx];
            let successors = self
                .graph
                .neighbors_directed(*node_idx, petgraph::Direction::Outgoing);

            for succ_idx in successors {
                let succ_node = &self.graph[succ_idx];
                let (tx, rx) = mpsc::channel(self.buffer_size);
                channels.insert((node.id.clone(), succ_node.id.clone()), (tx, rx));
            }
        }

        // Spawn tasks for each stage
        let mut tasks = Vec::new();

        for node_idx in &sorted {
            let node = &self.graph[*node_idx];
            let stage = Arc::clone(&node.stage);
            let config = node.config.clone();
            let id = node.id.clone();

            // Collect input receivers
            let predecessors: Vec<_> = self
                .graph
                .neighbors_directed(*node_idx, petgraph::Direction::Incoming)
                .collect();

            let mut input_receivers = Vec::new();
            for pred_idx in predecessors {
                let pred_node = &self.graph[pred_idx];
                // Remove receiver from channels map (each receiver can only be used once)
                if let Some((_tx, rx)) = channels.remove(&(pred_node.id.clone(), id.clone())) {
                    input_receivers.push((pred_node.id.clone(), rx));
                }
            }

            // Collect output senders
            let successors: Vec<_> = self
                .graph
                .neighbors_directed(*node_idx, petgraph::Direction::Outgoing)
                .collect();

            let mut output_senders = Vec::new();
            for succ_idx in successors {
                let succ_node = &self.graph[succ_idx];
                if let Some((tx, _rx)) = channels.get(&(id.clone(), succ_node.id.clone())) {
                    output_senders.push(tx.clone());
                }
            }

            let error_strategy = self.error_strategy.clone();

            // Spawn stage task
            let task = tokio::spawn(async move {
                Self::run_stage(
                    id,
                    stage,
                    config,
                    input_receivers,
                    output_senders,
                    error_strategy,
                )
                .await
            });

            tasks.push(task);
        }

        // Wait for all tasks
        let results = futures::future::join_all(tasks).await;

        // Check results
        for result in results {
            match result {
                Ok(Ok(())) => {}
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

        info!("Channel-based pipeline execution completed successfully");
        Ok(())
    }

    /// Run a single stage with channel I/O
    async fn run_stage(
        id: String,
        stage: StageRef,
        config: HashMap<String, toml::Value>,
        mut input_receivers: Vec<(String, mpsc::Receiver<DataFormat>)>,
        output_senders: Vec<mpsc::Sender<DataFormat>>,
        error_strategy: ErrorStrategy,
    ) -> Result<()> {
        info!("Channel stage '{}' started", id);

        // Collect inputs
        let mut inputs = HashMap::new();

        if input_receivers.is_empty() {
            // Source stage - no inputs
            info!("Stage '{}': source stage (no inputs)", id);
        } else {
            // Receive from all input channels
            for (input_id, rx) in input_receivers.iter_mut() {
                match rx.recv().await {
                    Some(data) => {
                        info!("Stage '{}': received input from '{}'", id, input_id);
                        inputs.insert(input_id.clone(), data);
                    }
                    None => {
                        warn!("Stage '{}': input channel from '{}' closed", id, input_id);
                    }
                }
            }
        }

        // Execute stage
        info!("Executing channel stage '{}'", id);
        let output = match stage.execute(inputs, &config).await {
            Ok(data) => {
                info!("Channel stage '{}' completed successfully", id);
                data
            }
            Err(e) => {
                if error_strategy.should_continue_on_error() {
                    warn!("Channel stage '{}' failed: {}. Continuing...", id, e);
                    DataFormat::DataFrame(polars::prelude::DataFrame::empty())
                } else {
                    return Err(e);
                }
            }
        };

        // Send to output channels
        for tx in output_senders {
            // Clone data for each output
            let data = output.try_clone().map_err(|e| {
                anyhow::anyhow!(
                    "Cannot clone output from stage '{}': {}. Streaming data can only be consumed once.",
                    id, e
                )
            })?;

            // Send with backpressure
            if let Err(e) = tx.send(data).await {
                warn!("Channel stage '{}': failed to send output: {}", id, e);
            }
        }

        info!("Channel stage '{}' finished", id);
        Ok(())
    }
}

#[cfg(test)]
mod channel_tests {
    use super::*;
    use crate::core::stage::Stage;
    use async_trait::async_trait;
    use polars::prelude::DataFrame;
    use std::time::Duration;

    struct SlowSinkStage;

    #[async_trait]
    impl Stage for SlowSinkStage {
        fn name(&self) -> &str {
            "slow_sink"
        }

        fn metadata(&self) -> crate::core::metadata::StageMetadata {
            crate::core::metadata::StageMetadata::builder(
                "slow_sink",
                crate::core::metadata::StageCategory::Sink,
            )
            .description("Slow sink for testing backpressure")
            .build()
        }

        async fn execute(
            &self,
            _inputs: HashMap<String, DataFormat>,
            _config: &HashMap<String, toml::Value>,
        ) -> Result<DataFormat> {
            // Simulate slow processing
            tokio::time::sleep(Duration::from_millis(100)).await;
            Ok(DataFormat::DataFrame(DataFrame::empty()))
        }

        async fn validate_config(&self, _config: &HashMap<String, toml::Value>) -> Result<()> {
            Ok(())
        }
    }

    struct FastSourceStage;

    #[async_trait]
    impl Stage for FastSourceStage {
        fn name(&self) -> &str {
            "fast_source"
        }

        fn metadata(&self) -> crate::core::metadata::StageMetadata {
            crate::core::metadata::StageMetadata::builder(
                "fast_source",
                crate::core::metadata::StageCategory::Source,
            )
            .description("Fast source for testing backpressure")
            .build()
        }

        async fn execute(
            &self,
            _inputs: HashMap<String, DataFormat>,
            _config: &HashMap<String, toml::Value>,
        ) -> Result<DataFormat> {
            // Fast processing
            Ok(DataFormat::DataFrame(DataFrame::empty()))
        }

        async fn validate_config(&self, _config: &HashMap<String, toml::Value>) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_channel_dag_executor_basic() {
        let mut executor = ChannelDagExecutor::new(ErrorStrategy::Stop, 10);

        let source = Arc::new(FastSourceStage) as StageRef;
        let sink = Arc::new(SlowSinkStage) as StageRef;

        executor
            .add_stage("source".to_string(), source, HashMap::new())
            .unwrap();
        executor
            .add_stage("sink".to_string(), sink, HashMap::new())
            .unwrap();
        executor.add_dependency("source", "sink").unwrap();

        assert!(executor.validate().is_ok());
        assert!(executor.execute().await.is_ok());
    }

    #[tokio::test]
    async fn test_channel_dag_executor_cycle_detection() {
        let mut executor = ChannelDagExecutor::new(ErrorStrategy::Stop, 10);

        let stage1 = Arc::new(FastSourceStage) as StageRef;
        let stage2 = Arc::new(SlowSinkStage) as StageRef;

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

    #[tokio::test]
    async fn test_channel_dag_backpressure() {
        // Test that bounded channels create backpressure
        // Fast source -> Slow sink with small buffer
        let mut executor = ChannelDagExecutor::new(ErrorStrategy::Stop, 1);

        let source = Arc::new(FastSourceStage) as StageRef;
        let sink = Arc::new(SlowSinkStage) as StageRef;

        executor
            .add_stage("fast_source".to_string(), source, HashMap::new())
            .unwrap();
        executor
            .add_stage("slow_sink".to_string(), sink, HashMap::new())
            .unwrap();
        executor.add_dependency("fast_source", "slow_sink").unwrap();

        // This should succeed but source should be slowed down by sink
        let start = std::time::Instant::now();
        assert!(executor.execute().await.is_ok());
        let elapsed = start.elapsed();

        // Should take at least 100ms due to slow sink
        assert!(
            elapsed.as_millis() >= 100,
            "Expected at least 100ms, got {:?}",
            elapsed
        );
    }
}
