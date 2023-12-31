#![feature(slice_pattern)]

extern crate core;

pub mod io_service;
pub mod event_sink;
pub mod message_receiver;
pub mod message_sender;
pub mod handle_event;
pub mod endpoint;
pub mod node;

mod event;
mod event_sink_impl;
mod framed_codec;
mod framed_header;
mod net_handler;
mod node_context;
mod message_receiver_channel;
mod message_channel;

mod event_channel;

pub mod notifier;
pub mod message_incoming;
pub mod message_incoming_dummy;

pub mod opt_send;
mod opt_ep;
pub mod client;
pub mod message_receiver_endpoint;
mod message_sender_endpoint;
mod parse_dtm_message;
pub mod task;


