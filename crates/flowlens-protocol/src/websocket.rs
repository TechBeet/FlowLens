//! WebSocket frame parsing for FlowLens
//!
//! Supports:
//! - Text, Binary, Close, Ping, Pong frames
//! - Masked and unmasked frames
//! - Extended payload lengths (16-bit and 64-bit)

use flowlens_core::models::{WebSocketFrame, WsOpcode};

/// Parse a WebSocket frame from raw bytes.
/// Returns (frame, bytes_consumed) if successful.
pub fn parse_websocket_frame(data: &[u8]) -> Option<(WebSocketFrame, usize)> {
    if data.len() < 2 {
        return None;
    }

    let _fin = (data[0] & 0x80) != 0;
    let opcode = match data[0] & 0x0F {
        0x0 => WsOpcode::Continuation,
        0x1 => WsOpcode::Text,
        0x2 => WsOpcode::Binary,
        0x8 => WsOpcode::Close,
        0x9 => WsOpcode::Ping,
        0xA => WsOpcode::Pong,
        _ => return None,
    };

    let masked = (data[1] & 0x80) != 0;
    let mut payload_len = (data[1] & 0x7F) as usize;
    let mut offset = 2;

    // Extended payload length
    if payload_len == 126 {
        if data.len() < offset + 2 {
            return None;
        }
        payload_len = u16::from_be_bytes([data[offset], data[offset + 1]]) as usize;
        offset += 2;
    } else if payload_len == 127 {
        if data.len() < offset + 8 {
            return None;
        }
        payload_len = u64::from_be_bytes([
            data[offset], data[offset + 1], data[offset + 2], data[offset + 3],
            data[offset + 4], data[offset + 5], data[offset + 6], data[offset + 7],
        ]) as usize;
        offset += 8;
    }

    // Masking key
    let mask_key = if masked {
        if data.len() < offset + 4 {
            return None;
        }
        let key = [data[offset], data[offset + 1], data[offset + 2], data[offset + 3]];
        offset += 4;
        Some(key)
    } else {
        None
    };

    if data.len() < offset + payload_len {
        return None;
    }

    let mut payload = data[offset..offset + payload_len].to_vec();

    // Unmask
    if let Some(key) = mask_key {
        for (i, byte) in payload.iter_mut().enumerate() {
            *byte ^= key[i % 4];
        }
    }

    let total_consumed = offset + payload_len;

    Some((
        WebSocketFrame {
            opcode,
            payload,
            payload_size: payload_len,
            is_masked: masked,
            timestamp: 0,
        },
        total_consumed,
    ))
}

/// Parse all WebSocket frames from data.
pub fn parse_all_frames(data: &[u8]) -> Vec<WebSocketFrame> {
    let mut frames = Vec::new();
    let mut pos = 0;
    while pos < data.len() {
        match parse_websocket_frame(&data[pos..]) {
            Some((frame, consumed)) => {
                frames.push(frame);
                pos += consumed;
            }
            None => break,
        }
    }
    frames
}

/// Create a masked WebSocket frame (client to server).
pub fn create_masked_frame(opcode: WsOpcode, payload: &[u8]) -> Vec<u8> {
    let mut frame = Vec::new();
    let first_byte = match opcode {
        WsOpcode::Text => 0x81,
        WsOpcode::Binary => 0x82,
        WsOpcode::Close => 0x88,
        WsOpcode::Ping => 0x89,
        WsOpcode::Pong => 0x8A,
        WsOpcode::Continuation => 0x80,
    };
    frame.push(first_byte);

    let mask_bit = 0x80;
    if payload.len() < 126 {
        frame.push((payload.len() as u8) | mask_bit);
    } else if payload.len() < 65536 {
        frame.push(126 | mask_bit);
        frame.extend_from_slice(&(payload.len() as u16).to_be_bytes());
    } else {
        frame.push(127 | mask_bit);
        frame.extend_from_slice(&(payload.len() as u64).to_be_bytes());
    }

    // Masking key (simple for testing)
    let mask_key = [0x37, 0xfa, 0x21, 0x3d];
    frame.extend_from_slice(&mask_key);

    for (i, &byte) in payload.iter().enumerate() {
        frame.push(byte ^ mask_key[i % 4]);
    }

    frame
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_unmasked_text_frame() {
        let data = vec![0x81, 0x05, b'h', b'e', b'l', b'l', b'o'];
        let (frame, consumed) = parse_websocket_frame(&data).unwrap();
        assert_eq!(frame.opcode, WsOpcode::Text);
        assert_eq!(frame.payload, b"hello");
        assert!(!frame.is_masked);
        assert_eq!(consumed, 7);
    }

    #[test]
    fn test_parse_masked_frame() {
        // Masked "hello" with key [0x37, 0xfa, 0x21, 0x3d]
        let mask_key = [0x37u8, 0xfa, 0x21, 0x3d];
        let payload = b"hello";
        let mut data = vec![0x81, 0x85];
        data.extend_from_slice(&mask_key);
        for (i, &b) in payload.iter().enumerate() {
            data.push(b ^ mask_key[i % 4]);
        }
        let (frame, consumed) = parse_websocket_frame(&data).unwrap();
        assert_eq!(frame.opcode, WsOpcode::Text);
        assert_eq!(frame.payload, b"hello");
        assert!(frame.is_masked);
        assert_eq!(consumed, 11);
    }

    #[test]
    fn test_parse_all_frames() {
        let mut data = vec![0x81, 0x03, b'a', b'b', b'c'];
        data.extend_from_slice(&[0x82, 0x02, b'x', b'y']);
        let frames = parse_all_frames(&data);
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].opcode, WsOpcode::Text);
        assert_eq!(frames[1].opcode, WsOpcode::Binary);
    }

    #[test]
    fn test_parse_extended_length() {
        let payload = vec![b'a'; 200];
        let mut data = vec![0x81, 0x7E];
        data.extend_from_slice(&(200u16).to_be_bytes());
        data.extend_from_slice(&payload);
        let (frame, _) = parse_websocket_frame(&data).unwrap();
        assert_eq!(frame.payload_size, 200);
        assert_eq!(frame.payload.len(), 200);
    }
}