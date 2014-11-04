use std::io::TcpStream;

#[deriving(Show)]
struct rxchat {
    chan: String,
    msg: String
}

fn send_stream (stream:&Result<TcpStream,std::io::IoError>,msg: &str) { //redo: borrow/return tcpstream instead?
    let mut s2 = stream.clone(); //is this memory intensive if called often?
    let msg = msg.to_string() + "\n";
    s2.write_str(msg.as_slice());
    s2.flush(); //I find this unnecessary if newline is present, but here it is anyways
}

fn main () {
    let (dbtx,dbrx) = std::comm::channel(); //debug channel, unhandled irc msg
    let (mtx,mrx) = std::comm::channel(); //all chat, for another task to consume

    let mut stream = TcpStream::connect("irc.freenode.com", 6667);
    let mut s2 = stream.clone(); //clone stream to share

    spawn(proc() { //read stream, run handler
        let mut buf = [0u8,..2048]; //build buffer to work with

        'handler: loop { 
            let len = s2.read(buf);
            let len = match len {
                Ok(v) => v,
                Err(e) => {
                    println!("error reading stream: {}",e);
                    dbtx.send(e.to_string());
                    break 'handler;
                }
            };

            let msg = buf.slice(0,len).to_vec(); //truncate data in buffer
            let msg = String::from_utf8(msg); //decode as utf8 (assumes utf8)
            let msg = match msg {
                Ok(v) => v,
                Err(e) => {
                    println!("error, not utf8!");
                    dbtx.send("error: not utf8".to_string());
                    break 'handler;
                }
            };
            
            //slice and dice the message so we can pick out what we need
            let vmsg: Vec<&str> = msg.as_slice().split(' ').collect();
            let cmsg: Vec<&str> = msg.as_slice().split(':').collect();
            match vmsg[0] { //I wonder what irc would look like if rewritten today
                "PING" => {
                    let s = "PONG ".to_string() + vmsg[1];
                    send_stream(&s2, s.as_slice());
                    println!("ping-pong: {}",vmsg[1])
                },
                _ => match vmsg[1] {
                    "PRIVMSG" =>  mtx.send(rxchat{chan:vmsg[2].to_string(),
                                                  msg:cmsg[2].to_string()}),
                    "NOTICE" => println!("notice: {}",cmsg[2]),
                    "JOIN" => println!("joining: {}",vmsg[2]),
                    "353" => println!("chan/users: {}/{}",vmsg[4],cmsg[2]),
                    _ => dbtx.send(msg.to_string()) //glom all other commands/text to a debug chan
                }
            }
        }

        drop(s2);
    });


    send_stream (&stream,"NICK rust-test-bot");
    send_stream (&stream,"USER rust-test-bot localhost some-server :no one special");
    send_stream (&stream,"JOIN #rust-bots");

    'chat: loop {
        let chat = mrx.recv(); //receive what the handler task sends us, blocks until it does
        
        match chat.msg.as_slice() {
            "quit\r\n" => break 'chat, //handler task will panic and shutdown
            _ => println!("privmsg: {}",chat)
        }
    }

    drop(stream); // close the connection
}
