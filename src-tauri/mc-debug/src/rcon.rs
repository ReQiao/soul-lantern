use crate::Result;
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    time::{Duration, Instant},
};

pub struct Client {
    socket: TcpStream,
    next_id: i32,
}
enum PacketError {
    Idle,
    Other(String),
}
impl PacketError {
    fn message(self) -> String {
        match self {
            Self::Idle => "RCON 响应超时。".into(),
            Self::Other(e) => e,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{net::TcpListener, thread};

    fn response(id: i32, kind: i32, text: &str) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&((text.len() + 10) as i32).to_le_bytes());
        out.extend_from_slice(&id.to_le_bytes());
        out.extend_from_slice(&kind.to_le_bytes());
        out.extend_from_slice(text.as_bytes());
        out.extend_from_slice(&[0, 0]);
        out
    }
    fn request(stream: &mut TcpStream) -> i32 {
        let mut len = [0; 4];
        stream.read_exact(&mut len).unwrap();
        let mut body = vec![0; i32::from_le_bytes(len) as usize];
        stream.read_exact(&mut body).unwrap();
        i32::from_le_bytes(body[..4].try_into().unwrap())
    }
    fn server(handler: impl FnOnce(TcpStream) + Send + 'static) -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        thread::spawn(move || handler(listener.accept().unwrap().0));
        port
    }
    #[test]
    fn auth_ack_cannot_hide_auth_failure() {
        let port = server(|mut s| {
            let id = request(&mut s);
            s.write_all(&response(id, 0, "")).unwrap();
            s.write_all(&response(-1, 2, "")).unwrap();
        });
        assert!(
            Client::connect(port, "wrong")
                .err()
                .unwrap()
                .contains("认证失败")
        );
    }
    #[test]
    fn command_fragments_are_collected_without_fence_packets() {
        let port = server(|mut s| {
            let id = request(&mut s);
            s.write_all(&response(id, 2, "")).unwrap();
            let id = request(&mut s);
            for body in ["first", "second"] {
                for chunk in response(id, 0, body).chunks(3) {
                    s.write_all(chunk).unwrap();
                    thread::sleep(Duration::from_millis(2));
                }
            }
            thread::sleep(Duration::from_millis(400));
        });
        assert_eq!(
            Client::connect(port, "ok")
                .unwrap()
                .send("say hello")
                .unwrap(),
            "firstsecond"
        );
    }
    #[test]
    fn closed_connection_is_never_a_successful_empty_response() {
        let port = server(|mut s| {
            let id = request(&mut s);
            s.write_all(&response(id, 2, "")).unwrap();
            let _ = request(&mut s);
        });
        assert!(
            Client::connect(port, "ok")
                .unwrap()
                .send("say hello")
                .is_err()
        );
    }
}
impl Client {
    pub fn connect(port: u16, password: &str) -> Result<Self> {
        let address: SocketAddr = ([127, 0, 0, 1], port).into();
        let socket = TcpStream::connect_timeout(&address, Duration::from_secs(2))
            .map_err(|e| e.to_string())?;
        socket
            .set_write_timeout(Some(Duration::from_secs(5)))
            .map_err(|e| e.to_string())?;
        let mut client = Self { socket, next_id: 1 };
        let id = client.write(3, password)?;
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let (received, kind, _) = client.packet(deadline).map_err(PacketError::message)?;
            if kind == 2 && received == -1 {
                return Err("本机调试 RCON 认证失败。".into());
            }
            if kind == 2 && received == id {
                return Ok(client);
            }
        }
    }
    fn write(&mut self, kind: i32, body: &str) -> Result<i32> {
        let id = self.next_id;
        self.next_id += 1;
        let mut packet = Vec::with_capacity(body.len() + 14);
        packet.extend_from_slice(&((body.len() + 10) as i32).to_le_bytes());
        packet.extend_from_slice(&id.to_le_bytes());
        packet.extend_from_slice(&kind.to_le_bytes());
        packet.extend_from_slice(body.as_bytes());
        packet.extend_from_slice(&[0, 0]);
        self.socket
            .write_all(&packet)
            .map_err(|e| format!("发送 RCON 请求失败：{e}"))?;
        Ok(id)
    }
    fn read_exact(&mut self, buffer: &mut [u8], deadline: Instant) -> Result<()> {
        let timeout = deadline
            .checked_duration_since(Instant::now())
            .ok_or("RCON 响应超时。")?;
        self.socket
            .set_read_timeout(Some(timeout))
            .map_err(|e| e.to_string())?;
        self.socket
            .read_exact(buffer)
            .map_err(|e| format!("读取 RCON 响应失败：{e}"))
    }
    fn packet(
        &mut self,
        deadline: Instant,
    ) -> std::result::Result<(i32, i32, String), PacketError> {
        let mut length = [0; 4];
        let mut read = 0;
        while read < 4 {
            let timeout = deadline
                .checked_duration_since(Instant::now())
                .ok_or_else(|| PacketError::Other("RCON 响应超时。".into()))?;
            self.socket
                .set_read_timeout(Some(timeout))
                .map_err(|e| PacketError::Other(e.to_string()))?;
            match self.socket.read(&mut length[read..]) {
                Ok(0) => return Err(PacketError::Other("RCON 连接已关闭。".into())),
                Ok(n) => read += n,
                Err(e)
                    if read == 0
                        && matches!(
                            e.kind(),
                            std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                        ) =>
                {
                    return Err(PacketError::Idle);
                }
                Err(e) => return Err(PacketError::Other(format!("RCON 响应不完整：{e}"))),
            }
        }
        let length = i32::from_le_bytes(length);
        if !(10..=4_194_304).contains(&length) {
            return Err(PacketError::Other("RCON 响应长度无效。".into()));
        }
        let mut packet = vec![0; length as usize];
        self.read_exact(&mut packet, deadline)
            .map_err(PacketError::Other)?;
        if packet[packet.len() - 2..] != [0, 0] {
            return Err(PacketError::Other("RCON 响应结尾无效。".into()));
        }
        Ok((
            i32::from_le_bytes(packet[..4].try_into().unwrap()),
            i32::from_le_bytes(packet[4..8].try_into().unwrap()),
            String::from_utf8_lossy(&packet[8..packet.len() - 2]).into_owned(),
        ))
    }
    pub fn send(&mut self, command: &str) -> Result<String> {
        let id = self.write(2, command)?;
        let hard_deadline = Instant::now() + Duration::from_secs(30);
        let mut body = String::new();
        let mut received = false;
        loop {
            let deadline = if received {
                (Instant::now() + Duration::from_millis(200)).min(hard_deadline)
            } else {
                hard_deadline
            };
            match self.packet(deadline) {
                Ok((response_id, 0, fragment)) if response_id == id => {
                    received = true;
                    body.push_str(&fragment);
                    if body.len() > 1_000_000 {
                        return Err("RCON 输出过大。".into());
                    }
                }
                Ok(_) => {}
                Err(PacketError::Idle) if received => return Ok(body),
                Err(error) => return Err(error.message()),
            }
            if Instant::now() >= hard_deadline {
                return Err("RCON 响应超时。".into());
            }
        }
    }
}
