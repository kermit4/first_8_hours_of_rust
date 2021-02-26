use std::env;
use std::os::unix::fs::FileExt;
use std::path::Path;
use std::io::Read;
use std::fs;
use std::fs::File;
use std::net::UdpSocket;
use bit_vec::BitVec;
use serde::{Serialize, Deserialize};

struct Upload {
	file: File,
	len: u64,
	first_missing: u64,
	next_missing: u64,
	requested: u64,
	highest_seen: u64,
	bitmap: BitVec,
	lastreq: u64,
	hash: [u8;32],  
	}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Content{
	len: u64,
	offset: u64,
	hash: [u8;32], 
	data: [u8;32], // something about serde being limited to 32 byte u8s??  i took a wrong turn there i guess
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct ReqAnother {
	offset: u64,
	hash: [u8;32],
}

fn main() {
	let args: Vec<String> = env::args().collect();
	if args.len() > 2 { 
		send(&args[1],&args[2]).expect("send()");
	} else { 
		receive().expect("receive");
	}
}

fn send(pathname: &String, host: &String)  -> Result<bool,std::io::Error> {
	let socket = UdpSocket::bind("0.0.0.0:0").expect("bind failed");
	let mut file = File::open(pathname)?;
    let metadata = fs::metadata(&pathname).expect("unable to read metadata");
    let mut buffer = [0;32]; // vec![0; 32 as usize];
    file.read(&mut buffer).expect("buffer overflow");
	let mut started = false;
	loop {
		let mut hash = [0u8; 32];
		hex::decode_to_slice("f000000000000000f000000000000000f000000000000000f000000000000000", &mut hash as &mut [u8]).expect("not hex"); // not the real hash obviously
		if !started { 
			let c =  Content {
				len: metadata.len(),
				offset: 0,
				hash: hash,
				data: buffer,
			};
			started=true;
			send_block(c,host,&socket,&file);
		} else { 
			let mut buf = [0;1500]; //	[0; ::std::mem::size_of::Content];
			let (_amt, _src) = socket.recv_from(&mut buf).expect("socket error");
		let req: ReqAnother = bincode::deserialize(&buf).unwrap();
		println!("sending block: {:?}", req.offset);
			let c =  Content {
				len: metadata.len(),
				offset: req.offset,
				hash: hash,
				data: buffer,
			};
			send_block(c,host,&socket,&file);
		}
	}
}

fn send_block(mut c: Content,host: &String, socket: &UdpSocket, file: &File) {
	file.read_at(&mut c.data,c.offset*32).expect("cant read");
	let encoded: Vec<u8> = bincode::serialize(&c).unwrap();
	socket.send_to(&encoded[..],host).expect("cant send_to");
}

fn blocks(len: u64) -> u64 {
	return (len+32-1) / 32;
}

fn receive() -> Result<bool,std::io::Error> { 
	let socket = UdpSocket::bind("0.0.0.0:34254").expect("bind failed");
	use std::collections::HashMap;
	let mut uploads = HashMap::new();
	loop {
		let mut buf = [0;1500]; //	[0; ::std::mem::size_of::Content];
		let (_amt, src) = socket.recv_from(&mut buf).expect("socket error");
		let c: Content = bincode::deserialize(&buf).unwrap();
		println!("recieved block: {:?}", c.offset);
		if ! uploads.contains_key(&hex::encode(c.hash)) { // new upload
			let u = Upload {
				hash: c.hash,
				lastreq:0,
				file: File::create(Path::new(&hex::encode(c.hash)))?,
				len: c.len,
				first_missing: 0,
				next_missing: 0,
				highest_seen: 0,
				requested: 0,
				bitmap: BitVec::from_elem(blocks(c.len) as usize, false),
			};
			uploads.insert(hex::encode(c.hash),u);
		} 
		let mut u = uploads.get_mut(&hex::encode(c.hash)).unwrap();
		u.file.write_at(&c.data, c.offset*32).expect("cant write");
		if u.bitmap.get(c.offset as usize).unwrap() {
			println!("dup: {:?}", c.offset);
		}
		if c.offset > u.highest_seen {
			u.highest_seen=c.offset
		}
		u.bitmap.set(c.offset as usize,true);
		if c.offset == u.first_missing {
			while u.first_missing < blocks(u.len) && u.bitmap.get(u.first_missing as usize).unwrap() {
				u.first_missing+=1;
			}
		}

		if u.first_missing==blocks(u.len) { // upload done
			u.file.set_len(u.len)?;
//			uploads.remove(&hex::encode(c.hash));  this will just start over if packets are in flight, so it needs a delay
			continue;
		}

		let mut m=ReqAnother { offset: 0, hash: c.hash,};
		u.lastreq+=1;
		if u.lastreq>=blocks(u.len) { // "done" but just filling in holes now
			m.offset=u.next_missing; u.next_missing+=1;
			println!("requesting missing block {:?}", m.offset);
		} else { 
			m.offset = u.lastreq;
			println!("requesting block {:?}", m.offset);
		}
		u.next_missing%=blocks(u.len);
		while u.bitmap.get(u.next_missing as usize).unwrap() {
			u.next_missing+=1;
			u.next_missing%=blocks(u.len);
		}
		m.hash=u.hash;
	
		let encoded: Vec<u8> = bincode::serialize(&m).unwrap();
		socket.send_to(&encoded[..],&src).expect("cant send_to");
		u.requested+=1;

		if (u.requested%30) == 0 { // push it to 1% packet loss
			if u.next_missing<u.highest_seen || u.lastreq+1>=blocks(u.len) {
				m.offset=u.next_missing; u.next_missing+=1;
				println!("requesting extra, missing block {:?}", m.offset);
			} else {
					u.lastreq+=1;
					m.offset=u.lastreq;
					println!("requesting extra block {:?}", m.offset);
			}
			u.next_missing%=blocks(u.len);
			while u.bitmap.get(u.next_missing as usize).unwrap() {
				u.next_missing+=1;
				u.next_missing%=blocks(u.len);
			}
			let encoded: Vec<u8> = bincode::serialize(&m).unwrap();
			socket.send_to(&encoded[..],&src).expect("cant send_to");
			u.requested+=1;
		}
	}
}
