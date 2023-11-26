use tokio::sync::mpsc;

pub type MessageChReceiver<T> = mpsc::UnboundedReceiver<T>;
pub type MessageChSender<T> = mpsc::UnboundedSender<T>;

