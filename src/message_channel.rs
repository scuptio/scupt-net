use tokio::sync::mpsc;

pub type MessageChReceiver<MsgTrait> = mpsc::UnboundedReceiver<MsgTrait>;
pub type MessageChSender<MsgTrait> = mpsc::UnboundedSender<MsgTrait>;

