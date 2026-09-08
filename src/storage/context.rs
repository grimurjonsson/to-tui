use anyhow::{Context, Result};

use std::cell::RefCell;
use std::future::Future;
use std::path::PathBuf;

tokio::task_local! {
    static REQUEST_ROOT: PathBuf;
}

thread_local! {
    static BLOCKING_ROOT: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

pub(crate) fn data_root() -> Option<PathBuf> {
    REQUEST_ROOT
        .try_with(Clone::clone)
        .ok()
        .or_else(|| BLOCKING_ROOT.with(|root| root.borrow().clone()))
}

pub(crate) async fn scope<F: Future>(root: PathBuf, future: F) -> F::Output {
    REQUEST_ROOT.scope(root, future).await
}

pub(crate) fn with_root<T>(root: PathBuf, operation: impl FnOnce() -> T) -> T {
    struct Restore(Option<PathBuf>);
    impl Drop for Restore {
        fn drop(&mut self) {
            BLOCKING_ROOT.with(|root| *root.borrow_mut() = self.0.take());
        }
    }
    let _restore = Restore(BLOCKING_ROOT.with(|slot| slot.replace(Some(root))));
    operation()
}

pub(crate) async fn blocking<T: Send + 'static>(
    operation: impl FnOnce() -> T + Send + 'static,
) -> Result<T> {
    let root = crate::utils::paths::get_to_tui_dir()?;
    tokio::task::spawn_blocking(move || with_root(root, operation))
        .await
        .context("Storage worker failed")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_concurrent_scopes_propagate_to_blocking_workers() {
        let jobs = (0..40).map(|index| {
            tokio::spawn(async move {
                let root = PathBuf::from(format!("/tenant-{index}"));
                scope(root.clone(), async {
                    tokio::task::yield_now().await;
                    assert_eq!(data_root(), Some(root.clone()));
                    assert_eq!(blocking(data_root).await.unwrap(), Some(root));
                })
                .await;
                assert_eq!(data_root(), None);
            })
        });
        for job in jobs.collect::<Vec<_>>() {
            job.await.unwrap();
        }
    }

    #[test]
    fn test_blocking_scope_restores_after_panic() {
        with_root(PathBuf::from("/outer"), || {
            let result =
                std::panic::catch_unwind(|| with_root(PathBuf::from("/inner"), || panic!("test")));
            assert!(result.is_err());
            assert_eq!(data_root(), Some(PathBuf::from("/outer")));
        });
        assert_eq!(data_root(), None);
    }
}
