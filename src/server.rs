use std::{
    collections::HashMap,
    io::{Read, Write},
    sync::{Arc, Mutex},
    thread,
};

use mio::{
    net::{TcpListener, TcpStream},
    Events, Interest, Poll, Token, Waker,
};

const SERVER: Token = Token(0);
const NEW_CONNECTION: Token = Token(1);

pub struct Server {
    pub connections: Arc<Mutex<HashMap<usize, (TcpStream, Vec<String>)>>>,
    pub create_connection_waker: Arc<Waker>,
}

impl Server {
    pub fn new() -> Self {
        Server {
            connections: Arc::new(Mutex::new(HashMap::new())),
            create_connection_waker: Arc::new(Waker::new),
        }
    }
}

impl Server {
    pub fn number_of_connections(&self) -> usize {
        let conns = self.connections.lock().unwrap();
        conns.len()
    }

    pub fn send_message(&self, chat_token: usize, message: &str) {
        let mut conns = self.connections.lock().unwrap();
        let (stream, messages) = conns.get_mut(&chat_token).unwrap();
        if stream.write_all(message.as_bytes()).is_ok() {
            messages.push(message.to_owned());
        }
    }

    pub fn get_messages(&self, chat_token: usize) -> Option<Vec<String>> {
        let conns = self.connections.lock().unwrap();
        conns
            .get(&chat_token)
            .map(|(_, messages)| messages.to_owned())
    }

    pub fn listen(&self) {
        let connections = self.connections.clone();
        thread::spawn(move || {
            let mut events = Events::with_capacity(128);
            let addr = "127.0.0.1:13265".parse().unwrap();
            let mut server = TcpListener::bind(addr)
                .unwrap_or_else(|_| panic!("Could not bind TcpListener to address {}", addr));

            let poll = Poll::new().expect("Could not create polling event handler");
            //     // Start listening for incoming connections.
            poll.registry()
                .register(&mut server, SERVER, Interest::READABLE)
                .expect("Could not register TcpListener to event polling");

            let mut socket_index = 1;

            // Start an event loop.
            loop {
                // Poll Mio for events, blocking until we get an event.
                poll.poll(&mut events, None)
                    .expect("Could not poll system for events");

                // Process each event.
                for event in events.iter() {
                    // We can use the token we previously provided to `register` to
                    // determine for which socket the event is.
                    match event.token() {
                        SERVER => {
                            let (mut stream, _) = server
                                .accept()
                                .expect("Could not establish connection with peer");

                            let connection_token = Token(socket_index);

                            // Once we have a successfull stream, we want to deregister the server from being polled as we no longer want to check for new incomming connections
                            poll.registry()
                                .register(&mut stream, connection_token, Interest::READABLE)
                                .unwrap();

                            // Create new connection with its assocated stream and history of messages
                            {
                                let mut conns = connections.lock().unwrap();
                                conns.insert(socket_index, (stream, Vec::new()));
                            }

                            socket_index += 1;
                        }
                        // Read incoming data
                        Token(n) => {
                            let mut conns = connections.lock().unwrap();
                            let (stream, messages) = conns.get_mut(&n).unwrap();

                            if event.is_readable() {
                                let mut buf = [0u8; 512];
                                match stream.read(&mut buf) {
                                    Ok(_) => messages.push(
                                        String::from_utf8(buf.to_vec())
                                            .unwrap()
                                            .trim_end_matches(char::from(0))
                                            .to_string(),
                                    ),
                                    Err(e) => {}
                                }
                            }
                        }
                        // We don't expect any events with tokens other than those we provided.
                        _ => unreachable!(),
                    }
                }
            }
        });
    }
}
