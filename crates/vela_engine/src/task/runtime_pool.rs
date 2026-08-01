use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use vela_bytecode::LinkedArtifact;

use crate::engine::Engine;
use crate::runtime::{Runtime, RuntimeBuildError};

#[derive(Clone)]
pub(crate) struct DetachedRuntimePool {
    inner: Arc<DetachedRuntimePoolInner>,
}

struct DetachedRuntimePoolInner {
    maximum_idle: usize,
    idle: parking_lot::Mutex<Vec<CachedRuntime>>,
    metrics: RuntimePoolMetrics,
}

struct CachedRuntime {
    engine: Engine,
    artifact: Arc<LinkedArtifact>,
    runtime: Runtime,
}

#[derive(Default)]
struct RuntimePoolMetrics {
    hits: AtomicU64,
    misses: AtomicU64,
    returns: AtomicU64,
    discards: AtomicU64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct RuntimePoolMetricsSnapshot {
    pub(super) hits: u64,
    pub(super) misses: u64,
    pub(super) returns: u64,
    pub(super) discards: u64,
}

impl DetachedRuntimePool {
    pub(super) fn new(maximum_idle: usize) -> Self {
        Self {
            inner: Arc::new(DetachedRuntimePoolInner {
                maximum_idle,
                idle: parking_lot::Mutex::new(Vec::new()),
                metrics: RuntimePoolMetrics::default(),
            }),
        }
    }

    pub(super) fn lease(
        &self,
        engine: &Engine,
        artifact: &Arc<LinkedArtifact>,
    ) -> Result<DetachedRuntimeLease, RuntimeBuildError> {
        let cached = {
            let mut idle = self.inner.idle.lock();
            idle.iter()
                .position(|cached| {
                    cached.engine.same_deployment(engine) && Arc::ptr_eq(&cached.artifact, artifact)
                })
                .map(|index| idle.swap_remove(index))
        };
        let runtime = match cached {
            Some(mut cached) => {
                increment(&self.inner.metrics.hits);
                if let Err(error) = cached.runtime.initialize_detached_pool_state() {
                    increment(&self.inner.metrics.discards);
                    return Err(error);
                }
                cached.runtime
            }
            None => {
                increment(&self.inner.metrics.misses);
                Runtime::from_linked_artifact(engine.clone(), Arc::clone(artifact))?
            }
        };
        Ok(DetachedRuntimeLease {
            pool: self.clone(),
            engine: engine.clone(),
            artifact: Arc::clone(artifact),
            runtime: Some(runtime),
        })
    }

    pub(super) fn metrics(&self) -> RuntimePoolMetricsSnapshot {
        let metrics = &self.inner.metrics;
        RuntimePoolMetricsSnapshot {
            hits: metrics.hits.load(Ordering::Relaxed),
            misses: metrics.misses.load(Ordering::Relaxed),
            returns: metrics.returns.load(Ordering::Relaxed),
            discards: metrics.discards.load(Ordering::Relaxed),
        }
    }
}

/// One initialized isolated Runtime. Drop clears every mutable owner before
/// returning the uninitialized shell to the bounded scope cache.
pub(crate) struct DetachedRuntimeLease {
    pool: DetachedRuntimePool,
    engine: Engine,
    artifact: Arc<LinkedArtifact>,
    runtime: Option<Runtime>,
}

impl DetachedRuntimeLease {
    pub(crate) fn runtime(&mut self) -> &mut Runtime {
        self.runtime
            .as_mut()
            .expect("detached Runtime lease is active")
    }
}

impl Drop for DetachedRuntimeLease {
    fn drop(&mut self) {
        let Some(mut runtime) = self.runtime.take() else {
            return;
        };
        runtime.clear_detached_pool_state();
        let mut idle = self.pool.inner.idle.lock();
        if idle.len() >= self.pool.inner.maximum_idle {
            increment(&self.pool.inner.metrics.discards);
            return;
        }
        idle.push(CachedRuntime {
            engine: self.engine.clone(),
            artifact: Arc::clone(&self.artifact),
            runtime,
        });
        increment(&self.pool.inner.metrics.returns);
    }
}

fn increment(counter: &AtomicU64) {
    let _ = counter.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
        Some(value.saturating_add(1))
    });
}
