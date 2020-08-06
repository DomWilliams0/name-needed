use std::env;
use std::io::Write;
use std::sync::mpsc;
use std::thread::{self, JoinHandle};

use log::*;
use serde::Serialize;

use crate::sink::EventSink;
use crate::{Event, Span};
use std::fs::{File, OpenOptions};

#[derive(Default)]
pub struct JsonPipeSink {
    thread: Option<JoinHandle<()>>,
    channel: Option<mpsc::Sender<Message>>,
}

#[derive(Serialize)]
enum Message {
    Stop,
    Event(Event),
    EnterSpan(Span),
    EndSpan,
}

fn make_pipe() -> Result<File, String> {
    let path = {
        let mut p = env::temp_dir();
        p.push("game.events");
        p
    };
    if !path.exists() {
        unix_named_pipe::create(&path, None)
            .map_err(|e| format!("failed to create pipe ({:?}): {}", path, e))?;
    }
    OpenOptions::new()
        .write(true)
        .read(true) // so opening doesnt block on waiting for a reader
        .append(true)
        .open(&path)
        .map_err(|e| format!("failed to open pipe ({:?}): {}", path, e))
}

impl EventSink for JsonPipeSink {
    fn on_attach(&mut self) {
        let (result_tx, result_rx) = mpsc::channel();
        let (tx, rx) = mpsc::channel();

        self.channel = Some(tx);
        self.thread = Some(
            thread::Builder::new()
                .name("struclog-ipc".to_owned())
                .spawn(move || {
                    let mut pipe = match make_pipe() {
                        Ok(pipe) => pipe,
                        Err(e) => {
                            result_tx
                                .send(Some(e))
                                .expect("failed to send error message");
                            return;
                        }
                    };

                    // success
                    result_tx
                        .send(None)
                        .expect("failed to send success message");

                    let mut buffer = Vec::with_capacity(256);
                    loop {
                        match rx.recv() {
                            Err(e) => panic!("failed to recv: {}", e),
                            Ok(Message::Stop) => break,
                            Ok(msg) => serde_json::to_writer(&mut buffer, &msg)
                                .expect("failed to serialize message"),
                        };

                        //                buffer.push(b'\r');
                        buffer.push(b'\n');
                        pipe.write_all(&buffer).expect("failed to write to pipe");
                        buffer.clear();
                    }

                    info!("Stopping event thread cleanly");
                })
                .expect("failed to start ipc thread"),
        );

        match result_rx.recv() {
            Err(e) => panic!("failed to recv result from event thread: {}", e),
            Ok(None) => info!("started event thread successfully"),
            Ok(Some(err)) => panic!("failed to start event thread: {}", err),
        }
    }

    fn on_detach(&mut self) {
        let channel = self.channel.take().expect("not initialized");
        channel
            .send(Message::Stop)
            .expect("failed to send stop message");

        info!("Joining on event thread");
        let handle = self.thread.take().expect("not initialized");
        handle.join().expect("failed to join on event thread");
    }

    fn enter_span(&mut self, s: Span) {
        self.send(Message::EnterSpan(s));
    }

    fn pop_span(&mut self) {
        self.send(Message::EndSpan);
    }

    fn post(&mut self, e: Event) {
        self.send(Message::Event(e));
    }
}

impl JsonPipeSink {
    fn send(&mut self, m: Message) {
        self.channel
            .as_ref()
            .expect("not initialized")
            .send(m)
            .expect("failed to send message to event thread");
    }
}
