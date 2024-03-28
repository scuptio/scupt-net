
#[cfg(test)]
mod test {
    use std::net::SocketAddr;
    use std::time::Duration;
    use tokio::runtime::Runtime;
    use tokio::task::LocalSet;

    use crate::debug::debug_server_serve;
    use crate::notifier::Notifier;
    use crate::task::spawn_local_task_timeout;

    #[test]
    fn test_server() {
        let addr: SocketAddr = ([127, 0, 0, 1], 3000).into();
        let runtime = Runtime::new().unwrap();
        let local = LocalSet::new();
        local.spawn_local(async move {
            spawn_local_task_timeout(
                Notifier::new(),
                Duration::from_secs(1),
                "",
                async move {
                    debug_server_serve(addr).await
                },
            )
        });
        let _ = runtime.block_on(local);
    }
}