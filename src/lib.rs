use std::io::Write;
use std::net::SocketAddrV4;
use std::str::FromStr;

pub struct Finder{
    name: String,
    socket: Option<SocketAddrV4>,
    beacon_receiving_socket: std::net::UdpSocket,
    last_beacon_time: std::time::Instant
}

pub struct Beacon{
    data: Vec<u8>
}

pub enum Status{
    Found(SocketAddrV4),
    TimeSince(std::time::Duration)
}

struct Target<'a>{
    name: &'a str,
    source_address: SocketAddrV4,
    port: u16
}

pub fn new(name: String) -> Result<Finder, std::io::Error>{
    let last_beacon_time = std::time::Instant::now();
    let beacon_receiving_socket = std::net::UdpSocket::bind("0.0.0.0:9092")?;
    beacon_receiving_socket.set_nonblocking(true);
    Ok( Finder{name, socket: None, last_beacon_time, beacon_receiving_socket} )
}

pub fn beacon(name: &str, address: u16) -> Beacon{
    let mut data = vec![0u8; name.len() + 8];
    write!(&mut data[..], "-{}-{}-", name, address);
    Beacon{data}
}

impl Finder{
    fn check_socket(&self) -> Option<(std::net::SocketAddrV4, String)>{

        let mut data_found = None;
        let mut source_address: SocketAddrV4;
        let mut buffer = [0u8; 500];

        let mut latest_message = None;
        loop{
            match self.beacon_receiving_socket.recv_from(&mut buffer){
                Ok(packet) => {
                    latest_message = Some(packet);
                }
                Err(error) => {
                    break;
                }
            }
        }
        if let Some(packet) = latest_message{
            source_address = match packet.1{
                std::net::SocketAddr::V4(src) => src,
                _ => panic!("V6 not supported")
            };
            let data = String::from_utf8(buffer.to_vec()).unwrap();
            data_found = Some( (source_address, data) );
        }
        data_found
    }

    fn extract_message<'a>(&self, data: &'a str) -> (&'a str, u16){
        let mut data = data.split('-');
        let name = data.nth(1).unwrap();
        let port = data.next().unwrap();
        let port: u16 = port.parse().unwrap();
        (name, port)
    }

    fn check_for_change(&mut self, target: &Target) -> Option<SocketAddrV4> {
        if target.name == self.name {
            match self.socket{
                Some(ref mut socket) => { 
                    if socket.ip() != target.source_address.ip() || 
                        socket.port() != target.port{
                        socket.set_ip( target.source_address.ip().clone() );
                        socket.set_port( target.port );

                        return Some( socket.clone() );
                    }
                },
                None => {
                    let socket = SocketAddrV4::new( target.source_address.ip().clone(), 
                                                    target.port );
                    self.socket = Some( socket.clone() );
                    return Some( socket.clone() );
                }
            }
        }
       None 
    }

    pub fn poll_status(&mut self) -> Result<Status, std::io::Error>{
        let mut status: Status;
        let data_found = self.check_socket();

        match data_found{
            Some( (source_address, data) ) => {
                let (name, port) = self.extract_message(& data);

                let target = Target{source_address, name, port};

                match self.check_for_change(&target){
                    Some(socket) => {
                        status = Status::Found(socket);
                    },
                    None =>{
                        status = Status::TimeSince( std::time::Duration::from_secs(0) );
                    }
                }
                self.last_beacon_time = std::time::Instant::now();
            },
            None => {
                let change_in_time = self.last_beacon_time.elapsed();
                status = Status::TimeSince(change_in_time);
            }
        }


        Ok(status)
    }
}

impl Beacon{
    pub fn send(&self) -> Result<(), std::io::Error> {
        let socket = std::net::UdpSocket::bind("0.0.0.0:9091")?;
        socket.set_broadcast(true)?;
        socket.send_to(&self.data[..], "255.255.255.255:9092")?;
        Ok(())
    }
}
