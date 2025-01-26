mod secure_stream;
mod file_transferer;

use core::str;
use std::{env, fs::File, io::{self, prelude::*, ErrorKind}, net::TcpStream, sync::{mpsc::{self, Receiver, TryRecvError}, Arc, Mutex}, thread, time::{self, Duration, UNIX_EPOCH}};

use secure_stream::SecureStream;

fn main() {
    let mut response_buffer: [u8; 1024] = [0; 1024];

    // connect to the server address provided by either the 'RSPI_SERVER_ADDR' enviorment variable
    // or the first command line argument
    let args: Vec<String> = env::args().collect();
    let mut addr = env::var("RSPI_SERVER_ADDR").unwrap_or(String::from("127.0.0.1:8080"));
    if args.len()>1{
        addr = args[1].clone();
    }

    let stream = TcpStream::connect(addr).unwrap();

    // the first message the server is expecting from us is a password, which we send here.
    let mut stream = SecureStream::new(stream).set_hash(get_hash().unwrap());
    let pass = env::var("RSPI_SERVER_PASS").unwrap_or(String::from("Password"));
    let _ = stream.write(pass.as_bytes());

    // thread safe boolean that defines whether or not we are current in the process of sending or receiving a file
    let is_sending_file = Arc::new(Mutex::new(false));

    stream.set_read_timeout(Some(Duration::new(0,1000000))).unwrap();
    let stdin_channel = spawn_stdin_channel();

    let mut stream_copy = stream.try_clone().unwrap();
    let is_sending_file_copy = is_sending_file.clone();
    let _ = ctrlc::set_handler(move || {
        if let Ok(sending) = is_sending_file_copy.lock(){
            if *sending{
                panic!("File transfer was interrupted");
            }else{let _ = stream_copy.write(b"SIGINT");}
        }else{let _ = stream_copy.write(b"SIGINT");}
    });
    
    loop{
        // println!("locked");
        let input: Option<String> = match stdin_channel.try_recv() {
            Ok(key) => Some(key),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => None,
        };
        match input{
            Some(input_content) => {
                let trimmed_content = input_content.trim();
                if trimmed_content=="close conn"{
                    break;
                }else if trimmed_content.starts_with("rspi getfile"){
                    if let Some(f_dir) = trimmed_content.strip_prefix("rspi getfile"){
                        let f_path = std::path::PathBuf::from(f_dir.trim());
                        let f_name = f_path.file_name().unwrap_or(std::ffi::OsStr::new("new_file"));
                        let current_dir = env::current_dir().unwrap();
                        match File::create(current_dir.join(f_name)){
                            Ok(file) => {
                                *is_sending_file.lock().unwrap() = true;
                                stream.set_read_timeout(Some(Duration::new(2,0))).unwrap();
                                let _ = stream.write(trimmed_content.as_bytes());
                                match file_transferer::recv(&mut stream, file){
                                    Ok(_) => (),
                                    Err(e) => println!("{}",e)
                                };
                                stream.set_read_timeout(Some(Duration::new(0,1000000))).unwrap();
                                *is_sending_file.lock().unwrap() = false;
                            }
                            Err(e) => println!("Could not create file {}: {}",f_name.to_str().unwrap_or("new_file"),e)
                        }
                    }else{
                        println!("Could not parse file name. Usage:\nrspi getfile [file name]")
                    }
                }else if trimmed_content.starts_with("rspi sendfile"){
                    if let Some(f_name) = trimmed_content.strip_prefix("rspi sendfile"){
                        let f_name = f_name.trim();
                        let current_dir = env::current_dir().unwrap();
                        match File::open(&current_dir.join(f_name)){
                            Ok(file) => {
                                *is_sending_file.lock().unwrap() = true;
                                let _ = stream.write(trimmed_content.as_bytes());
                                match file_transferer::send(&mut stream, file){
                                    Ok(_) => (),
                                    Err(e) => println!("{}",e)
                                };
                                *is_sending_file.lock().unwrap() = false;
                            }
                            Err(e) => println!("Could not open file {}: {}",f_name,e)
                        }
                    }else{
                        println!("Could not parse file name. Usage:\nrspi sendfile [file name]")
                    }
                }else{
                    let _ = stream.write(trimmed_content.as_bytes());
                }
            },
            None => ()
        }

        match stream.read(&mut response_buffer){
            Ok(msg_len) => {
                if msg_len==0{println!("Connection Closed"); break}
                else {
                    // println!("{} {} {} {} {}\n{:?}",response_buffer[32],response_buffer[33],response_buffer[34],response_buffer[35],response_buffer[36],response_buffer);
                    let received_msg = str::from_utf8(&response_buffer[0..msg_len]).unwrap();
                    // println!("recieved message! {}",msg_len);
                    print!("{}",received_msg);
                    let _ = io::stdout().flush();
                }
            },
            Err(e) => {
                match e.kind(){
                    ErrorKind::WouldBlock | ErrorKind::TimedOut => (),
                    _ => {
                        println!("Something went wrong:\n{}\nClosing connection...",e);
                        break;
                    }
                }
            },
        }
    }
}

fn spawn_stdin_channel() -> Receiver<String> {
    let (tx, rx) = mpsc::channel::<String>();
    thread::spawn(move || loop {
        let mut buffer = String::new();
        io::stdin().read_line(&mut buffer).unwrap();
        tx.send(buffer).unwrap();
    });
    rx
}

fn rng_32(seed: &mut u64) -> u32{
    let old_seed = *seed;
    *seed = seed.overflowing_mul(6364136223846793005u64).0 + 3217;
    let shifted = (((old_seed >> 18) ^ old_seed) >> 27) as u32;
    let rot = (old_seed >> 59) as u32;
    (shifted >> rot) | shifted << (rot.overflowing_neg().0 & 31)
}

fn rng_64(seed: &mut u64) -> u64{
    let left = rng_32(seed) as u64;
    let right = rng_32(seed) as u64;
    (left << 32) | right
}

/// Gets the hash used to encrypt messages by checking for the "RSPI_SERVER_HASHKEY" enviorment variable
fn get_hash() -> Result<u64, String>{
    let hashkey: u64 = match env::var("RSPI_SERVER_HASHKEY").unwrap_or(String::from("0")).parse(){
        Ok(n) => n, 
        Err(_) => return Err(String::from("RSPI_SERVER_HASHKEY enviorment variable cannoted be parsed to a u64!")),
    };
    let mut seed = time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() / 5;
    Ok(hashkey ^ rng_64(&mut seed))
}