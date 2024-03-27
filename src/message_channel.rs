use tokio::sync::mpsc;

pub type MessageChAsyncReceiver<T> = mpsc::UnboundedReceiver<T>;
pub type MessageChAsyncSender<T> = mpsc::UnboundedSender<T>;

pub type MessageChSyncReceiver<T> = std::sync::mpsc::Receiver<T>;
pub type MessageChSyncSender<T> = std::sync::mpsc::Sender<T>;