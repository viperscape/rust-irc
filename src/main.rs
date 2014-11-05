use std::io::TcpStream;
use std::io::BufferedStream;


#[deriving(Show)]
struct ChatMsg {
    chan: String, //originating irc chan (may be own nick if private message)
    msg: String,
    nick: String, //nick originating message
    host: String, //host for nick
}

fn send_stream (s:&mut BufferedStream<TcpStream>,msg: &str) -> std::io::IoResult<()> {
    s.write_str(msg.as_slice()); //we could one line this, but let's break it down
    s.write_u8(b'\n'); //write buffer now consists of string and newline
    s.flush() //flush to underlying stream
}

fn main () {
    let (dbtx,dbrx) = std::comm::channel(); //debug channel, unhandled irc msg
    let (mtx,mrx) = std::comm::channel(); //all chat, for another task to consume

    let mut stream = 
        match TcpStream::connect("irc.freenode.com", 6667)  {
            Err(e) => {
                println!("error connecting: {}",e);
                return //kill main
            },
            Ok(s) => s
        };

    //let's work with buffered streams for convenience
    let mut bufstream = BufferedStream::new(stream.clone()); //buffered stream for use by main task
    let mut bufstream2 = BufferedStream::new(stream.clone()); //for use by handler task

    spawn(proc() { //read stream, run handler

        'handler: loop { 
            let msg = match bufstream2.read_until(b'\n') { //read until newline byte
                Ok(v) => v,
                Err(e) => {
                    println!("error reading stream: {}",e);
                    dbtx.send(e.to_string()); //send the debug chan the io error
                    break 'handler; //exit handler, and let task end
                }
            };

            //msg is currently a byte vector, let's convert to utf8 string
            let msg = match String::from_utf8(msg) { //decode as utf8 (assumes utf8)
                Ok(v) => v, //v is properly decoded buffer
                Err(e) => { //e is the original buffer before attempting to decode
                    println!("error, not utf8!");
                    dbtx.send("error: not utf8".to_string()); //send debug chan that byte stream is not utf8, optionally you could try and decode with a different character set for the error result (e), which is the original buffer
                    break 'handler;
                }
            };

            //slice and dice the message so we can pick out what we need
            let vmsg: Vec<&str> = msg.as_slice().split(' ').collect();
            let cmsg: Vec<&str> = msg.as_slice().split(':').collect();

            match vmsg[0] { //I wonder what irc would look like if rewritten today
                "PING" => {
                    let s = "PONG ".to_string() + vmsg[1]; //pong back the message, keeps connection alive
                    send_stream(&mut bufstream2, s.as_slice());
                    println!("ping-pong: {}",vmsg[1])
                },
                _ => match vmsg[1] {
                    "PRIVMSG" => match cmsg[2] {
                        "quit\r\n" => break 'handler, //this also breaks the main task due to chat chan going out of scope
                        _ => { //otherwise continue using chat chan to communicate with main task
                            let nick_host: Vec<&str> = cmsg[1].as_slice().split('!').collect(); //get nick!host
                            let host: Vec<&str> = nick_host[1].as_slice().split(' ').collect(); //split out host
                            mtx.send(ChatMsg{chan:vmsg[2].to_string(),
                                             msg:cmsg[2].to_string(),
                                             nick:nick_host[0].to_string(),
                                             host:host[0].to_string()})
                        }
                    },
                    "NOTICE" => println!("notice: {}",cmsg[2]),
                    "JOIN" => println!("joining: {}",vmsg[2]),
                    "353" => println!("chan/users: {}/{}",vmsg[4],cmsg[2]),
                    _ => dbtx.send(msg.to_string()) //glom all other commands/text to a debug chan
                }
            }
        }

        drop(bufstream2);
    });

    send_stream (&mut bufstream,"NICK rust-test-bot");
    send_stream (&mut bufstream,"USER rust-test-bot localhost some-server :no one special");
    send_stream (&mut bufstream,"JOIN #greathonu");

    'chat: loop { //todo: consider regex matching for key terms
        let chat = mrx.recv(); //receive what the handler task sends us, blocks until it does
        
        println!("privmsg: {}",chat);
    }

    drop(bufstream);
    drop(stream);
}
