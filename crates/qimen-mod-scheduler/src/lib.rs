use async_trait::async_trait;
use cron::Schedule;
use qimen_error::Result;
use qimen_plugin_api::Module;
use std::future::Future;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct ScheduledTask {
    pub id: String,
    pub name: String,
    pub cron_expr: String,
    pub enabled: bool,
}

type TaskHandler = Box<dyn Fn() -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

pub struct Scheduler {
    tasks: Vec<(ScheduledTask, TaskHandler)>,
    handles: Vec<tokio::task::JoinHandle<()>>,
}

impl Scheduler {
    pub fn new() -> Self {
        Self {
            tasks: Vec::new(),
            handles: Vec::new(),
        }
    }

    pub fn add_task<F>(&mut self, task: ScheduledTask, handler: F)
    where
        F: Fn() -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync + 'static,
    {
        self.tasks.push((task, Box::new(handler)));
    }

    pub async fn start(&mut self) {
        let tasks = std::mem::take(&mut self.tasks);

        for (task, handler) in tasks {
            if !task.enabled {
                tracing::info!(task_id = %task.id, task_name = %task.name, "skipping disabled task");
                continue;
            }

            let schedule = match Schedule::from_str(&task.cron_expr) {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!(
                        task_id = %task.id,
                        task_name = %task.name,
                        cron_expr = %task.cron_expr,
                        error = %e,
                        "invalid cron expression, skipping task"
                    );
                    continue;
                }
            };

            let task_id = task.id.clone();
            let task_name = task.name.clone();
            let handler = Arc::new(handler);

            let handle = tokio::spawn(async move {
                tracing::info!(task_id = %task_id, task_name = %task_name, "scheduled task started");

                loop {
                    let now = chrono::Utc::now();
                    let next = match schedule.upcoming(chrono::Utc).next() {
                        Some(next) => next,
                        None => {
                            tracing::warn!(
                                task_id = %task_id,
                                "no upcoming schedule found, stopping task"
                            );
                            break;
                        }
                    };

                    let duration = (next - now).to_std().unwrap_or_default();
                    tokio::time::sleep(duration).await;

                    tracing::debug!(task_id = %task_id, task_name = %task_name, "executing scheduled task");
                    handler().await;
                }
            });

            self.handles.push(handle);
        }
    }

    pub async fn stop(&mut self) {
        for handle in self.handles.drain(..) {
            handle.abort();
        }
        tracing::info!("all scheduled tasks stopped");
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

pub struct SchedulerModule {
    scheduler: Arc<Mutex<Scheduler>>,
}

impl Default for SchedulerModule {
    fn default() -> Self {
        Self {
            scheduler: Arc::new(Mutex::new(Scheduler::new())),
        }
    }
}

impl SchedulerModule {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn scheduler(&self) -> Arc<Mutex<Scheduler>> {
        Arc::clone(&self.scheduler)
    }
}

#[async_trait]
impl Module for SchedulerModule {
    fn id(&self) -> &'static str {
        "scheduler"
    }

    async fn on_load(&self) -> Result<()> {
        tracing::info!("scheduler module loaded");
        Ok(())
    }
}
