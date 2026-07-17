use std::{fmt::Write as _, time::Duration};

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use hkdf::Hkdf;
use pbkdf2::pbkdf2_hmac;
use rand::{rngs::OsRng, RngCore};
use rust_pake::pake::{Pake, PakePubKey, Role, SIEC255Params};
use sha2::{Digest, Sha256};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::timeout,
};

pub const RELAY_ADDRESS: &str = "croc.schollz.com:9009";
const RELAY_PASSWORD: &[u8] = b"pass123";
const FRAME_MAGIC: &[u8; 4] = b"croc";
const MAX_FRAME_BYTES: u32 = 16 * 1024 * 1024;
const PAYLOAD_PREFIX: &[u8] = b"OPENBURN-CROC-1\0";
const ACK: &[u8] = b"OPENBURN-CROC-ACK";

pub type ProtocolResult<T> = std::result::Result<T, String>;

pub struct CrocConnection {
    stream: TcpStream,
}

impl CrocConnection {
    pub async fn connect(code: &str) -> ProtocolResult<Self> {
        let room = room_name(code)?;
        log::info!(
            "croc relay: connecting to {RELAY_ADDRESS} (code length={})",
            code.trim().len()
        );
        let stream = timeout(Duration::from_secs(20), TcpStream::connect(RELAY_ADDRESS))
            .await
            .map_err(|_| format!("timed out connecting to croc relay {RELAY_ADDRESS}"))?
            .map_err(|error| format!("could not connect to croc relay {RELAY_ADDRESS}: {error}"))?;
        log::info!("croc relay: TCP connection established");
        let mut connection = Self { stream };
        connection.authenticate(&room).await?;
        log::info!("croc relay: room handshake completed");
        Ok(connection)
    }

    async fn authenticate(&mut self, room: &str) -> ProtocolResult<()> {
        log::debug!("croc relay: starting PAKE handshake");
        let mut pake = Pake::<SIEC255Params>::new(Role::Sender, Some(&[1, 2, 3]));
        self.send_frame(&serde_json::to_vec(&pake.pub_pake).map_err(|error| error.to_string())?)
            .await?;
        log::debug!("croc relay: PAKE public key sent");
        let peer: PakePubKey = serde_json::from_slice(&self.receive_frame().await?)
            .map_err(|error| format!("croc relay PAKE response is invalid: {error}"))?;
        pake.update(peer)
            .map_err(|error| format!("croc relay PAKE failed: {error}"))?;
        let session_key = pake
            .k
            .ok_or_else(|| "croc relay did not establish a session key".to_string())?;
        log::debug!("croc relay: PAKE session key established");

        let mut salt = [0u8; 8];
        OsRng.fill_bytes(&mut salt);
        self.send_frame(&salt).await?;
        let encryption_key = pbkdf2_key(&session_key, &salt);
        self.send_frame(&encrypt(&encryption_key, RELAY_PASSWORD)?)
            .await?;
        log::debug!("croc relay: relay password proof sent");

        let banner = loop {
            let response = self.receive_frame().await?;
            if response == [1] {
                continue;
            }
            let banner = decrypt(&encryption_key, &response).map_err(|error| {
                log::error!("croc relay: could not decrypt connection response: {error}");
                error
            })?;
            break banner;
        };
        log::debug!(
            "croc relay: connection response received: {}",
            display_bytes(&banner)
        );
        if !has_connection_banner_separator(&banner) {
            log::error!(
                "croc relay returned an invalid connection response: {}",
                display_bytes(&banner)
            );
            return Err(format!(
                "croc relay returned an invalid connection response: {}",
                display_bytes(&banner)
            ));
        }

        self.send_frame(&encrypt(&encryption_key, room.as_bytes())?)
            .await?;
        log::debug!("croc relay: room name sent");
        loop {
            let response = self.receive_frame().await?;
            if response == [1] {
                continue;
            }
            let response = decrypt(&encryption_key, &response)?;
            if response == b"ok" {
                log::debug!("croc relay: room accepted");
                return Ok(());
            }
            log::error!("croc relay rejected room: {}", display_bytes(&response));
            return Err(format!(
                "croc relay rejected the room: {}",
                display_bytes(&response)
            ));
        }
    }

    pub async fn send_frame(&mut self, payload: &[u8]) -> ProtocolResult<()> {
        if payload.len() > MAX_FRAME_BYTES as usize {
            return Err("croc transfer frame is too large".to_string());
        }
        self.stream
            .write_all(FRAME_MAGIC)
            .await
            .map_err(|error| format!("croc relay write failed: {error}"))?;
        self.stream
            .write_u32_le(payload.len() as u32)
            .await
            .map_err(|error| format!("croc relay write failed: {error}"))?;
        self.stream
            .write_all(payload)
            .await
            .map_err(|error| format!("croc relay write failed: {error}"))?;
        Ok(())
    }

    pub async fn receive_frame(&mut self) -> ProtocolResult<Vec<u8>> {
        let mut magic = [0u8; 4];
        self.stream
            .read_exact(&mut magic)
            .await
            .map_err(|error| format!("croc relay read failed: {error}"))?;
        if &magic != FRAME_MAGIC {
            return Err("croc relay sent an invalid frame".to_string());
        }
        let size = self
            .stream
            .read_u32_le()
            .await
            .map_err(|error| format!("croc relay read failed: {error}"))?;
        if size > MAX_FRAME_BYTES {
            return Err("croc relay sent an oversized frame".to_string());
        }
        let mut payload = vec![0u8; size as usize];
        self.stream
            .read_exact(&mut payload)
            .await
            .map_err(|error| format!("croc relay read failed: {error}"))?;
        Ok(payload)
    }
}

pub async fn send_payload(code: &str, payload: &str) -> ProtocolResult<()> {
    log::info!("croc transfer: sending payload ({} bytes)", payload.len());
    let mut connection = CrocConnection::connect(code).await?;
    let key = payload_key(code)?;
    let mut packet = PAYLOAD_PREFIX.to_vec();
    packet.extend(encrypt(&key, payload.as_bytes())?);
    connection.send_frame(&packet).await?;
    log::debug!("croc transfer: payload sent; waiting for receiver acknowledgement");

    loop {
        let response = timeout(Duration::from_secs(60), connection.receive_frame())
            .await
            .map_err(|_| "timed out waiting for croc receiver confirmation".to_string())??;
        if response == [1] {
            continue;
        }
        if response == ACK {
            log::info!("croc transfer: receiver acknowledgement received");
            return Ok(());
        }
        log::error!(
            "croc transfer: unexpected receiver response: {}",
            display_bytes(&response)
        );
        return Err("croc receiver returned an invalid confirmation".to_string());
    }
}

pub async fn receive_payload(code: &str) -> ProtocolResult<String> {
    log::info!("croc transfer: waiting for payload");
    let mut connection = CrocConnection::connect(code).await?;
    let key = payload_key(code)?;
    loop {
        let packet = connection.receive_frame().await?;
        if packet == [1] {
            continue;
        }
        if !packet.starts_with(PAYLOAD_PREFIX) {
            return Err("croc transfer payload is invalid".to_string());
        }
        let payload = decrypt(&key, &packet[PAYLOAD_PREFIX.len()..])?;
        let payload = String::from_utf8(payload)
            .map_err(|_| "croc transfer payload is not valid JSON".to_string())?;
        connection.send_frame(ACK).await?;
        log::info!("croc transfer: payload received ({} bytes)", payload.len());
        return Ok(payload);
    }
}

pub fn generate_code() -> String {
    const ALPHABET: &[u8] = b"abcdefghjkmnpqrstuvwxyz23456789";
    let mut random = [0u8; 30];
    OsRng.fill_bytes(&mut random);
    let prefix = random[..4]
        .iter()
        .map(|byte| char::from(b'0' + (byte % 10)))
        .collect::<String>();
    let suffix = random[4..]
        .iter()
        .map(|byte| char::from(ALPHABET[(*byte as usize) % ALPHABET.len()]))
        .collect::<String>();
    format!("{prefix}-{suffix}")
}

fn room_name(code: &str) -> ProtocolResult<String> {
    let code = code.trim();
    let prefix = code
        .as_bytes()
        .get(..4)
        .ok_or_else(|| "croc code is too short".to_string())?;
    if code.len() < 6 {
        return Err("croc code is too short".to_string());
    }
    let mut hash = Sha256::new();
    hash.update(prefix);
    hash.update(b"croc");
    let digest = hash.finalize();
    let mut room = String::with_capacity(64);
    for byte in digest {
        write!(&mut room, "{byte:02x}").expect("writing to String cannot fail");
    }
    Ok(room)
}

fn payload_key(code: &str) -> ProtocolResult<[u8; 32]> {
    let mut key = [0u8; 32];
    Hkdf::<Sha256>::new(Some(b"openburn-croc-v1"), code.trim().as_bytes())
        .expand(b"account-transfer", &mut key)
        .map_err(|_| "could not derive croc transfer key".to_string())?;
    Ok(key)
}

fn pbkdf2_key(password: &[u8], salt: &[u8]) -> [u8; 32] {
    let mut key = [0u8; 32];
    pbkdf2_hmac::<Sha256>(password, salt, 100, &mut key);
    key
}

#[allow(deprecated)]
fn encrypt(key: &[u8; 32], plaintext: &[u8]) -> ProtocolResult<Vec<u8>> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|error| error.to_string())?;
    let mut nonce = [0u8; 12];
    OsRng.fill_bytes(&mut nonce);
    let mut encrypted = nonce.to_vec();
    encrypted.extend(
        cipher
            .encrypt(Nonce::from_slice(&nonce), plaintext)
            .map_err(|_| "croc payload encryption failed".to_string())?,
    );
    Ok(encrypted)
}

#[allow(deprecated)]
fn decrypt(key: &[u8; 32], encrypted: &[u8]) -> ProtocolResult<Vec<u8>> {
    if encrypted.len() < 12 {
        return Err("croc encrypted payload is too short".to_string());
    }
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|error| error.to_string())?;
    cipher
        .decrypt(Nonce::from_slice(&encrypted[..12]), &encrypted[12..])
        .map_err(|_| "croc encrypted payload authentication failed".to_string())
}

fn display_bytes(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn has_connection_banner_separator(banner: &[u8]) -> bool {
    banner.windows(3).any(|window| window == b"|||")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_code_has_croc_shape() {
        let code = generate_code();
        assert_eq!(code.len(), 31);
        assert_eq!(code.as_bytes()[4], b'-');
        assert!(room_name(&code).unwrap().len() == 64);
    }

    #[test]
    fn payload_key_depends_on_full_code() {
        assert_ne!(
            payload_key("1234-alpha").unwrap(),
            payload_key("1234-bravo").unwrap()
        );
    }

    #[test]
    fn croc_pake_public_json_round_trips() {
        let pake = Pake::<SIEC255Params>::new(Role::Sender, Some(&[1, 2, 3]));
        let encoded = serde_json::to_vec(&pake.pub_pake).unwrap();
        assert!(!String::from_utf8_lossy(&encoded).contains("e+"));
        let _: PakePubKey = serde_json::from_slice(&encoded).unwrap();
    }

    #[test]
    fn accepts_relay_banner_with_data_ports_and_address() {
        assert!(has_connection_banner_separator(
            b"9010,9011,9012,9013|||103.84.5.168:6434"
        ));
    }
}
