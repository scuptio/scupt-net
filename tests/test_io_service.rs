use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::sync::mpsc::Sender;

use std::time::Duration;

use bincode::{Decode, Encode};
use scupt_util::error_type::ET;
use scupt_util::logger::logger_setup;
use scupt_util::message::{Message, MsgTrait};
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

use scupt_net::es_option::{ESConnectOpt, ESServeOpt, ESStopOpt};
use scupt_net::event_sink_async::EventSinkAsync;
use scupt_net::event_sink_sync::EventSinkSync;
use scupt_net::io_service::{IOService, IOServiceOpt};
use scupt_net::message_receiver_async::ReceiverAsync;
use scupt_net::message_receiver_sync::ReceiverSync;
use scupt_net::message_sender_async::SenderAsync;
use scupt_net::message_sender_sync::SenderSync;
use scupt_net::notifier::Notifier;
use scupt_net::opt_send::OptSend;
use scupt_net::task::spawn_local_task;

#[derive(
Clone,
Hash,
PartialEq,
Eq,
Debug,
Serialize,
Deserialize,
Decode,
Encode,
)]
enum TestMsg {
    Id(u32),
    Stop(NID),
}

impl MsgTrait for TestMsg {}

#[test]
fn test_async() {
    logger_setup("debug");
    let r = test_service_async(1, 1, 0);
    assert!(r.is_ok());
}

#[test]
fn test_sync() {
    logger_setup("debug");
    let r = test_service_sync(1, 1, 0);
    assert!(r.is_ok());
}

fn test_service_async(
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
        let addr = SocketAddr::new(ip, 8100 + i as u16 + 1);
        let node_id = (i + 600) as NID;
        node_ids.insert(node_id);
        id2address.insert(node_id, addr);
    }
    for (k, _) in &id2address {
        let name = format!("service_{}", k);
        let opt = IOServiceOpt {
            num_message_receiver,
            testing: false,
            sync_service: false,
            port_debug: Some(3001),
        };
        let s = IOService::<TestMsg>::new_async_service(k.clone(), name, opt, Notifier::new())?;
        services.push(Arc::new(s));
    }
    let (stop_s, stop_receiver) = mpsc::unbounded_channel::<NID>();

    for (_, s) in services.iter().enumerate() {
        let service = s.clone();
        let mut senders = vec![];
        let default_sink = service.default_sink();
        let default_sender = service.default_sender();
        senders.push(default_sender);
        {
            let sink = default_sink.clone();
            let node_id = service.node_id();
            let addr = id2address.get(&node_id).unwrap().clone();
            let thd_start_serve = std::thread::spawn(move || {
                let runtime = Builder::new_current_thread().enable_all().build().unwrap();
                let ls = LocalSet::new();
                let f = async move {
                    let r = serve_async(sink, addr).await;
                    assert!(r.is_ok());
                };
                ls.spawn_local(async move {
                    spawn_local_task(Notifier::new(), "", f)
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
                s.block_run(None, runtime);
                trace!("{} run thread exit", node_id)
            });
            threads.push(thd_run);
        }

        {
            let receivers = service.receiver();
            for (i, recv) in receivers.iter().enumerate() {
                let r = recv.clone();
                let stop_sender = stop_s.clone();
                let node_id = service.node_id();
                let thd_recv = std::thread::spawn(move || {
                    let runtime = Builder::new_current_thread().enable_all().build().unwrap();
                    let ls = LocalSet::new();
                    let f = async move {
                        let r = receive_async(node_id, r, stop_sender).await;
                        assert!(r.is_ok());
                    };
                    ls.spawn_local(async {
                        let _ = spawn_local_task(Notifier::new(), "", f);
                    });
                    runtime.block_on(ls);
                    trace!("{} {} receive thread exit", node_id, i);
                });
                threads.push(thd_recv);
            }
        }
        {
            for i in 0..num_event_sender {
                let sender = service.new_sender(i.to_string())?;
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
                    let f = async move {
                        let r = connect_async(node_id.clone(), sink.clone(), addrs.clone()).await;
                        trace!("before wait, connect done {}, {:?}", node_id, r);
                        let _ = b.wait().await;
                        trace!("after wait, connect done {}", node_id);
                        assert!(r.is_ok());
                        let r = send_async(node_id.clone(), 10, sender.clone(), addrs.clone()).await;
                        assert!(r.is_ok());
                    };
                    ls.spawn_local(async move {
                        spawn_local_task(
                            Notifier::new(), "", f,
                        )
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
            let f = async move {
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
                    let r = s.default_sink().stop(ESStopOpt::default()).await;
                    trace!("stop {}", s.node_id());
                    assert!(r.is_ok());
                }
                trace!("stop thread exit");
            };

            ls.spawn_local(async move {
                spawn_local_task(Notifier::new(), "wait stop", f)
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

fn test_service_sync(
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
        let addr = SocketAddr::new(ip, 8200 + i as u16 + 1);
        let node_id = (i + 600) as NID;
        node_ids.insert(node_id);
        id2address.insert(node_id, addr);
    }
    for (k, _) in &id2address {
        let name = format!("service_{}", k);
        let opt = IOServiceOpt {
            num_message_receiver,
            testing: false,
            sync_service: true,
            port_debug: None,
        };
        let s = IOService::<TestMsg>::new_sync_service(k.clone(), name, opt, Notifier::new())?;
        services.push(Arc::new(s));
    }
    let (stop_s, stop_receiver) = std::sync::mpsc::channel::<NID>();

    for (_, s) in services.iter().enumerate() {
        let service = s.clone();
        let mut senders = vec![];
        let default_sink = service.default_sink();
        let default_sender = service.default_sender();
        senders.push(default_sender);
        {
            let sink = default_sink.clone();
            let node_id = service.node_id();
            let addr = id2address.get(&node_id).unwrap().clone();
            let thd_start_serve = std::thread::spawn(move || {
                let _r = serve_sync(sink, addr);
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
                s.block_run(None, runtime);
                trace!("{} run thread exit", node_id)
            });
            threads.push(thd_run);
        }

        {
            let receivers = service.receiver();
            for (i, recv) in receivers.iter().enumerate() {
                let r = recv.clone();
                let stop_sender = stop_s.clone();
                let node_id = service.node_id();
                let thd_recv = std::thread::spawn(move || {
                    let _r = receive_sync(node_id, r, stop_sender);
                    trace!("{} {} receive thread exit", node_id, i);
                });
                threads.push(thd_recv);
            }
        }
        {
            for i in 0..num_event_sender {
                let sender = service.new_sender(i.to_string())?;
                senders.push(sender);
            }

            let barrier = Arc::new(std::sync::Barrier::new(senders.len()));
            for (i, s) in senders.iter().enumerate() {
                let sender = s.clone();
                let sink = default_sink.clone();
                let addrs = id2address.clone();
                let b = barrier.clone();
                let node_id = service.node_id();
                let thd = std::thread::spawn(move || {
                    let r = connect_sync(node_id.clone(), sink.clone(), addrs.clone());
                    trace!("before wait, connect done {}, {:?}", node_id, r);
                    let _ = b.wait();
                    trace!("after wait, connect done {}", node_id);
                    assert!(r.is_ok());
                    let r = send_sync(node_id.clone(), 10, sender.clone(), addrs.clone());
                    assert!(r.is_ok());

                    trace!("{} {} sender thread exit", node_id, i);
                });
                threads.push(thd);
            }
        }
    }
    {
        let ids = node_ids.clone();
        let service_vec = services.clone();
        let stop_thd = std::thread::spawn(move || {
            let mut node_id_set = ids;
            let receiver = stop_receiver;
            while !node_id_set.is_empty() {
                let opt_r = receiver.recv();
                match opt_r {
                    Ok(node_id) => {
                        trace!("receive stop {}", node_id);
                        node_id_set.remove(&node_id);
                    }
                    Err(_e) => {}
                }
            }
            trace!("stop all service");
            for (_, s) in service_vec.iter().enumerate() {
                let _r = s.default_sink().stop(ESStopOpt::default());
                trace!("stop {}", s.node_id());
                //assert!(r.is_ok());
            }
            trace!("stop thread exit");
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


async fn serve_async<M: MsgTrait + 'static>(
    sink: Arc<dyn EventSinkAsync<M>>,
    addr: SocketAddr,
) -> Res<()> {
    sink.serve(addr, ESServeOpt::default()).await?;
    trace!("serve on address {}" , addr.to_string());
    Ok(())
}

fn serve_sync<M: MsgTrait + 'static>(
    sink: Arc<dyn EventSinkSync<M>>,
    addr: SocketAddr,
) -> Res<()> {
    sink.serve(addr, ESServeOpt::default())?;
    trace!("serve on address {}" , addr.to_string());
    Ok(())
}

async fn connect_async<M: MsgTrait + 'static>(
    _node_id: NID,
    sender: Arc<dyn EventSinkAsync<M>>,
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

fn connect_sync<M: MsgTrait + 'static>(
    _node_id: NID,
    sender: Arc<dyn EventSinkSync<M>>,
    addrs: HashMap<NID, SocketAddr>,
) -> Res<()> {
    for (node_id, addr) in &addrs {
        loop {
            let r = sender.connect(node_id.clone(), addr.clone(), ESConnectOpt::default());
            match r {
                Ok(_) => {
                    break;
                }
                Err(e) => {
                    match e {
                        ET::IOError(_) => {
                            std::thread::sleep(Duration::from_secs(1));
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
            ESConnectOpt::default())?;
    }
    Ok(())
}

async fn send_async(
    service_node_id: NID,
    num_message: u32,
    sender: Arc<dyn SenderAsync<TestMsg>>,
    addrs: HashMap<NID, SocketAddr>,
) -> Res<()> {
    for i in 0..num_message {
        let id = i + 1;

        for (node_id, _) in &addrs {
            let message = Message::new(TestMsg::Id(id), service_node_id, node_id.clone());
            sender.send(message.clone(), OptSend::default()).await?;
        }
        for (node_id, _) in &addrs {
            let message = Message::new(TestMsg::Id(id), service_node_id, node_id.clone());
            sender.send(message.clone(), OptSend::default()).await?;
        }
    }

    let message = Message::new(TestMsg::Stop(service_node_id), service_node_id, service_node_id);
    sender.send(message, OptSend::default()).await?;

    trace!("send done {}", service_node_id);
    Ok(())
}

fn send_sync(
    service_node_id: NID,
    num_message: u32,
    sender: Arc<dyn SenderSync<TestMsg>>,
    addrs: HashMap<NID, SocketAddr>,
) -> Res<()> {
    for i in 0..num_message {
        let id = i + 1;

        for (node_id, _) in &addrs {
            let message = Message::new(TestMsg::Id(id), service_node_id, node_id.clone());
            sender.send(message.clone(), OptSend::default())?;
        }
        for (node_id, _) in &addrs {
            let message = Message::new(TestMsg::Id(id), service_node_id, node_id.clone());
            sender.send(message.clone(), OptSend::default())?;
        }
    }

    let message = Message::new(TestMsg::Stop(service_node_id), service_node_id, service_node_id);
    sender.send(message, OptSend::default())?;

    trace!("send done {}", service_node_id);
    Ok(())
}

async fn receive_async(
    node_id: NID,
    receiver: Arc<dyn ReceiverAsync<TestMsg>>,
    stop_sender: UnboundedSender<NID>,
) -> Res<()> {
    loop {
        let result = receiver.receive().await;
        match result {
            Ok(m) => {
                match m.payload() {
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

fn receive_sync(
    node_id: NID,
    receiver: Arc<dyn ReceiverSync<TestMsg>>,
    stop_sender: Sender<NID>,
) -> Res<()> {
    loop {
        let result = receiver.receive();
        match result {
            Ok(m) => {
                match m.payload() {
                    TestMsg::Id(id) => {
                        trace!("receive id {}", id);
                    }
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