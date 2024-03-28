use scupt_util::error_type::ET;
use scupt_util::message::{decode_message, Message, MsgTrait};
use scupt_util::res::Res;
use scupt_util::serde_json_string::SerdeJsonString;
use tracing::error;

pub fn parse_dtm_message<M: MsgTrait + 'static>(byte: &[u8]) -> Res<Message<M>> {
    let (json, _) = decode_message::<Message<SerdeJsonString>>(byte)?;
    let string = json.payload().to_string();
    let r = serde_json::from_str::<Message<M>>(string.as_str());
    match r {
        Ok(m) => { Ok(m) }
        Err(e) => {
            error!("parse DTM action message error: {}, json:{}", e.to_string(), string);
            Err(ET::SerdeError(e.to_string()))
        }
    }
}
