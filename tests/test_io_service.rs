use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use bincode::{Decode, Encode};
use scupt_util::error_type::ET;
use scupt_util::init_logger::logger_setup;
use scupt_util::message::MsgTrait;
use scupt_util::node_id::NID;
use scupt_util::res::Res;
use serde::{Deserialize, Serialize};
use tokio::runtime::Builder;
use tokio::sync::Barrier;
use tokio::sync::mpsc;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::LocalSet;
use tokio::time;
use tracing::{error, trace};

use scupt_net::event_sink::{ESConnectOpt, ESServeOpt, ESStopOpt, EventSink};
use scupt_net::io_service::IOService;
use scupt_net::message_receiver::MessageReceiver;
use scupt_net::message_sender::MessageSender;
use scupt_net::notifier::Notifier;
use scupt_net::opt_send::OptSend;

#[derive(
Clone,
Hash,
PartialEq,
Eq,
Debug,
Serialize,
Deserialize,
Decode,
Encode
)]
enum TestMsg {
    Id(u32),
    Stop(NID),
}

impl Default for TestMsg {
    fn default() -> Self {
        Self::Id(0)
    }
}
impl MsgTrait for TestMsg {}

#[test]
fn test() {
    logger_setup();
    let r = test_service(1, 1, 0);
    assert!(r.is_ok());
}

fn test_service(
    num_nodes: u32,
    num_message_receiver: u32,
    num_event_sender: u32,
) -> Res<()> {
    let mut id2address = HashMap::new();
    let mut node_ids = HashSet::new();

    let mut services = vec![];
    let mut threads = vec![];
    for i in 0..num_nodes {
        let ip = IpAddr::V4("127.0.0.1".parse().unwrap());
        let addr = SocketAddr::new(ip, 8000 + i as u16 + 1);
        let node_id = (i + 600) as NID;
        node_ids.insert(node_id);
        id2address.insert(node_id, addr);
    }
    for (k, _) in &id2address {
        let name = format!("service_{}", k);
        let s = IOService::<TestMsg>::new(k.clone(), name, num_message_receiver, Notifier::new())?;
        services.push(Arc::new(s));
    }
    let (stop_s, stop_receiver) = mpsc::unbounded_channel::<NID>();

    for (_, s) in services.iter().enumerate() {
        let service = s.clone();
        let mut senders = vec![];
        let default_sink = service.default_event_sink();
        let default_sender = service.default_message_sender();
        senders.push(default_sender);
        {
            let sink = default_sink.clone();
            let node_id = service.node_id();
            let addr = id2address.get(&node_id).unwrap().clone();
            let thd_start_serve = std::thread::spawn(move || {
                let runtime = Builder::new_current_thread().enable_all().build().unwrap();
                let ls = LocalSet::new();
                ls.spawn_local(async move {
                    let r = serve(sink, addr).await;
                    assert!(r.is_ok());
                });
                runtime.block_on(ls);
                trace!("{} serve thread exit", node_id)
            });
            threads.push(thd_start_serve);
        }

        {
            let r = Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            let runtime = Arc::new(r);
            let s = service.clone();
            let node_id = service.node_id();
            let thd_run = std::thread::spawn(move || {
                s.run(None, runtime);
                trace!("{} run thread exit", node_id)
            });
            threads.push(thd_run);
        }

        {
            let receivers = service.message_receivers();
            for (i, recv) in receivers.iter().enumerate() {
                let r = recv.clone();
                let stop_sender = stop_s.clone();
                let node_id = service.node_id();
                let thd_recv = std::thread::spawn(move || {
                    let runtime = Builder::new_current_thread().enable_all().build().unwrap();
                    let ls = LocalSet::new();
                    ls.spawn_local(async move {
                        let r = receive(node_id, r, stop_sender).await;
                        assert!(r.is_ok());
                    });
                    runtime.block_on(ls);
                    trace!("{} {} receive thread exit", node_id, i);
                });
                threads.push(thd_recv);
            }
        }
        {
            for i in 0..num_event_sender {
                let sender = service.new_message_sender(i.to_string())?;
                senders.push(sender);
            }

            let barrier = Arc::new(Barrier::new(senders.len()));
            for (i, s) in senders.iter().enumerate() {
                let sender = s.clone();
                let sink = default_sink.clone();
                let addrs = id2address.clone();
                let b = barrier.clone();
                let node_id = service.node_id();
                let thd = std::thread::spawn(move || {
                    let runtime = Builder::new_current_thread().enable_all().build().unwrap();
                    let ls = LocalSet::new();
                    ls.spawn_local(async move {
                        let r = connect(node_id.clone(), sink.clone(), addrs.clone()).await;
                        trace!("before wait, connect done {}, {:?}", node_id, r);
                        let _ = b.wait().await;
                        trace!("after wait, connect done {}", node_id);
                        assert!(r.is_ok());
                        let r = send(node_id.clone(), 10, sender.clone(), addrs.clone()).await;
                        assert!(r.is_ok());
                    });
                    runtime.block_on(ls);
                    trace!("{} {} sender thread exit", node_id, i);
                });
                threads.push(thd);
            }
        }
    }
    {
        let ids = node_ids.clone();
        let service_vec = services.clone();
        let stop_thd = std::thread::spawn(|| {
            let runtime = Builder::new_current_thread().enable_all().build().unwrap();
            let ls = LocalSet::new();
            ls.spawn_local(async move {
                let mut node_id_set = ids;
                let mut receiver = stop_receiver;
                while !node_id_set.is_empty() {
                    let opt_r = receiver.recv().await;
                    match opt_r {
                        Some(node_id) => {
                            trace!("receive stop {}", node_id);
                            node_id_set.remove(&node_id);
                        }
                        None => {}
                    }
                }
                trace!("stop all service");
                for (_, s) in service_vec.iter().enumerate() {
                    let r = s.default_event_sink().stop(ESStopOpt::default()).await;
                    trace!("stop {}", s.node_id());
                    assert!(r.is_ok());
                }
                trace!("stop thread exit");
            });
            runtime.block_on(ls);
        });
        threads.push(stop_thd);
    }
    for s in threads {
        s.join().unwrap();
    }
    for _s in services {
        trace!("{}", _s.node_id())
    }
    Ok(())
}

async fn serve(
    sink: Arc<dyn EventSink>,
    addr: SocketAddr,
) -> Res<()> {
    sink.serve(addr, ESServeOpt::default()).await?;
    trace!("serve on address {}" , addr.to_string());
    Ok(())
}

async fn connect(
    _node_id: NID,
    sender: Arc<dyn EventSink>,
    addrs: HashMap<NID, SocketAddr>,
) -> Res<()> {
    for (node_id, addr) in &addrs {
        loop {
            let r = sender.connect(node_id.clone(), addr.clone(), ESConnectOpt::default()).await;
            match r {
                Ok(_) => {
                    break;
                }
                Err(e) => {
                    match e {
                        ET::IOError(_) => {
                            time::sleep(Duration::from_secs(1)).await;
                        }
                        _e => {
                            return Err(_e);
                        }
                    }
                }
            }
        }
    }
    for (node_id, addr) in &addrs {
        let _ = sender.connect(
            node_id.clone(),
            addr.clone(),
            ESConnectOpt::default()).await?;
    }
    Ok(())
}

async fn send(
    service_node_id: NID,
    num_message: u32,
    sender: Arc<dyn MessageSender<TestMsg>>,
    addrs: HashMap<NID, SocketAddr>,
) -> Res<()> {
    for i in 0..num_message {
        let id = i + 1;
        let message = TestMsg::Id(id);
        for (node_id, _) in &addrs {
            sender.send(node_id.clone(), message.clone(), OptSend::default()).await?;
        }
        for (node_id, _) in &addrs {
            sender.send(node_id.clone(), message.clone(), OptSend::default()).await?;
        }
    }

    sender.send(service_node_id.clone(), TestMsg::Stop(service_node_id), OptSend::default()).await?;

    trace!("send done {}", service_node_id);
    Ok(())
}

async fn receive(
    node_id: NID,
    receiver: Arc<dyn MessageReceiver<TestMsg>>,
    stop_sender: UnboundedSender<NID>,
) -> Res<()> {
    loop {
        let result = receiver.receive().await;
        match result {
            Ok(m) => {
                match m {
                    TestMsg::Id(_) => {}
                    TestMsg::Stop(id) => {
                        let r = stop_sender.send(id);
                        match r {
                            Ok(_) => {}
                            Err(e) => {
                                error!("send error, {}", e.to_string());
                            }
                        }
                        break;
                    }
                }
            }
            Err(_) => { break; }
        }
    }
    trace!("receive done {}", node_id);
    Ok(())
}